#![forbid(unsafe_code)]

//! Modul Metrik & Observabilitas Prometheus / OpenMetrics Aurion.
//! Menyediakan registri metrik performa deterministik berpresisi integer murni (Zero-Float).

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Registri metrik terpadu simpul Aurion yang mematuhi format Prometheus OpenMetrics.
#[derive(Debug)]
pub struct MetricsRegistry {
    pub chain_id: u64,
    pub block_height: AtomicU64,
    pub bft_round: AtomicU64,
    pub bft_validators_active: AtomicUsize,
    pub connected_peers: AtomicUsize,
    pub mempool_size: AtomicUsize,
    pub node_sync_status: AtomicU64, // 1 = Synced, 0 = Syncing
    pub transactions_processed_total: AtomicU64,
    pub burned_quanta_total: Arc<Mutex<u128>>,
    pub blocks_finalized_total: AtomicU64,
    pub bft_finality_latency_ms: AtomicU64,
    pub active_protocol_version: AtomicU32,
}

impl MetricsRegistry {
    pub fn new(chain_id: u64) -> Self {
        Self {
            chain_id,
            block_height: AtomicU64::new(0),
            bft_round: AtomicU64::new(0),
            bft_validators_active: AtomicUsize::new(4),
            connected_peers: AtomicUsize::new(0),
            mempool_size: AtomicUsize::new(0),
            node_sync_status: AtomicU64::new(1),
            transactions_processed_total: AtomicU64::new(0),
            burned_quanta_total: Arc::new(Mutex::new(0)),
            blocks_finalized_total: AtomicU64::new(0),
            bft_finality_latency_ms: AtomicU64::new(0),
            active_protocol_version: AtomicU32::new(1),
        }
    }

    /// Rekam mutasi blok baru pada metrik.
    pub fn record_block(&self, height: u64, round: u64, tx_count: u64, burned_quanta: u128, latency_ms: u64) {
        self.block_height.store(height, Ordering::SeqCst);
        self.bft_round.store(round, Ordering::SeqCst);
        self.transactions_processed_total.fetch_add(tx_count, Ordering::SeqCst);
        self.blocks_finalized_total.fetch_add(1, Ordering::SeqCst);
        self.bft_finality_latency_ms.store(latency_ms, Ordering::SeqCst);
        
        let mut burned = self.burned_quanta_total.lock().unwrap();
        *burned = burned.saturating_add(burned_quanta);
    }

    /// Rekam ukuran mempool terkini.
    pub fn set_mempool_size(&self, size: usize) {
        self.mempool_size.store(size, Ordering::SeqCst);
    }

    /// Rekam jumlah peer terhubung.
    pub fn set_connected_peers(&self, peers: usize) {
        self.connected_peers.store(peers, Ordering::SeqCst);
    }

