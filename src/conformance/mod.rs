//! Modul Conformance Test Suite (CTS) & Compliance Harness Aurion.

pub mod report;
pub mod runner;
pub mod vectors;

pub use report::{generate_json_report, generate_markdown_report, print_terminal_report};
pub use runner::{run_all_pillars, PillarExecutionResult, TestStatus};
pub use vectors::generate_golden_vectors_json;
