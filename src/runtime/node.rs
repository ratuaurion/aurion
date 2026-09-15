#![forbid(unsafe_code)]

//! AurionNode: Daemon Simpul Blockchain Berdaulat Terpadu.
//! Menyatukan ChainLedger, MempoolEngine, BftEngine, SubscriptionManager, dan RpcServer.

use crate::consensus::block::Block;
use crate::consensus::certificate::CommitCertificate;
use crate::consensus::engine::{BftEngine, BftEngineError};
use crate::core::Address;
use crate::crypto::Keypair;
use crate::gateway::rpc::methods::RpcContext;
use crate::gateway::rpc::pubsub::SubscriptionManager;
use crate::gateway::rpc::server::RpcServer;
use crate::genesis::builder::GenesisInitialization;
use crate::mempool::MempoolEngine;
use crate::runtime::config::NodeConfig;
use crate::state::chain::ChainLedger;
use std::io;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

/// Simpul runtime terpadu Aurion.
pub struct AurionNode {
    pub config: NodeConfig,
    pub ledger: Arc<Mutex<ChainLedger>>,
    pub mempool: Arc<Mutex<MempoolEngine>>,
    pub pubsub: Arc<SubscriptionManager>,
    pub rpc_context: Arc<RpcContext>,
    pub bft_engine: Arc<Mutex<BftEngine>>,
}

impl AurionNode {
    /// Inisialisasi simpul Aurion baru dari genesis state dan konfigurasi node.
    pub fn new(
        config: NodeConfig,
        genesis: GenesisInitialization,
        validator_keypair: Option<Keypair>,
        validator_index: Option<u32>,
    ) -> Self {
        let ledger = Arc::new(Mutex::new(ChainLedger::from_genesis(genesis)));
        let rpc_context = Arc::new(RpcContext::new(config.chain_id));
        let mempool = Arc::clone(&rpc_context.mempool);
        let pubsub = Arc::new(SubscriptionManager::new());

        // Sinkronisasi state awal genesis ke rpc_context
        {
            let guard = ledger.lock().unwrap();
            rpc_context.current_height.store(0, Ordering::SeqCst);
            rpc_context.finalized_height.store(0, Ordering::SeqCst);
            *rpc_context.accounts.lock().unwrap() = guard.accounts.clone();
            rpc_context
                .headers
                .lock()
                .unwrap()
                .insert(0, guard.genesis_header.clone());
        }


        let bft_engine = Arc::new(Mutex::new(BftEngine::new(
            validator_keypair,
            validator_index,
        )));

        Self {
            config,
            ledger,
            mempool,
            pubsub,
            rpc_context,
            bft_engine,
        }
    }

    /// Apakah simpul beroperasi sebagai validator konsensus aktif.
    pub fn is_validator(&self) -> bool {
        self.bft_engine.lock().unwrap().is_validator()
    }

    /// Sinkronkan state terkini dari ledger ke rpc_context agar query RPC akurat 100%.
    pub fn sync_rpc_context(&self) {
        let ledger_guard = self.ledger.lock().unwrap();
        let latest_h = ledger_guard.latest_height();
        let finalized_h = ledger_guard.finalized_height();

        self.rpc_context
            .current_height
            .store(latest_h, Ordering::SeqCst);
        self.rpc_context
            .finalized_height
            .store(finalized_h, Ordering::SeqCst);

        *self.rpc_context.accounts.lock().unwrap() = ledger_guard.accounts.clone();

        let mut headers_guard = self.rpc_context.headers.lock().unwrap();
        for block in &ledger_guard.blocks {
            headers_guard.insert(block.height(), block.header.clone());
        }

        let mut certs_guard = self.rpc_context.certificates.lock().unwrap();
        for block in &ledger_guard.blocks {
            if let Some(cert) = &block.commit_certificate {
                certs_guard.insert(block.height(), cert.clone());
            }
        }
    }

    /// Produksi dan finalisasi blok berikutnya ke dalam ledger (untuk mode validator atau testing).
    pub fn produce_and_commit_block(
        &self,
        cert: CommitCertificate,
        miner: &Address,
        timestamp: u64,
    ) -> Result<Block, BftEngineError> {
        let mut ledger_guard = self.ledger.lock().unwrap();
        let mut mempool_guard = self.mempool.lock().unwrap();
        let bft_guard = self.bft_engine.lock().unwrap();

        // 1. Rakit proposal blok
        let block = bft_guard.assemble_block_proposal(
            &ledger_guard,
            &mempool_guard,
            cert.round,
            timestamp,
            miner,
            1024 * 1024,
        );

        // 2. Komit blok ke dalam ledger dan bersihkan mempool
        bft_guard.commit_block(
            &mut ledger_guard,
            &mut mempool_guard,
            block.clone(),
            cert,
            miner,
        )?;

        // 3. Perbarui rpc_context
        let block_height = block.height();
        let block_header = block.header.clone();
        let accounts_snapshot = ledger_guard.accounts.clone();
        drop(ledger_guard);
        drop(mempool_guard);
        drop(bft_guard);

        self.rpc_context
            .current_height
            .store(block_height, Ordering::SeqCst);
        self.rpc_context
            .finalized_height
            .store(block_height, Ordering::SeqCst);
        *self.rpc_context.accounts.lock().unwrap() = accounts_snapshot;
        self.rpc_context
            .headers
            .lock()
            .unwrap()
            .insert(block_height, block_header.clone());

        // 4. Siarkan notifikasi real-time via WebSocket
        self.pubsub.notify_new_head(&block_header);
        self.pubsub.notify_finalized_head(&block_header);

        Ok(block)
    }

    /// Menjalankan server JSON-RPC 2.0 dan WebSocket simpul.
    pub async fn run_rpc_server(
        &self,
        shutdown_rx: Option<watch::Receiver<bool>>,
    ) -> Result<(), io::Error> {
        let mut server = RpcServer::new(
            Arc::clone(&self.rpc_context),
            Arc::clone(&self.pubsub),
            &self.config.rpc_bind,
        );

        if let Some(rx) = shutdown_rx {
            server = server.with_shutdown(rx);
        }

        server.run().await
    }
}
