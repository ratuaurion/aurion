//! Konfigurasi runtime terpadu Aurion.

pub struct NodeConfig {
    pub chain_id: u32,
    pub p2p_bind: String,
    pub rpc_bind: String,
    pub is_validator: bool,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            chain_id: 1001,
            p2p_bind: "0.0.0.0:9000".to_string(),
            rpc_bind: "127.0.0.1:8545".to_string(),
            is_validator: false,
        }
    }
}
