//! Supervisor siklus hidup proses Aurion, Isolasi Sentry, dan Pemeriksaan Kesehatan (Health Checks).
//! Mematuhi Dokumen 12 (12-OPERATIONAL-RULES.md).

use crate::runtime::config::{NodeConfig, NodeRole};
use thiserror::Error;

pub const MAX_STATE_LATENCY_MS: u64 = 50;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HealthCheckError {
    #[error("Node process daemon is not active / running")]
    NotRunning,
    #[error("Insufficient active peers: expected >= {expected}, got {got}")]
    InsufficientPeers { expected: usize, got: usize },
    #[error("Node is actively synchronizing blocks: height lag {sync_lag} > 1")]
    SyncLagTooHigh { sync_lag: u64 },
    #[error("Database state read latency too high: {latency_ms}ms > {limit_ms}ms")]
    StateLatencyTooHigh { latency_ms: u64, limit_ms: u64 },
    #[error("Unauthorized connection attempt to private validator from {0}")]
    UnauthorizedValidatorAccess(String),
}

/// Supervisor siklus hidup simpul dan firewall jaringan sentry.
pub struct RuntimeSupervisor {
    pub config: NodeConfig,
    pub is_running: bool,
    pub active_peer_count: usize,
    pub sync_lag: u64,
    pub last_state_latency_ms: u64,
}

impl RuntimeSupervisor {
    pub fn new(config: NodeConfig) -> Self {
        Self {
            config,
            is_running: false,
            active_peer_count: 0,
            sync_lag: 0,
            last_state_latency_ms: 0,
        }
    }

    pub fn start(&mut self) {
        self.is_running = true;
    }

    pub fn stop(&mut self) {
        self.is_running = false;
    }

    /// Mandat Isolasi Validator: Memeriksa apakah koneksi peer diizinkan.
    /// Validator HANYA boleh menerima koneksi dari Sentry Nodes terdaftar.
    pub fn is_peer_allowed(&self, peer_locator: &str) -> bool {
        match self.config.role {
            NodeRole::Validator => self
                .config
                .sentry_peers
                .iter()
                .any(|sentry| sentry == peer_locator),
            NodeRole::Sentry | NodeRole::FullNode => true,
        }
    }

    /// Pemeriksaan Ringan (Shallow Liveness Check: `/healthz`).
    pub fn check_liveness(&self) -> bool {
        self.is_running
    }

    /// Pemeriksaan Mendalam (Deep Readiness Check: `/healthz/deep`).
    pub fn check_readiness(&self) -> Result<(), HealthCheckError> {
        if !self.is_running {
            return Err(HealthCheckError::NotRunning);
        }

        if self.active_peer_count < self.config.min_peer_threshold {
            return Err(HealthCheckError::InsufficientPeers {
                expected: self.config.min_peer_threshold,
                got: self.active_peer_count,
            });
        }

        if self.sync_lag > 1 {
            return Err(HealthCheckError::SyncLagTooHigh {
                sync_lag: self.sync_lag,
            });
        }

        if self.last_state_latency_ms > MAX_STATE_LATENCY_MS {
            return Err(HealthCheckError::StateLatencyTooHigh {
                latency_ms: self.last_state_latency_ms,
                limit_ms: MAX_STATE_LATENCY_MS,
            });
        }

        Ok(())
    }
}
