//! GERBANG KONSISTENSI KONSENSUS AURION (anti-regresi terminologi & skema).
//!
//! # Mengapa test ini ada
//!
//! Aurion adalah protokol **BFT deterministik dengan round-based finality**.
//! Konstitusi Pasal 2 secara eksplisit **melarang** konsep penambangan (*mining*),
//! kalkulasi tingkat kesulitan (*difficulty*), dan kompetisi hash.
//!
//! Sejarah: sebuah agen pernah menggabungkan model *account-based* dengan
//! *mining*, dan terminology serta skema fee lamanya **membocor ke lapisan
//! produksi** (receipt STF, prompt wallet, audit runner, conformance vectors).
//! Skema lama itu bahkan bertentangan dengan implementasi aktual di
//! `MonetaryState::split_fee`, sehingga validasi bisa menguji sesuatu yang salah.
//!
//! # Fungsi test ini
//!
//! Mengubah "&minus;" tidak di-github" menjadi aturan yang **dapat diuji**.
//! Jika agen berikutnya reintroduksi mining atau skema 20/80, `cargo test`
//! GAGAL dengan pesan yang menyebut file dan baris.
//!
//! ```bash
//! cargo test --offline --test bft_purity_gate
//! ```

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Kumpulkan file `.rs` rekursif di bawah `dir`.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Pola skema fee lama yang sudah dihapus dari kanonik.
const FORBIDDEN_FEE_PATTERNS: &[(&str, &str)] = &[
    (
        "fee_miner",
        "Field fee_miner* sudah dihapus; pakai fee_validator.",
    ),
    (
        "fee_burn_percent\": 20",
        "Skema 20% burn dihapus; kanonik 0% burn.",
    ),
    (
        "20% burn",
        "Skema 20% burn dihapus; kanonik 0% burn / 100% validator.",
    ),
    (
        "80% miner",
        "Skema 80% miner dihapus; kanonik 100% validator BFT.",
    ),
];

// ---------------------------------------------------------------------------
// 1. Terminologi mining NOL di lapisan produksi
// ---------------------------------------------------------------------------

