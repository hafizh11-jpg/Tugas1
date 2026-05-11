// src/analyzer.rs
// Modul ini menangani logika inti parsing, filtering, dan agregasi data log.
// Bertanggung jawab atas: ekstraksi level log, validasi format, perhitungan statistik,
// dan penanganan error pada data input yang tidak terduga.

use std::collections::HashMap;
use std::io::BufRead;

/// Struct untuk menyimpan hasil analisis log.
///
/// ARSITEKTUR & DESIGN:
/// - Derive(Debug, Clone) memungkinkan inspeksi nilai di debugger dan duplikasi aman jika diperlukan.
/// - Field menggunakan tipe u64 untuk mencegah integer overflow pada file log enterprise (>4 miliar baris).
/// - HashMap digunakan karena kompleksitas lookup O(1) dan tidak memerlukan urutan tetap.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub total_lines: u64,
    pub filtered_lines: u64,
    pub level_counts: HashMap<String, u64>,
}

/// Fungsi utama untuk parsing dan analisis.
///
/// SIGNATURE EXPLANATION (Untuk semua tingkat):
/// - reader: impl BufRead adalah pola "generic interface injection". Fungsi ini tidak terikat
///   pada tipe File, melainkan pada kemampuan membaca buffer. Ini memudahkan unit testing
///   karena kita bisa mensimulasikan file dengan array byte di memori.
/// - level_filter: Option<&str> adalah pola nullable string. None = tidak ada filter,
///   Some("ERROR") = hanya hitung baris ERROR.
/// - Return Type: Result<..., Box<dyn std::error::Error>> adalah container error polimorfik.
///   Di bahasa lain, ini setara dengan "throw/catch Exception". Di Rust, ini memaksa pemanggil
///   untuk secara eksplisit menangani atau meneruskan error, mencegah silent failure.
pub fn parse_and_analyze(
    reader: impl BufRead,
    level_filter: Option<&str>,
) -> Result<AnalysisResult, Box<dyn std::error::Error>> {
    // Inisialisasi state analisis dalam scope fungsi.
    let mut total_lines: u64 = 0;
    let mut filtered_lines: u64 = 0;
    let mut level_counts: HashMap<String, u64> = HashMap::new();

    // Iterasi baris per baris. reader.lines() menghasilkan iterator of Result<String, IoError>.
    // Pola ini memungkinkan fault-tolerant processing: error pada 1 baris tidak menghentikan seluruh aplikasi.
    for line_result in reader.lines() {
        total_lines += 1;

        // ERROR HANDLING LEVEL I/O:
        // Jika terjadi error disk/encoding saat membaca baris, catat warning ke stderr
        // dan lanjut ke baris berikutnya. Ini adalah praktik standar untuk log analyzer production.
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "Warning: Gagal membaca baris ke-{}. Detail: {}",
                    total_lines, e
                );
                continue;
            }
        };

        // Abaikan baris kosong untuk menghindari noise statistik.
        if line.trim().is_empty() {
            continue;
        }

        // Ekstraksi level log menggunakan helper function.
        let level = extract_log_level(&line);

        // Update counter level.
        // entry().or_insert(0) adalah idiom Rust untuk "ambil nilai jika ada, jika tidak insert 0".
        // Menghindari branching manual (if contains -> get else -> insert).
        *level_counts.entry(level.clone()).or_insert(0) += 1;

        // Terapkan logika filter.
        // eq_ignore_ascii_case memastikan pencocokan case-insensitive tanpa alokasi string baru.
        let should_count = match &level_filter {
            Some(filter) => level.eq_ignore_ascii_case(filter),
            None => true,
        };

        if should_count {
            filtered_lines += 1;
        }
    }

    // Return hasil akhir. Konstruktor struct eksplisit di Rust meningkatkan kejelasan data mapping.
    Ok(AnalysisResult {
        total_lines,
        filtered_lines,
        level_counts,
    })
}

/// Fungsi helper untuk mengekstrak level log dari sebuah baris teks.
///
/// LOGIKA PARSING & NORMALISASI:
/// Log sistem tidak memiliki standar baku. Fungsi ini menggunakan heuristic yang robust:
/// 1. Bersihkan karakter pembuka umum ([, -, :, spasi) yang sering muncul di format syslog/app log.
/// 2. Ambil token pertama yang terpisah oleh whitespace.
/// 3. Hapus punctuation trailing yang mungkin menempel (misal "INFO:" atau "ERROR,").
/// 4. Normalisasi alias (WARNING -> WARN, CRITICAL/PANIC -> FATAL) untuk konsistensi agregasi.
/// 5. Jika tidak cocok dengan standar industri, kembalikan "OTHER".
fn extract_log_level(line: &str) -> String {
    let trimmed = line.trim_start_matches(|c: char| {
        c.is_whitespace() || c == '[' || c == ']' || c == ':' || c == '-' || c == '.' || c == ','
    });

    let first_token = trimmed.split_whitespace().next().unwrap_or("UNKNOWN");

    let clean_token = first_token.trim_end_matches(|c: char| c == ':' || c == ',' || c == '.');
    let upper_token = clean_token.to_uppercase();

    match upper_token.as_str() {
        "TRACE" | "DEBUG" | "INFO" | "WARN" | "WARNING" | "ERROR" | "FATAL" | "CRITICAL"
        | "PANIC" => match upper_token.as_str() {
            "WARNING" => "WARN".to_string(),
            "CRITICAL" | "FATAL" | "PANIC" => "FATAL".to_string(),
            other => other.to_string(),
        },
        _ => "OTHER".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Unit test memvalidasi alur parsing dasar tanpa filter.
    /// Cursor<&[u8]> mengimplementasikan BufRead, memungkinkan testing tanpa I/O disk.
    #[test]
    fn test_basic_log_parsing() {
        let sample_log =
            "INFO: Server started\n[ERROR] Database connection failed\nDEBUG: Cache cleared\n";
        let cursor = Cursor::new(sample_log.as_bytes());

        let result = parse_and_analyze(cursor, None).expect("Parsing should not fail");

        assert_eq!(result.total_lines, 3);
        assert_eq!(result.filtered_lines, 3);
        assert_eq!(*result.level_counts.get("INFO").unwrap_or(&0), 1);
        assert_eq!(*result.level_counts.get("ERROR").unwrap_or(&0), 1);
    }

    /// Unit test memvalidasi logika filter level.
    #[test]
    fn test_level_filter() {
        let sample_log = "INFO: OK\nERROR: Fail\nINFO: Retry\nWARN: Slow query\n";
        let cursor = Cursor::new(sample_log.as_bytes());

        let result = parse_and_analyze(cursor, Some("ERROR")).expect("Parsing should not fail");

        assert_eq!(result.total_lines, 4);
        assert_eq!(result.filtered_lines, 1);
        assert_eq!(*result.level_counts.get("ERROR").unwrap_or(&0), 1);
    }

    /// Unit test memvalidasi fault-tolerance terhadap baris rusak/kosong.
    #[test]
    fn test_fault_tolerance() {
        let sample_log = "\n   \n[INVALID_LINE_NO_LEVEL] Something happened\nERROR: Real error\n";
        let cursor = Cursor::new(sample_log.as_bytes());

        let result = parse_and_analyze(cursor, None).expect("Parsing should not fail");

        // Baris kosong diabaikan, line invalid masuk ke OTHER, ERROR terdeteksi.
        assert_eq!(result.total_lines, 4);
        assert_eq!(*result.level_counts.get("OTHER").unwrap_or(&0), 1);
        assert_eq!(*result.level_counts.get("ERROR").unwrap_or(&0), 1);
    }
}
