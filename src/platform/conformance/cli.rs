//! Antarmuka CLI untuk Conformance Test Suite (CTS) & Unified Matrix Aurion.

use crate::conformance::{
    generate_golden_vectors_json, generate_json_report, generate_markdown_report,
    generate_matrix_json, generate_matrix_markdown, print_terminal_matrix, print_terminal_report,
    run_all_pillars, run_unified_matrix, MatrixStatus, TestStatus,
};
use std::fs;
use std::process;

pub fn handle_conformance_subcommand(subargs: &[String]) {
    let subcmd = if !subargs.is_empty() {
        subargs[0].as_str()
    } else {
        "run"
    };

    match subcmd {
        "run" => {
            let has_all = subargs.iter().any(|arg| arg == "--all" || arg == "-a");

            if has_all {
                println!("\n[AURION CTS] Initializing Unified 54-Pillar Multi-Layer Conformance Audit (L1..L5)...");
                let matrix = run_unified_matrix();
                print_terminal_matrix(&matrix);

                // Handle exports
                let mut i = 1;
                while i < subargs.len() {
                    match subargs[i].as_str() {
                        "--export" | "-e" if i + 1 < subargs.len() => {
                            let path = &subargs[i + 1];
                            let json = generate_matrix_json(&matrix);
                            if let Err(e) = fs::write(path, json) {
                                eprintln!("Error writing JSON matrix to {path}: {e}");
                            } else {
                                println!("  [OK] Exported machine-readable JSON matrix: {path}");
                            }
                            i += 1;
                        }
                        "--export-md" if i + 1 < subargs.len() => {
                            let path = &subargs[i + 1];
                            let md = generate_matrix_markdown(&matrix);
                            if let Err(e) = fs::write(path, md) {
                                eprintln!("Error writing Markdown matrix to {path}: {e}");
                            } else {
                                println!("  [OK] Exported Markdown matrix: {path}");
                            }
                            i += 1;
                        }
                        _ => {}
                    }
                    i += 1;
                }

                let all_passed = matrix
                    .pillars
                    .iter()
                    .all(|p| p.status == MatrixStatus::Passed);
                if !all_passed {
                    process::exit(1);
                }
            } else {
                println!(
                    "\n[AURION CTS] Initializing 8-Pillar Protocol Conformance Audit (L1 Core)..."
                );
                println!("Tip: Use 'aurion conformance run --all' or 'aurion conformance matrix' to audit all 54 multi-layer pillars.");
                let results = run_all_pillars();
                print_terminal_report(&results);

                // Handle optional exports
                let mut i = 1;
                while i < subargs.len() {
                    match subargs[i].as_str() {
                        "--export" | "-e" if i + 1 < subargs.len() => {
                            let path = &subargs[i + 1];
                            let json = generate_json_report(&results);
                            if let Err(e) = fs::write(path, json) {
                                eprintln!("Error writing JSON report to {path}: {e}");
                            } else {
                                println!("  [OK] Exported machine-readable JSON report: {path}");
                            }
                            i += 1;
                        }
                        "--export-md" if i + 1 < subargs.len() => {
                            let path = &subargs[i + 1];
                            let md = generate_markdown_report(&results);
                            if let Err(e) = fs::write(path, md) {
                                eprintln!("Error writing Markdown report to {path}: {e}");
                            } else {
                                println!("  [OK] Exported Markdown report: {path}");
                            }
                            i += 1;
                        }
                        _ => {}
                    }
                    i += 1;
                }

                let all_passed = results.iter().all(|r| r.status == TestStatus::Passed);
                if !all_passed {
                    process::exit(1);
                }
            }
        }
        "matrix" => {
            println!(
                "\n[AURION CTS] Executing Unified 54-Pillar Conformance Audit Matrix (L1..L5)..."
            );
            let matrix = run_unified_matrix();
            print_terminal_matrix(&matrix);

            let mut i = 1;
            while i < subargs.len() {
                match subargs[i].as_str() {
                    "--export" | "-e" if i + 1 < subargs.len() => {
                        let path = &subargs[i + 1];
                        let json = generate_matrix_json(&matrix);
                        if let Err(e) = fs::write(path, json) {
                            eprintln!("Error writing JSON matrix to {path}: {e}");
                        } else {
                            println!("  [OK] Exported machine-readable JSON matrix: {path}");
                        }
                        i += 1;
                    }
                    "--export-md" if i + 1 < subargs.len() => {
                        let path = &subargs[i + 1];
                        let md = generate_matrix_markdown(&matrix);
                        if let Err(e) = fs::write(path, md) {
                            eprintln!("Error writing Markdown matrix to {path}: {e}");
                        } else {
                            println!("  [OK] Exported Markdown matrix: {path}");
                        }
                        i += 1;
                    }
                    _ => {}
                }
                i += 1;
            }

            let all_passed = matrix
                .pillars
                .iter()
                .all(|p| p.status == MatrixStatus::Passed);
            if !all_passed {
                process::exit(1);
            }
        }
        "export-vectors" => {
            let vectors_json = generate_golden_vectors_json();
            let mut output_path = None;

            let mut i = 1;
            while i < subargs.len() {
                if (subargs[i] == "--output" || subargs[i] == "-o") && i + 1 < subargs.len() {
                    output_path = Some(subargs[i + 1].clone());
                    i += 1;
                }
                i += 1;
            }

            if let Some(path) = output_path {
                if let Err(e) = fs::write(&path, &vectors_json) {
                    eprintln!("Error writing golden vectors to {path}: {e}");
                    process::exit(1);
                } else {
                    println!("  [OK] Successfully exported golden test vectors to: {path}");
                }
            } else {
                println!("{vectors_json}");
            }
        }
        _ => {
            println!("Usage: aurion conformance <run|matrix|export-vectors> [options]");
            println!("  run [--all] [--export report.json] [--export-md report.md]");
            println!("  matrix [--export matrix.json] [--export-md matrix.md]");
            println!("  export-vectors [--output vectors.json]");
        }
    }
}
