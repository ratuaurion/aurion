#![forbid(unsafe_code)]

//! Aurion Unified 54-Pillar Conformance Audit Matrix Integration Test.
//! Validates the full multi-layer compliance matrix across all 5 Aurion layers:
//! - Layer-1 Sovereign Core Base: 8 Pillars (REQ-L1-01..08)
//! - Layer-2 Scaling Layer: 10 Pillars (REQ-L2-01..10)
//! - Layer-3 Specialized Networks: 12 Pillars (REQ-L3-01..12)
//! - Layer-4 Interoperability: 12 Pillars (REQ-L4-01..12)
//! - Layer-5 Global Infrastructure: 12 Pillars (REQ-L5-01..12)
//!
//! Total: 54 Pillars (100% PASS, Zero unsafe_code, Zero float).

use aurion::conformance::{
    generate_matrix_json, generate_matrix_markdown, run_unified_matrix, LayerId, MatrixStatus,
    UnifiedConformanceMatrix,
};

#[test]
fn test_unified_matrix_execution_54_pillars() {
    let matrix = run_unified_matrix();

    // 1. Overall verification
    assert_eq!(
        matrix.total_pillars, 54,
        "Must audit exactly 54 pillars across L1..L5"
    );
    assert_eq!(matrix.passed_pillars, 54, "All 54 pillars must PASS");
    assert_eq!(
        matrix.failed_pillars, 0,
        "Zero failures allowed in conformance audit"
    );
    assert!(
        matrix.verdict.contains("100% CANONICAL CERTIFIED"),
        "Verdict must be CANONICAL CERTIFIED"
    );
    assert_eq!(
        matrix.overall_compliance_bps, 10_000,
        "Compliance rate must be exactly 100.00% (10,000 bps)"
    );

    // 2. Layer count verification
    assert_eq!(
        matrix.layers.len(),
        5,
        "Matrix must cover exactly 5 evolutionary layers"
    );

    // L1: 8 Pillars
    let l1 = &matrix.layers[0];
    assert_eq!(l1.layer, LayerId::L1);
    assert_eq!(l1.total_pillars, 8);
    assert_eq!(l1.passed_pillars, 8);
    assert_eq!(l1.failed_pillars, 0);
    assert_eq!(l1.compliance_rate_bps, 10_000);

    // L2: 10 Pillars
    let l2 = &matrix.layers[1];
    assert_eq!(l2.layer, LayerId::L2);
    assert_eq!(l2.total_pillars, 10);
    assert_eq!(l2.passed_pillars, 10);
    assert_eq!(l2.failed_pillars, 0);
    assert_eq!(l2.compliance_rate_bps, 10_000);

    // L3: 12 Pillars
    let l3 = &matrix.layers[2];
    assert_eq!(l3.layer, LayerId::L3);
    assert_eq!(l3.total_pillars, 12);
    assert_eq!(l3.passed_pillars, 12);
    assert_eq!(l3.failed_pillars, 0);
    assert_eq!(l3.compliance_rate_bps, 10_000);

    // L4: 12 Pillars
    let l4 = &matrix.layers[3];
    assert_eq!(l4.layer, LayerId::L4);
    assert_eq!(l4.total_pillars, 12);
    assert_eq!(l4.passed_pillars, 12);
    assert_eq!(l4.failed_pillars, 0);
    assert_eq!(l4.compliance_rate_bps, 10_000);

    // L5: 12 Pillars
    let l5 = &matrix.layers[4];
    assert_eq!(l5.layer, LayerId::L5);
    assert_eq!(l5.total_pillars, 12);
    assert_eq!(l5.passed_pillars, 12);
    assert_eq!(l5.failed_pillars, 0);
    assert_eq!(l5.compliance_rate_bps, 10_000);

    // 3. Individual pillar verification
    assert_eq!(matrix.pillars.len(), 54);
    for pillar in &matrix.pillars {
        assert_eq!(
            pillar.status,
            MatrixStatus::Passed,
            "Pillar {} must pass",
            pillar.requirement_id
        );
        assert!(
            !pillar.requirement_id.is_empty(),
            "Requirement ID cannot be empty"
        );
        assert!(!pillar.title.is_empty(), "Pillar title cannot be empty");
        assert!(
            !pillar.invariant.is_empty(),
            "Invariant reference cannot be empty"
        );
        assert!(!pillar.details.is_empty(), "Pillar details cannot be empty");
    }
}

