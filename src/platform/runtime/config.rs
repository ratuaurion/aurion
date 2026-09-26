//! Konfigurasi runtime terpadu Aurion dan Topologi Sentry Node.
//! Mematuhi Dokumen 12 (12-OPERATIONAL-RULES.md).

use serde::{Deserialize, Serialize};

use crate::consensus::bft::resolve_epoch_blocks;
use crate::genesis::builder::GENESIS_CHAIN_ID;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    /// Simpul publik standar penerima lalu lintas P2P dan RPC.
    FullNode,
    /// Simpul penyaring DDoS dan proxy pembatas laju di depan validator.
    Sentry,
    /// Simpul konsensus inti terisolasi (RPC publik ditutup total).
    Validator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainnetBootstrapPeer {
    pub name: String,
    pub endpoint: String,
    pub public_key_hex: String,
}

/// Endpoint default Bootnode Resmi Aurion (VPS).
pub const OFFICIAL_MAINNET_BOOTNODE: &str = "tcp/116.212.72.89:7447";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub chain_id: u32,
    pub is_dev_mode: bool,
    pub role: NodeRole,
    pub p2p_bind: String,
    pub rpc_bind: String,
    pub metrics_bind: String,
    pub bootnode: Option<String>,
    pub sentry_peers: Vec<String>,
    pub max_tx_rate: u32,
    pub max_sync_rate: u32,
    pub min_peer_threshold: usize,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            chain_id: GENESIS_CHAIN_ID, // Canonical Mainnet Chain ID (single source of truth)
            is_dev_mode: false,
            role: NodeRole::FullNode,
            p2p_bind: "0.0.0.0:9000".to_string(),
            rpc_bind: "127.0.0.1:8545".to_string(),
            metrics_bind: "0.0.0.0:9100".to_string(),
            bootnode: None,
            sentry_peers: Vec::new(),
            max_tx_rate: 200,
            max_sync_rate: 50,
            min_peer_threshold: 3,
        }
    }
}

impl NodeConfig {
    pub fn new_validator(sentry_peers: Vec<String>) -> Self {
        Self {
            role: NodeRole::Validator,
            sentry_peers,
            ..Default::default()
        }
    }

    pub fn new_sentry(p2p_bind: String) -> Self {
        Self {
            role: NodeRole::Sentry,
            p2p_bind,
            ..Default::default()
        }
    }

    pub fn is_validator(&self) -> bool {
        self.role == NodeRole::Validator
    }

    pub fn is_sentry(&self) -> bool {
        self.role == NodeRole::Sentry
    }

    pub fn epoch_blocks(&self) -> u64 {
        resolve_epoch_blocks(self.is_dev_mode)
    }

    /// Daftar simpul bootstrap / genesis validator bootnodes resmi Mainnet.
    pub fn mainnet_bootnodes() -> Vec<MainnetBootstrapPeer> {
        vec![
            MainnetBootstrapPeer {
                name: "Official Sovereign Bootnode (VPS)".to_string(),
                endpoint: "116.212.72.89:7447".to_string(),
                public_key_hex: "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737"
                    .to_string(),
            },
            MainnetBootstrapPeer {
                name: "Genesis Validator 1 (Bootnode Alpha)".to_string(),
                endpoint: "seed1.aurion.network:9000".to_string(),
                public_key_hex: "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737"
                    .to_string(),
            },
            MainnetBootstrapPeer {
                name: "Genesis Validator 2 (Bootnode Beta)".to_string(),
                endpoint: "seed2.aurion.network:9000".to_string(),
                public_key_hex: "204040e364c10f2bec9c1fe500a1cd4c247c89d650a01ed7e82caba867877c21"
                    .to_string(),
            },
            MainnetBootstrapPeer {
                name: "Genesis Validator 3 (Bootnode Gamma)".to_string(),
                endpoint: "seed3.aurion.network:9000".to_string(),
                public_key_hex: "66cd608b928b88e50e0efeaa33faf1c43cefe07294b0b87e9fe0aba6a3cf7633"
                    .to_string(),
            },
            MainnetBootstrapPeer {
                name: "Genesis Validator 4 (Bootnode Delta)".to_string(),
                endpoint: "seed4.aurion.network:9000".to_string(),
                public_key_hex: "20828bf5c5bdcacb684863336c202fb5599da48be5596615742170705beca9f7"
                    .to_string(),
            },
        ]
    }
}
