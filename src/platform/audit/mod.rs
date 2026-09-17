#![forbid(unsafe_code)]

//! Subsistem Audit Keamanan Eksternal & Pengerasan Penetrasi Aurion (PRD-013).
//!
//! Menyediakan tipe data struktural dan runner audit otomatis untuk mengevaluasi
//! kepatuhan kriptografis, konsensus, isolasi state machine, dan hygiene memori.
//!
//! Invariant Kunci:
//! - AUR-ARCH-001: Single Sovereign Ecosystem.
//! - AUR-ARCH-011: Absolute Zero Unsafe Code (#![forbid(unsafe_code)]).
//! - AUR-ARCH-012: Absolute Zero Float Arithmetic (Fixed Precision Quantum u128).

pub mod runner;

pub use runner::SecurityAuditRunner;

use serde::{Deserialize, Serialize};

/// Kategori cakupan audit keamanan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuditCategory {
    Cryptography,
    Consensus,
    StateMachine,
    Networking,
    VirtualMachine,
    Storage,
    MemoryHygiene,
}

impl AuditCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cryptography => "CRYPTOGRAPHY",
            Self::Consensus => "CONSENSUS",
            Self::StateMachine => "STATE_MACHINE",
            Self::Networking => "NETWORKING",
            Self::VirtualMachine => "VIRTUAL_MACHINE",
            Self::Storage => "STORAGE",
            Self::MemoryHygiene => "MEMORY_HYGIENE",
        }
    }
}

/// Tingkat keparahan audit keamanan formal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AuditSeverity {
    Informational,
    Low,
    Medium,
    High,
    Critical,
}

impl AuditSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Informational => "INFORMATIONAL",
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
            Self::Critical => "CRITICAL",
        }
    }
}

/// Status evaluasi per item uji audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditStatus {
    Passed,
    Mitigated,
    Failed,
}

impl AuditStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Passed => "PASSED",
            Self::Mitigated => "MITIGATED",
            Self::Failed => "FAILED",
        }
    }
}

/// Hasil evaluasi per klausul pengujian audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditCheckResult {
    pub id: String,
    pub name: String,
    pub category: AuditCategory,
    pub severity: AuditSeverity,
    pub invariant: String,
    pub status: AuditStatus,
    pub details: String,
}

/// Laporan resmi hasil audit keamanan komprehensif.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub title: String,
    pub version: String,
    pub timestamp: u64,
    pub total_checks: usize,
    pub passed_checks: usize,
    pub failed_checks: usize,
    pub readiness_verdict: String,
    pub results: Vec<AuditCheckResult>,
}

impl AuditReport {
    /// Menghasilkan representasi JSON yang rapi dan deterministik.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Menghasilkan dokumen ringkasan dalam format Markdown (murni integer, zero-float).
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        let pass_pct = (self.passed_checks * 100) / self.total_checks.max(1);
        md.push_str(&format!("# {}\n", self.title));
        md.push_str(&format!("> **Versi Biner:** {} | **Waktu Evaluasi:** {} | **Hasil:** **{}**\n\n", self.version, self.timestamp, self.readiness_verdict));
        md.push_str(&format!("* **Total Pengujian:** {}\n", self.total_checks));
        md.push_str(&format!("* **Pengujian Lolos:** {} ({}%)\n", self.passed_checks, pass_pct));
        md.push_str(&format!("* **Pengujian Gagal:** {}\n\n", self.failed_checks));
        md.push_str("| ID | Kategori | Tingkat Keparahan | Invariant | Status | Nama Pengujian |\n");
        md.push_str("| :--- | :--- | :---: | :--- | :---: | :--- |\n");

        for res in &self.results {
            md.push_str(&format!(
                "| **{}** | {} | {} | `{}` | **{}** | {} |\n",
                res.id,
                res.category.as_str(),
                res.severity.as_str(),
                res.invariant,
                res.status.as_str(),
                res.name
            ));
        }

        md
    }
}
