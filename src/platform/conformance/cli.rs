//! Antarmuka CLI untuk Conformance Test Suite (CTS) Aurion.

use std::fs;
use std::process;
use crate::conformance::{
    generate_golden_vectors_json, generate_json_report, generate_markdown_report,
    print_terminal_report, run_all_pillars, TestStatus,
};

pub fn handle_conformance_subcommand(subargs: &[String]) {
    let subcmd = if !subargs.is_empty() {
        subargs[0].as_str()
    } else {
        "run"
    };

    match subcmd {
        "run" => {
            println!("\n[AURION CTS] Initializing 8-Pillar Protocol Conformance Audit...");
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
            println!("Usage: aurion conformance <run|export-vectors> [options]");
            println!("  run [--export report.json] [--export-md report.md]");
            println!("  export-vectors [--output vectors.json]");
        }
    }
}