#[test]
fn test_matrix_json_serialization_and_roundtrip() {
    let matrix = run_unified_matrix();
    let json_str = generate_matrix_json(&matrix);

    assert!(!json_str.is_empty(), "JSON output cannot be empty");
    assert!(json_str.contains("\"total_pillars\": 54"));
    assert!(json_str.contains("100% CANONICAL CERTIFIED"));
    assert!(json_str.contains("\"overall_compliance_bps\": 10000"));

    // Verify roundtrip deserialization
    let decoded: UnifiedConformanceMatrix = serde_json::from_str(&json_str)
        .expect("JSON matrix must deserialize cleanly into UnifiedConformanceMatrix");

    assert_eq!(decoded.total_pillars, 54);
    assert_eq!(decoded.passed_pillars, 54);
    assert_eq!(decoded.failed_pillars, 0);
    assert!(decoded.verdict.contains("100% CANONICAL CERTIFIED"));
    assert_eq!(decoded.layers.len(), 5);
    assert_eq!(decoded.pillars.len(), 54);
}

#[test]
fn test_matrix_markdown_report_generation() {
    let matrix = run_unified_matrix();
    let md = generate_matrix_markdown(&matrix);

    assert!(!md.is_empty(), "Markdown output cannot be empty");
    assert!(md.contains("# Aurion Unified Conformance Audit Matrix"));
    assert!(md.contains("100% CANONICAL CERTIFIED"));
    assert!(md.contains("100.00%"));
    assert!(md.contains("54/54 Pillars"));

    // Check layer summary table
    assert!(md.contains("Layer-1 (Sovereign Core)"));
    assert!(md.contains("Layer-2 (Scaling Rollup)"));
    assert!(md.contains("Layer-3 (Specialized Domains)"));
    assert!(md.contains("Layer-4 (Interoperability)"));
    assert!(md.contains("Layer-5 (Global Infrastructure)"));

    // Check requirement IDs from all 5 layers
    assert!(md.contains("REQ-L1-01"));
    assert!(md.contains("REQ-L1-08"));
    assert!(md.contains("REQ-L2-01"));
    assert!(md.contains("REQ-L2-10"));
    assert!(md.contains("REQ-L3-01"));
    assert!(md.contains("REQ-L3-12"));
    assert!(md.contains("REQ-L4-01"));
    assert!(md.contains("REQ-L4-12"));
    assert!(md.contains("REQ-L5-01"));
    assert!(md.contains("REQ-L5-12"));
}

#[test]
fn test_matrix_file_export_roundtrip() {
    let matrix = run_unified_matrix();
    let temp_dir = std::env::temp_dir();
    let json_path = temp_dir.join("aurion_test_matrix.json");
    let md_path = temp_dir.join("aurion_test_matrix.md");

    // Write exports
    let json_data = generate_matrix_json(&matrix);
    let md_data = generate_matrix_markdown(&matrix);

    std::fs::write(&json_path, &json_data).expect("Failed to write test JSON");
    std::fs::write(&md_path, &md_data).expect("Failed to write test MD");

    // Verify written content
    let read_json = std::fs::read_to_string(&json_path).expect("Failed to read test JSON");
    let read_md = std::fs::read_to_string(&md_path).expect("Failed to read test MD");

    assert_eq!(read_json, json_data);
    assert_eq!(read_md, md_data);

    // Clean up
    let _ = std::fs::remove_file(&json_path);
    let _ = std::fs::remove_file(&md_path);
}
