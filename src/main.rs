// src/main.rs
// File ini berfungsi sebagai entry point aplikasi dan orkestrator alur kerja.
// Bertanggung jawab atas: parsing argumen CLI, validasi input, manajemen I/O file,
// pemanggilan modul analisis, dan formatting output ke terminal.

use std::env;
use std::fs::File;
use std::io::BufReader;
use std::process;

// Deklarasi modul analyzer.
// Rust menggunakan sistem modul untuk memisahkan concern.
// Kata kunci 'mod' memberi tahu kompiler bahwa logika parsing dan agregasi
// berada di file 'src/analyzer.rs'.
mod analyzer;

/// Fungsi utama aplikasi.
///
/// SIGNATURE EXPLANATION:
/// - `fn main()`: Titik masuk standar program Rust.
/// - `-> Result<(), Box<dyn std::error::Error>>`: Mengembalikan tipe Result.
///   * `Ok(())`: Menandakan eksekusi berhasil tanpa nilai balikan.
///   * `Err(...)`: Menangkap error apa pun yang mengimplementasikan trait std::error::Error.
///   * Penggunaan Result di main memungkinkan operator '?' untuk error propagation.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. PENGAMBILAN ARGUMEN CLI
    let args: Vec<String> = env::args().collect();

    // 2. VALIDASI INPUT MANUAL
    if args.len() < 2 {
        eprintln!("Error: Path file log wajib disediakan sebagai argumen pertama.");
        eprintln!("Penggunaan: {} <path_file_log> [level_filter]", args[0]);
        eprintln!("Contoh: cargo run application.log ERROR");

        process::exit(1);
    }

    let file_path = &args[1];

    let level_filter = if args.len() > 2 {
        Some(args[2].to_uppercase())
    } else {
        None
    };

    // 3. MEMBUKA FILE DENGAN ERROR HANDLING TERPUSAT
    let file = File::open(file_path)?;

    // 4. OPTIMASI I/O DENGAN BUFFERED READER
    let reader = BufReader::new(file);

    // 5. DELEGASI LOGIKA ANALISIS KE MODUL KEDUA
    // Kompiler Rust secara otomatis menyimpulkan tipe AnalysisResult dari signature
    // fungsi parse_and_analyze, sehingga import eksplisit tidak diperlukan dan dihapus.
    let analysis_result = analyzer::parse_and_analyze(reader, level_filter.as_deref())?;

    // 6. RENDERING OUTPUT KE TERMINAL
    println!("=== Log Analysis Report ===");
    println!("Target File : {}", file_path);
    println!("Total Lines : {}", analysis_result.total_lines);
    println!("Filtered    : {}", analysis_result.filtered_lines);

    println!("Level Distribution:");
    for (level, count) in &analysis_result.level_counts {
        println!("  - {:<10}: {}", level, count);
    }
    println!("===========================");

    Ok(())
}
