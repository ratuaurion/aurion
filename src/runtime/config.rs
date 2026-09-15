//! Konfigurasi runtime terpadu Aurion dan Topologi Sentry Node.
//! Mematuhi Dokumen 12 (12-OPERATIONAL-RULES.md).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    /// Simpul publik standar penerima lalu lintas P2P dan RPC.
    FullNode,
    /// Simpul penyaring DDoS dan proxy pembatas laju di depan validator.
    Sentry,
    /// Simpul konsensus inti terisolasi (RPC publik ditutup total).
    Validator,
}

#[derive(Debug, Clone)]
pub struct NodeConfig {
    pub chain_id: u64,
    pub role: NodeRole,
    pub p2p_bind: String,
    pub rpc_bind: String,
    pub metrics_bind: String,
    pub sentry_peers: Vec<String>,
    pub max_tx_rate: u32,
    pub max_sync_rate: u32,
    pub min_peer_threshold: usize,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            chain_id: 1, // Mainnet
            role: NodeRole::FullNode,
            p2p_bind: "0.0.0.0:9000".to_string(),
            rpc_bind: "127.0.0.1:8545".to_string(),
            metrics_bind: "0.0.0.0:9100".to_string(),
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
}
