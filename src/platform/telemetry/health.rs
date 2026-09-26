#![forbid(unsafe_code)]

//! Modul Pemeriksaan Kesehatan (Health Checks) Ringan & Mendalam Aurion.
//! Menyediakan diagnosa liveness (`/healthz`) dan readiness (`/healthz/deep`).

use crate::runtime::config::NodeRole;
use serde::{Deserialize, Serialize};

/// Laporan status kesehatan ringan (Shallow Liveness).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShallowHealthReport {
    pub status: String,
    pub service: String,
    pub chain_id: u32,
    pub current_height: u64,
    pub role: String,
    pub connected_peers: usize,
    pub sync_state: String,
}

/// Status komponen individual untuk diagnosa mendalam.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub healthy: bool,
    pub message: String,
}

/// Laporan status kesehatan mendalam (Deep Readiness).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeepHealthReport {
    pub status: String, // "READY" | "DEGRADED"
    pub service: String,
    pub chain_id: u32,
    pub current_height: u64,
    pub components: Vec<ComponentHealth>,
    pub timestamp_epoch_s: u64,
}

/// Reporter kesehatan simpul terpadu.
#[derive(Debug, Clone)]
pub struct HealthReporter {
    pub chain_id: u32,
    pub role: NodeRole,
    pub min_peer_threshold: usize,
}

impl HealthReporter {
    pub fn new(chain_id: u32, role: NodeRole, min_peer_threshold: usize) -> Self {
        Self {
            chain_id,
            role,
            min_peer_threshold,
        }
    }

    /// Evaluasi liveness ringan (HTTP 200 jika node running).
    pub fn shallow_check(
        &self,
        current_height: u64,
        connected_peers: usize,
        is_synced: bool,
    ) -> ShallowHealthReport {
        ShallowHealthReport {
            status: "HEALTHY".to_string(),
            service: "aurion-node".to_string(),
            chain_id: self.chain_id,
            current_height,
            role: format!("{:?}", self.role),
            connected_peers,
            sync_state: if is_synced {
                "SYNCED".to_string()
            } else {
                "CATCHING_UP".to_string()
            },
        }
    }

    /// Evaluasi readiness mendalam memeriksa seluruh subsistem inti.
    pub fn deep_check(
        &self,
        current_height: u64,
        connected_peers: usize,
        sync_lag: u64,
        storage_healthy: bool,
        mempool_healthy: bool,
        consensus_healthy: bool,
    ) -> DeepHealthReport {
        let mut components = Vec::with_capacity(5);

        // 1. Storage ACID check
        components.push(ComponentHealth {
            name: "storage_redb_acid".to_string(),
            healthy: storage_healthy,
            message: if storage_healthy {
                "Storage engine online and accessible".to_string()
            } else {
                "Storage engine read/write error detected".to_string()
            },
        });

        // 2. Peer threshold check
        let peer_ok = connected_peers >= self.min_peer_threshold || self.min_peer_threshold == 0;
        components.push(ComponentHealth {
            name: "p2p_mesh_connectivity".to_string(),
            healthy: peer_ok,
            message: if peer_ok {
                format!(
                    "Peer count ({}) satisfies threshold ({})",
                    connected_peers, self.min_peer_threshold
                )
            } else {
                format!(
                    "Insufficient peers: {} < threshold {}",
                    connected_peers, self.min_peer_threshold
                )
            },
        });

        // 3. Sync lag check
        let sync_ok = sync_lag <= 1;
        components.push(ComponentHealth {
            name: "block_synchronization".to_string(),
            healthy: sync_ok,
            message: if sync_ok {
                format!("Node in sync (lag: {} blocks)", sync_lag)
            } else {
                format!("High sync lag: {} blocks behind canonical tip", sync_lag)
            },
        });

        // 4. Mempool integrity check
        components.push(ComponentHealth {
            name: "mempool_engine".to_string(),
            healthy: mempool_healthy,
            message: if mempool_healthy {
                "Mempool within capacity and admission parameters".to_string()
            } else {
                "Mempool saturated or admission queue stalled".to_string()
            },
        });

        // 5. Consensus engine check
        components.push(ComponentHealth {
            name: "bft_consensus_quorum".to_string(),
            healthy: consensus_healthy,
            message: if consensus_healthy {
                "BFT consensus engine active and responsive".to_string()
            } else {
                "BFT consensus round stalled or quorum unreachable".to_string()
            },
        });

        let all_healthy = components.iter().all(|c| c.healthy);
        let now_s = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        DeepHealthReport {
            status: if all_healthy {
                "READY".to_string()
            } else {
                "DEGRADED".to_string()
            },
            service: "aurion-node".to_string(),
            chain_id: self.chain_id,
            current_height,
            components,
            timestamp_epoch_s: now_s,
        }
    }
}
