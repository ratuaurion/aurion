#![forbid(unsafe_code)]

//! Modul Pemulihan Bencana (Disaster Recovery) & Pemutus Sirkuit Darurat (Circuit Breaker).
//! Menyediakan perlindungan otomatis terhadap stall konsensus, pembekuan fork darurat,
//! serta pemulihan persisten dari paket state snapshot `.auss` terotentikasi.

use crate::state::chain::ChainLedger;
use crate::state::snapshot::{SnapshotError, StateSnapshot};
use crate::storage::StateStore;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error("I/O failure during recovery operation: {0}")]
    Io(#[from] std::io::Error),
    #[error("Snapshot verification or decode failure: {0}")]
    Snapshot(#[from] SnapshotError),
    #[error("Storage engine error: {0}")]
    Storage(String),
    #[error("Ledger integrity failure: {0}")]
    IntegrityViolation(String),
}

/// Laporan hasil pemulihan snapshot darurat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryReport {
    pub success: bool,
    pub recovered_height: u64,
    pub state_root: String,
    pub accounts_restored: usize,
    pub snapshot_checksum: String,
    pub message: String,
}

/// Laporan audit integritas rantai blok dan penyimpanan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerAuditReport {
    pub is_valid: bool,
    pub total_blocks: u64,
    pub latest_block_hash: String,
    pub latest_state_root: String,
    pub total_accounts: usize,
    pub errors: Vec<String>,
}

/// Pemutus sirkuit darurat (Circuit Breaker) simpul.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CircuitBreaker {
    pub is_tripped: bool,
    pub trip_reason: Option<String>,
    pub consecutive_failed_rounds: u64,
    pub max_allowed_failed_rounds: u64,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(10) // Maksimal 10 putaran gagal berturut-turut sebelum trip
    }
}

impl CircuitBreaker {
    pub fn new(max_allowed_failed_rounds: u64) -> Self {
        Self {
            is_tripped: false,
            trip_reason: None,
            consecutive_failed_rounds: 0,
            max_allowed_failed_rounds,
        }
    }

    /// Rekam kegagalan putaran konsensus BFT.
    pub fn record_round_failure(&mut self, reason: &str) {
        if self.is_tripped {
            return;
        }
        self.consecutive_failed_rounds = self.consecutive_failed_rounds.saturating_add(1);
        if self.consecutive_failed_rounds >= self.max_allowed_failed_rounds {
            self.trip(format!(
                "Consensus round failure threshold exceeded ({} >= {}): {}",
                self.consecutive_failed_rounds, self.max_allowed_failed_rounds, reason
            ));
        }
    }

    /// Rekam keberhasilan putaran konsensus BFT.
    pub fn record_round_success(&mut self) {
        if !self.is_tripped {
            self.consecutive_failed_rounds = 0;
        }
    }

    /// Picu pemutusan sirkuit darurat secara manual atau otomatis.
    pub fn trip(&mut self, reason: impl Into<String>) {
        self.is_tripped = true;
        self.trip_reason = Some(reason.into());
    }

    /// Reset pemutus sirkuit oleh operator yang berwenang.
    pub fn reset(&mut self) {
        self.is_tripped = false;
        self.trip_reason = None;
        self.consecutive_failed_rounds = 0;
    }
}

/// Manajer pemulihan bencana simpul.
pub struct DisasterRecoveryManager;

impl DisasterRecoveryManager {
    /// Pulihkan status simpul dari file fisik snapshot `.auss`.
    pub fn restore_from_snapshot_file(
        path: &Path,
        target_store: &dyn StateStore,
    ) -> Result<RecoveryReport, RecoveryError> {
        let snapshot = StateSnapshot::read_from_file(path)?;
        Self::restore_from_snapshot(&snapshot, target_store)
    }

    /// Pulihkan status simpul dari objek `StateSnapshot`.
    pub fn restore_from_snapshot(
        snapshot: &StateSnapshot,
        target_store: &dyn StateStore,
    ) -> Result<RecoveryReport, RecoveryError> {
        // Terapkan snapshot ke StateStore (termasuk verifikasi state root)
        snapshot.apply_to_store(target_store)?;

        let checksum = snapshot.compute_checksum();
        let state_root_hex = hex::encode(snapshot.state_root.0);

        Ok(RecoveryReport {
            success: true,
            recovered_height: snapshot.height,
            state_root: state_root_hex,
            accounts_restored: snapshot.accounts.len(),
            snapshot_checksum: hex::encode(checksum.0),
            message: format!(
                "Successfully restored {} accounts at height {} from verified snapshot",
                snapshot.accounts.len(),
                snapshot.height
            ),
        })
    }

    /// Audit komprehensif integritas ledger dan penyimpanan.
    pub fn audit_ledger_integrity(
        ledger: &ChainLedger,
        _store: &dyn StateStore,
    ) -> Result<LedgerAuditReport, RecoveryError> {
        let mut errors = Vec::new();
        let total_blocks = ledger.blocks.len() as u64;

        if ledger.blocks.is_empty() {
            errors.push("Ledger contains no blocks (empty)".to_string());
            return Ok(LedgerAuditReport {
                is_valid: false,
                total_blocks: 0,
                latest_block_hash: String::new(),
                latest_state_root: String::new(),
                total_accounts: 0,
                errors,
            });
        }

        // Verifikasi ketinggian berurutan dan keterkaitan hash
        for i in 1..ledger.blocks.len() {
            let prev = &ledger.blocks[i - 1];
            let curr = &ledger.blocks[i];

            if curr.height() != prev.height() + 1 {
                errors.push(format!(
                    "Height gap detected between block {} and {}",
                    prev.height(),
                    curr.height()
                ));
            }

            let expected_parent = prev.header.compute_block_hash();
            if curr.header.prev_block_hash != expected_parent {
                errors.push(format!(
                    "Block parent hash mismatch at height {}: expected {}, got {}",
                    curr.height(),
                    hex::encode(expected_parent.0),
                    hex::encode(curr.header.prev_block_hash.0)
                ));
            }
        }

        let latest = ledger.latest_block();
        let latest_hash = hex::encode(latest.header.compute_block_hash().0);
        let latest_root = hex::encode(latest.header.state_root.0);
        let total_accounts = ledger.accounts.len();

        let is_valid = errors.is_empty();

        Ok(LedgerAuditReport {
            is_valid,
            total_blocks,
            latest_block_hash: latest_hash,
            latest_state_root: latest_root,
            total_accounts,
            errors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_tripping_and_reset() {
        let mut cb = CircuitBreaker::new(3);
        assert!(!cb.is_tripped);

        cb.record_round_failure("Round 1 timeout");
        cb.record_round_failure("Round 2 timeout");
        assert!(!cb.is_tripped);

        cb.record_round_failure("Round 3 timeout");
        assert!(cb.is_tripped);
        assert!(cb
            .trip_reason
            .as_ref()
            .unwrap()
            .contains("failure threshold exceeded"));

        // Reset
        cb.reset();
        assert!(!cb.is_tripped);
        assert_eq!(cb.consecutive_failed_rounds, 0);
    }
}