    /// Format data metrik ke dalam representasi teks OpenMetrics / Prometheus.
    pub fn render_openmetrics(&self) -> String {
        let chain = self.chain_id;
        let height = self.block_height.load(Ordering::SeqCst);
        let round = self.bft_round.load(Ordering::SeqCst);
        let validators = self.bft_validators_active.load(Ordering::SeqCst);
        let peers = self.connected_peers.load(Ordering::SeqCst);
        let mempool = self.mempool_size.load(Ordering::SeqCst);
        let sync = self.node_sync_status.load(Ordering::SeqCst);
        let txs = self.transactions_processed_total.load(Ordering::SeqCst);
        let blocks = self.blocks_finalized_total.load(Ordering::SeqCst);
        let latency = self.bft_finality_latency_ms.load(Ordering::SeqCst);
        let version = self.active_protocol_version.load(Ordering::SeqCst);
        let burned = *self.burned_quanta_total.lock().unwrap();

        let mut out = String::with_capacity(2048);

        out.push_str("# HELP aurion_block_height Current canonical block height of the sovereign ledger.\n");
        out.push_str("# TYPE aurion_block_height gauge\n");
        out.push_str(&format!("aurion_block_height{{chain_id=\"{}\"}} {}\n\n", chain, height));

        out.push_str("# HELP aurion_bft_round Current BFT consensus round.\n");
        out.push_str("# TYPE aurion_bft_round gauge\n");
        out.push_str(&format!("aurion_bft_round{{chain_id=\"{}\"}} {}\n\n", chain, round));

        out.push_str("# HELP aurion_bft_validators_active Number of active consensus validators.\n");
        out.push_str("# TYPE aurion_bft_validators_active gauge\n");
        out.push_str(&format!("aurion_bft_validators_active{{chain_id=\"{}\"}} {}\n\n", chain, validators));

        out.push_str("# HELP aurion_connected_peers Number of active authenticated P2P peers.\n");
        out.push_str("# TYPE aurion_connected_peers gauge\n");
        out.push_str(&format!("aurion_connected_peers{{chain_id=\"{}\"}} {}\n\n", chain, peers));

        out.push_str("# HELP aurion_mempool_size Number of pending transactions currently in the mempool.\n");
        out.push_str("# TYPE aurion_mempool_size gauge\n");
        out.push_str(&format!("aurion_mempool_size{{chain_id=\"{}\"}} {}\n\n", chain, mempool));

        out.push_str("# HELP aurion_node_sync_status Node synchronization status (1 = synced, 0 = syncing).\n");
        out.push_str("# TYPE aurion_node_sync_status gauge\n");
        out.push_str(&format!("aurion_node_sync_status{{chain_id=\"{}\"}} {}\n\n", chain, sync));

        out.push_str("# HELP aurion_transactions_processed_total Total count of transactions processed and finalized.\n");
        out.push_str("# TYPE aurion_transactions_processed_total counter\n");
        out.push_str(&format!("aurion_transactions_processed_total{{chain_id=\"{}\"}} {}\n\n", chain, txs));

        out.push_str("# HELP aurion_blocks_finalized_total Total number of blocks committed to the ledger.\n");
        out.push_str("# TYPE aurion_blocks_finalized_total counter\n");
        out.push_str(&format!("aurion_blocks_finalized_total{{chain_id=\"{}\"}} {}\n\n", chain, blocks));

        out.push_str("# HELP aurion_burned_quanta_total Cumulative quanta permanently burned by the 20% protocol fee split.\n");
        out.push_str("# TYPE aurion_burned_quanta_total counter\n");
        out.push_str(&format!("aurion_burned_quanta_total{{chain_id=\"{}\"}} {}\n\n", chain, burned));

        out.push_str("# HELP aurion_bft_finality_latency_ms Single-slot BFT finality commit latency in milliseconds.\n");
        out.push_str("# TYPE aurion_bft_finality_latency_ms gauge\n");
        out.push_str(&format!("aurion_bft_finality_latency_ms{{chain_id=\"{}\"}} {}\n\n", chain, latency));

        out.push_str("# HELP aurion_active_protocol_version Active on-chain protocol version.\n");
        out.push_str("# TYPE aurion_active_protocol_version gauge\n");
        out.push_str(&format!("aurion_active_protocol_version{{chain_id=\"{}\"}} {}\n\n", chain, version));

        out.push_str("# EOF\n");
        out
    }

    /// Format snapshot metrik ke dalam representasi JSON.
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            chain_id: self.chain_id,
            block_height: self.block_height.load(Ordering::SeqCst),
            bft_round: self.bft_round.load(Ordering::SeqCst),
            bft_validators_active: self.bft_validators_active.load(Ordering::SeqCst),
            connected_peers: self.connected_peers.load(Ordering::SeqCst),
            mempool_size: self.mempool_size.load(Ordering::SeqCst),
            node_sync_status: self.node_sync_status.load(Ordering::SeqCst),
            transactions_processed_total: self.transactions_processed_total.load(Ordering::SeqCst),
            blocks_finalized_total: self.blocks_finalized_total.load(Ordering::SeqCst),
            burned_quanta_total: self.burned_quanta_total.lock().unwrap().to_string(),
            bft_finality_latency_ms: self.bft_finality_latency_ms.load(Ordering::SeqCst),
            active_protocol_version: self.active_protocol_version.load(Ordering::SeqCst),
        }
    }
}

/// DTO representasi snapshot metrik untuk output JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub chain_id: u64,
    pub block_height: u64,
    pub bft_round: u64,
    pub bft_validators_active: usize,
    pub connected_peers: usize,
    pub mempool_size: usize,
    pub node_sync_status: u64,
    pub transactions_processed_total: u64,
    pub blocks_finalized_total: u64,
    pub burned_quanta_total: String,
    pub bft_finality_latency_ms: u64,
    pub active_protocol_version: u32,
}
