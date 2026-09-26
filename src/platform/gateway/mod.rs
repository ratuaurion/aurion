//! Lapisan Gerbang Aplikasi Aurion (Gateway Layer).
//! Mematuhi Pilihan B: Pemisahan Dua Kamar.
//! Kamar ini melayani antarmuka dunia luar (JSON-RPC 2.0, WebSocket, Metrics, Health Check).

pub mod contract_decode;
pub mod explorer;
pub mod faucet;
pub mod rpc;

pub use contract_decode::{describe_contract_interaction, DecodeStatus, TxStatus};
pub use explorer::{
    render_block_by_height, render_explorer_stats, render_sandbox_html, render_tx_by_hash,
};
pub use faucet::{FaucetConfig, FaucetDispenser, FaucetError};