#[test]
fn production_source_has_no_mining_terminology() {
    let mut files = Vec::new();
    rust_files(&src_dir(), &mut files);
    assert!(
        !files.is_empty(),
        "tidak ditemukan file sumber untuk diperiksa"
    );

    let mut violations: Vec<String> = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for (line_no, line) in text.lines().enumerate() {
            let lower = line.to_lowercase();
            for term in ["miner", "mining"] {
                if !lower.contains(term) {
                    continue;
                }
                // Komentar yang secara eksplisit MENCATAT penghapusan skema
                // lama diperbolehkan, karena itu justru dokumentasi.
                let documenting_removal = lower.contains("dihapus")
                    || lower.contains("terhapus")
                    || lower.contains("dilarang")
                    || lower.contains("forbidden");
                // String literal di dalam assertion guard (`.contains("miner")`)
                // juga diperbolehkan: itu alat deteksi, bukan terminologi.
                let is_guard_literal =
                    lower.contains(".contains(") || lower.contains("contains(\"");
                if !documenting_removal && !is_guard_literal {
                    violations.push(format!(
                        "{}:{}: '{term}'\n    | {}",
                        file.display(),
                        line_no + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Ditemukan {} kontaminasi terminologi mining di lapisan produksi:\n\n{}",
        violations.len(),
        violations.join("\n")
    );
}
// ---------------------------------------------------------------------------
// 2. Skema fee lama tidak boleh muncul sebagai kode yang bisa dieksekusi
// ---------------------------------------------------------------------------

#[test]
fn no_hardcoded_legacy_fee_split_in_production() {
    let mut files = Vec::new();
    rust_files(&src_dir(), &mut files);

    let mut violations: Vec<String> = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for (line_no, line) in text.lines().enumerate() {
            // Baris komentar bukan kode eksekutabel.
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with('*') {
                continue;
            }
            let lower = line.to_lowercase();
            for (pattern, reason) in FORBIDDEN_FEE_PATTERNS {
                if lower.contains(pattern) {
                    violations.push(format!(
                        "{}:{}: '{pattern}' -> {reason}\n    | {}",
                        file.display(),
                        line_no + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Ditemukan skema fee lama yang masih bisa dieksekusi:\n\n{}",
        violations.join("\n")
    );
}

// ---------------------------------------------------------------------------
// 3. Kanonik fee harus tetap 0% burn / 100% validator
// ---------------------------------------------------------------------------

#[test]
fn canonical_fee_constants_match_constitution() {
    // AUR-MON-003: 100% fee ke validator BFT, 0% burn.
    assert_eq!(
        aurion::core::FEE_BURN_PERCENTAGE,
        0,
        "FEE_BURN_PERCENTAGE harus 0 sesuai kanonik"
    );
    assert_eq!(
        aurion::core::FEE_VALIDATOR_PERCENTAGE,
        100,
        "FEE_VALIDATOR_PERCENTAGE harus 100 sesuai kanonik"
    );
}

#[test]
fn split_fee_matches_canonical_constants() {
    // Nilai yang benar-benar dipakai STF harus sama dengan konstanta.
    let fee = aurion::core::Quantum::new(100_000);
    let (burn, validator) = aurion::state::monetary::MonetaryState::split_fee(fee)
        .expect("split_fee tidak boleh gagal untuk fee wajar");
    assert_eq!(burn, aurion::core::Quantum::ZERO, "burn harus nol");
    assert_eq!(validator, fee, "validator menerima seluruh fee");
}

// ---------------------------------------------------------------------------
// 4. Tidak ada implementasi Proof-of-Work di konsensus
// ---------------------------------------------------------------------------

#[test]
fn no_proof_of_work_concepts_in_consensus() {
    let mut files = Vec::new();
    rust_files(&src_dir(), &mut files);

    // Pola yang secara teknis berarti kompetisi hash.
    let pow_patterns: &[&str] = &[
        "difficulty_target",
        "pow_hash",
        "mining_reward",
        "block_reward_per_hash",
        "nonce_search",
    ];

    let mut violations: Vec<String> = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let lower = text.to_lowercase();
        for pattern in pow_patterns {
            if lower.contains(pattern) {
                violations.push(format!("{}: mengandung '{pattern}'", file.display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Ditemukan konsep Proof-of-Work di lapisan produksi:\n\n{}",
        violations.join("\n")
    );
}
// ---------------------------------------------------------------------------
// 5. Konsistensi terminologi konsensus: ronde, BUKAN slot
// ---------------------------------------------------------------------------

/// Aurion adalah BFT **berbasis ronde**, bukan *single-slot*.
///
/// Audit menemukan dokumen dan komentar kode sama-sama mengklaim
/// "single-slot finality", padahal `consensus/bft/reactor.rs` mengimplementasikan
/// round timeout + round advance (lihat `docs/operations/BFT_PURITY_AUDIT.md`
/// §9). Proyek memilih menyelaraskan dokumentasi dengan kode; test ini menjaga
/// agar penyimpangan itu tidak muncul kembali diam-diam.
#[test]
fn no_single_slot_terminology_anywhere() {
    // Cakupan: seluruh file .rs dan .md di repo, kecuali `target/` dan `.git/`.
    let mut violations: Vec<String> = Vec::new();
    let mut checked = 0usize;

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == "target" || name == ".git" || name == "node_modules" {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let is_source = path.extension().is_some_and(|e| e == "rs");
            let is_doc = path.extension().is_some_and(|e| e == "md");
            if !is_source && !is_doc {
                continue;
            }
            // Berkas gerbang ini sendiri menyebut istilah yang dicari (string
            // deteksi + pesan galat), jadi dikecualikan agar test tidak
            // mendeteksi dirinya sendiri.
            if path.file_name().is_some_and(|n| n == "bft_purity_gate.rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            checked += 1;
            let lower = text.to_lowercase();
            if let Some(pos) = lower
                .find("single-slot")
                .or_else(|| lower.find("single slot"))
            {
                let line_no = text[..pos].matches('\n').count() + 1;
                violations.push(format!("{}:{line_no}", path.display()));
            }
        }
    }

    assert!(
        checked > 50,
        "pemindaian hanya menjangkau {checked} berkas; pola path mungkin salah"
    );
    assert!(
        violations.is_empty(),
        "Aurion adalah BFT berbasis ronde, BUKAN single-slot. Ditemukan {} klaim \
         'single-slot' pada:\n\n{}",
        violations.len(),
        violations.join("\n")
    );
}

/// Membuktikan implementasi konsensus memang multi-round, sehingga istilah
/// "single-slot" tidak mungkin dianggap sekadar penyebutan dokumentasi.
#[test]
fn consensus_implementation_is_round_based() {
    let reactor = std::fs::read_to_string(src_dir().join("consensus/bft/reactor.rs"))
        .expect("reactor.rs harus terbaca");

    // Round advance = bukti bahwa ini BFT multi-round, bukan slot tunggal.
    assert!(
        reactor.contains("current_round.saturating_add(1)"),
        "reactor harus menaikkan ronde (round advance) demi liveness"
    );
    // Round timeout harus memicu penanganan timeout.
    assert!(
        reactor.contains("round_timeout"),
        "reactor harus punya round timeout untuk liveness"
    );
}
