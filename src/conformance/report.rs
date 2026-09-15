//! Pembangkit Laporan Kepatuhan Protokol (Terminal, JSON, Markdown).

use crate::conformance::runner::{PillarExecutionResult, TestStatus};

pub fn print_terminal_report(results: &[PillarExecutionResult]) {
    println!("\n================================================================================");
    println!("       AURION PROTOCOL CONFORMANCE & COMPLIANCE TEST REPORT (v1.0.0)");
    println!("================================================================================");
    println!("  Target: Canonical Aurion Single-Binary Runtime (/bin/aurion)");
    println!("  Normative Standard: RFC 2119 / RFC 8174 Compliance Verification");
    println!("--------------------------------------------------------------------------------");
    println!("  ID  | Status | Execution Time | Pillar Name");
    println!("--------------------------------------------------------------------------------");

    let mut all_passed = true;
    let mut total_time_micros = 0;

    for r in results {
        total_time_micros += r.duration_micros;
        let status_str = match &r.status {
            TestStatus::Passed => "\x1b[32m[PASS]\x1b[0m",
            TestStatus::Failed(_) => {
                all_passed = false;
                "\x1b[31m[FAIL]\x1b[0m"
            }
        };

        println!(
            "  #{:<2} | {:<6} | {:>8} µs | {}",
            r.pillar_id, status_str, r.duration_micros, r.name
        );
    }

    println!("--------------------------------------------------------------------------------");
    let total_time_ms = total_time_micros / 1000;
    let total_time_rem_us = total_time_micros % 1000;
    println!("  Total Verification Time: {}.{:03} ms", total_time_ms, total_time_rem_us);

    if all_passed {
        println!("\n  \x1b[32m>>> PROTOCOL COMPLIANCE VERDICT: 100% CANONICAL & PASS (8/8 PILARS) <<<\x1b[0m");
    } else {
        println!("\n  \x1b[31m>>> PROTOCOL COMPLIANCE VERDICT: AUDIT FAILED (VIOLATIONS DETECTED) <<<\x1b[0m");
    }
    println!("================================================================================\n");
}

pub fn generate_json_report(results: &[PillarExecutionResult]) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"system\": \"aurion\",\n");
    out.push_str("  \"version\": \"1.0.0\",\n");
    out.push_str("  \"specification\": \"V1-CANONICAL\",\n");
    out.push_str("  \"standard\": \"RFC-2119-NORMATIVE\",\n");
    out.push_str("  \"total_pillars\": 8,\n");

    let passed_count = results.iter().filter(|r| r.status == TestStatus::Passed).count();
    out.push_str(&format!("  \"passed_pillars\": {passed_count},\n"));
    out.push_str(&format!("  \"verdict\": \"{}\",\n", if passed_count == 8 { "CERTIFIED_CANONICAL" } else { "NON_COMPLIANT" }));
    out.push_str("  \"pillars\": [\n");

    for (idx, r) in results.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!("      \"pillar_id\": {},\n", r.pillar_id));
        out.push_str(&format!("      \"name\": \"{}\",\n", r.name));
        out.push_str(&format!("      \"status\": \"{}\",\n", match &r.status { TestStatus::Passed => "PASSED", TestStatus::Failed(_) => "FAILED" }));
        out.push_str(&format!("      \"duration_microseconds\": {},\n", r.duration_micros));
        out.push_str(&format!("      \"detail\": \"{}\"\n", r.detail));
        if idx + 1 < results.len() {
            out.push_str("    },\n");
        } else {
            out.push_str("    }\n");
        }
    }

    out.push_str("  ]\n");
    out.push_str("}\n");
    out
}

pub fn generate_markdown_report(results: &[PillarExecutionResult]) -> String {
    let mut out = String::new();
    out.push_str("# Laporan Audit Kepatuhan Protokol Aurion (v1.0.0)\n\n");
    out.push_str("> **Status Evaluasi:** ");
    let all_passed = results.iter().all(|r| r.status == TestStatus::Passed);
    if all_passed {
        out.push_str("**100% CANONICAL CERTIFIED (8/8 PILAR LOLOS)**\n\n");
    } else {
        out.push_str("**AUDIT GAGAL — TERDETEKSI KETIDAKSESUAIAN**\n\n");
    }

    out.push_str("| ID | Status | Waktu Eksekusi | Nama Pilar | Hasil Pengujian |\n");
    out.push_str("| :-: | :---: | :---: | :--- | :--- |\n");

    for r in results {
        let status_str = match &r.status {
            TestStatus::Passed => "✅ PASS",
            TestStatus::Failed(_) => "❌ FAIL",
        };
        out.push_str(&format!(
            "| {} | {} | {} µs | {} | {} |\n",
            r.pillar_id, status_str, r.duration_micros, r.name, r.detail
        ));
    }

    out.push_str("\n---\n*Dihasilkan secara otomatis oleh Aurion Conformance Test Harness (`aurion conformance run`).*\n");
    out
}
