#![forbid(unsafe_code)]

//! AurionNode: Daemon Simpul Blockchain Berdaulat Terpadu.
//! Menyatukan ChainLedger, MempoolEngine, BftEngine, SubscriptionManager, dan RpcServer.

use crate::consensus::bft::{
    publish_committed_block, BftEngine, BftReactor, BftTransport, BlockProposalEnvelope,
    CommitCertificate, ZenohBftObserver, ZenohBftTransport,
};
use crate::consensus::block::Block;
use crate::consensus::engine::BftEngineError;
use crate::core::Address;
use crate::crypto::Keypair;
use crate::gateway::rpc::methods::RpcContext;
use crate::gateway::rpc::pubsub::SubscriptionManager;
use crate::gateway::rpc::server::RpcServer;
use crate::genesis::builder::GenesisInitialization;
use crate::mempool::MempoolEngine;
use crate::runtime::config::NodeConfig;
use crate::state::chain::ChainLedger;
use std::collections::HashSet;
use std::io;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::watch;

#[derive(Debug, Error)]
pub enum NodeError {
    #[error("node is not configured as a validator")]
    NotValidator,
    #[error("node state lock is poisoned")]
    StateLockPoisoned,
    #[error("consensus transport initialization failed: {0}")]
    Transport(String),
}

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
    pub fn epoch_blocks(&self) -> u64 {
        self.config.epoch_blocks()
    }

    /// Inisialisasi simpul Aurion baru dari genesis state dan konfigurasi node (in-memory mode).
    pub fn new(
        config: NodeConfig,
        genesis: GenesisInitialization,
        validator_keypair: Option<Keypair>,
        validator_index: Option<u32>,
    ) -> Self {
        Self::new_with_optional_store(config, genesis, validator_keypair, validator_index, None)
    }

    /// Inisialisasi simpul Aurion baru dengan persistent StateStore (redb).
    pub fn new_with_store(
        config: NodeConfig,
        genesis: GenesisInitialization,
        validator_keypair: Option<Keypair>,
        validator_index: Option<u32>,
        store: Arc<dyn crate::storage::StateStore>,
    ) -> Self {
        Self::new_with_optional_store(
            config,
            genesis,
            validator_keypair,
            validator_index,
            Some(store),
        )
    }

    /// Inisialisasi simpul Mainnet resmi dengan StateStore persisten.
    /// Memuat transkrip seremoni kanonikal dan menyinkronkan ledger produksi.
    pub fn new_mainnet(
        config: NodeConfig,
        validator_keypair: Option<Keypair>,
        validator_index: Option<u32>,
        store: Arc<dyn crate::storage::StateStore>,
    ) -> Self {
        let genesis = crate::genesis::ceremony::CeremonyTranscript::canonical_mainnet_genesis();
        Self::new_with_store(config, genesis, validator_keypair, validator_index, store)
    }

    /// Inisialisasi internal simpul dengan atau tanpa StateStore.
    pub fn new_with_optional_store(
        config: NodeConfig,
        genesis: GenesisInitialization,
        validator_keypair: Option<Keypair>,
        validator_index: Option<u32>,
        store: Option<Arc<dyn crate::storage::StateStore>>,
    ) -> Self {
        let ledger_inst = if let Some(s) = store {
            ChainLedger::from_genesis_with_store(genesis, s)
                .expect("Failed to initialize or recover ChainLedger with store")
        } else {
            ChainLedger::from_genesis(genesis)
        };

        let ledger = Arc::new(Mutex::new(ledger_inst));
        let rpc_context = Arc::new(RpcContext::new(config.chain_id));
        let mempool = Arc::clone(&rpc_context.mempool);
        let pubsub = Arc::new(SubscriptionManager::new());

        // Sinkronisasi state awal / recovered ke rpc_context
        {
            let guard = ledger.lock().unwrap();
            let latest_h = guard.latest_height();
            rpc_context.current_height.store(latest_h, Ordering::SeqCst);
            rpc_context
                .finalized_height
                .store(latest_h, Ordering::SeqCst);
            *rpc_context.accounts.lock().unwrap() = guard.accounts.clone();
            for (h, b) in guard.blocks.iter().enumerate() {
                rpc_context
                    .headers
                    .lock()
                    .unwrap()
                    .insert(h as u64, b.header.clone());
            }
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

    /// Replays finalized peer blocks in order before consensus starts.
    ///
    /// The proposer address is not encoded in the canonical block, so recovery
    /// deterministically identifies it by checking each validator address
    /// against the block's committed state root.
    pub fn catch_up_from_peer(&self, peer: &AurionNode) -> Result<u64, NodeError> {
        let peer_blocks = peer
            .ledger
            .lock()
            .map_err(|_| NodeError::StateLockPoisoned)?
            .blocks
            .clone();
        let mut ledger = self
            .ledger
            .lock()
            .map_err(|_| NodeError::StateLockPoisoned)?;
        let start = ledger.latest_height() as usize + 1;
        let mut applied = 0;
        for block in peer_blocks.into_iter().skip(start) {
            let validators = ledger.validator_set.validators.clone();
            let mut committed = false;
            for validator in validators {
                if ledger
                    .validate_block_proposal(&block, &validator.validator_id)
                    .is_ok()
                    && ledger
                        .apply_block(block.clone(), &validator.validator_id)
                        .is_ok()
                {
                    committed = true;
                    applied += 1;
                    break;
                }
            }
            if !committed {
                return Err(NodeError::Transport(format!(
                    "peer block {} failed sequential catch-up validation",
                    block.height()
                )));
            }
        }
        drop(ledger);
        self.sync_rpc_context();
        Ok(applied)
    }

    /// Terapkan blok terkomit yang diterima dari jaringan BFT (mode observer).
    ///
    /// Blok divalidasi ulang STF-nya dan sertifikat kuorumnya diverifikasi oleh
    /// `apply_block`, sehingga Sentry tidak mempercayai peer secara buta.
    pub fn ingest_committed_block(&self, block: Block) -> Result<u64, NodeError> {
        let height = block.height();
        let hash = block.hash().to_hex();
        let already_applied = {
            let ledger = self
                .ledger
                .lock()
                .map_err(|_| NodeError::StateLockPoisoned)?;
            height <= ledger.latest_height()
        };
        if already_applied {
            return Ok(height);
        }
        let validators = self
            .ledger
            .lock()
            .map_err(|_| NodeError::StateLockPoisoned)?
            .validator_set
            .validators
            .clone();
        let mut ledger = self
            .ledger
            .lock()
            .map_err(|_| NodeError::StateLockPoisoned)?;
        let mut committed = false;
        for validator in validators {
            if ledger
                .validate_block_proposal(&block, &validator.validator_id)
                .is_ok()
                && ledger
                    .apply_block(block.clone(), &validator.validator_id)
                    .is_ok()
            {
                committed = true;
                break;
            }
        }
        drop(ledger);
        if !committed {
            return Err(NodeError::Transport(format!(
                "committed block {height} failed sentry validation"
            )));
        }
        self.sync_rpc_context();
        println!("[AURION SENTRY] Ingested committed block height #{height} (hash: {hash})");
        Ok(height)
    }

    /// Loop ingress blok terkomit dari observer non-voting hingga shutdown.
    pub fn spawn_observer_ingress(
        self: Arc<Self>,
        mut observer: ZenohBftObserver,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                if *shutdown_rx.borrow() {
                    break;
                }
                tokio::select! {
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    block = observer.recv() => {
                        match block {
                            Ok(block) => {
                                if let Err(error) = self.ingest_committed_block(block) {
                                    eprintln!(
                                        "[AURION SENTRY] committed block ingestion rejected: {error}"
                                    );
                                }
                            }
                            Err(error) => {
                                eprintln!("[AURION SENTRY] committed block ingress error: {error}");
                            }
                        }
                    }
                }
            }
        })
    }

    pub async fn spawn_consensus_engine(
        self: Arc<Self>,
        validator_index: u32,
        signing_keypair: Keypair,
        zenoh_session: Arc<zenoh::Session>,
    ) -> Result<tokio::task::JoinHandle<()>, NodeError> {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let inner = self
            .spawn_consensus_engine_with_shutdown(
                validator_index,
                signing_keypair,
                zenoh_session,
                shutdown_rx,
            )
            .await?;
        Ok(tokio::spawn(async move {
            let _shutdown_guard = shutdown_tx;
            let _ = inner.await;
        }))
    }

    pub async fn spawn_consensus_engine_with_shutdown(
        self: Arc<Self>,
        validator_index: u32,
        signing_keypair: Keypair,
        zenoh_session: Arc<zenoh::Session>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> Result<tokio::task::JoinHandle<()>, NodeError> {
        let validator_set = self
            .ledger
            .lock()
            .map_err(|_| NodeError::StateLockPoisoned)?
            .validator_set
            .clone();
        let publish_session = Arc::clone(&zenoh_session);
        let transport =
            ZenohBftTransport::from_session(zenoh_session, validator_index, self.config.chain_id)
                .await
                .map_err(|error| NodeError::Transport(error.to_string()))?;
        let ledger = Arc::clone(&self.ledger);
        let node = Arc::clone(&self);
        let proposal_keypair = signing_keypair.clone();
        let mut reactor = BftReactor::new_with_ledger(
            validator_index,
            signing_keypair,
            validator_set.clone(),
            transport,
            ledger,
            std::time::Duration::from_secs(1),
        );

        Ok(tokio::spawn(async move {
            let mut proposed = None;
            let mut gossiped_transactions = HashSet::new();
            loop {
                if *shutdown_rx.borrow() {
                    break;
                }
                if proposed != Some((reactor.current_height, reactor.current_round))
                    && BftEngine::select_proposer(
                        &validator_set,
                        reactor.current_height,
                        reactor.current_round,
                        &node
                            .ledger
                            .lock()
                            .map(|ledger| ledger.latest_block().hash())
                            .unwrap_or_default(),
                    ) == validator_index
                {
                    let proposal = {
                        let ledger = match node.ledger.lock() {
                            Ok(ledger) => ledger,
                            Err(_) => break,
                        };
                        let mempool = match node.mempool.lock() {
                            Ok(mempool) => mempool,
                            Err(_) => break,
                        };
                        let miner = match validator_set.get_validator(validator_index) {
                            Some(entry) => entry.validator_id,
                            None => break,
                        };
                        let block =
                            BftEngine::new(Some(proposal_keypair.clone()), Some(validator_index))
                                .assemble_block_proposal(
                                    &ledger,
                                    &mempool,
                                    reactor.current_round,
                                    SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .map(|duration| duration.as_secs())
                                        .unwrap_or(ledger.latest_block().header.timestamp + 1),
                                    &miner,
                                    1024 * 1024,
                                );
                        BlockProposalEnvelope::new_signed(
                            block,
                            node.config.chain_id,
                            validator_index,
                            &proposal_keypair,
                        )
                    };
                    if let Err(error) = reactor.propose_local(proposal.clone()).await {
                        eprintln!(
                            "[AURION CONSENSUS] validator {validator_index} local proposal rejected: {error}"
                        );
                        break;
                    }
                    if let Err(error) = reactor.transport.broadcast_proposal(proposal).await {
                        eprintln!(
                            "[AURION CONSENSUS] validator {validator_index} proposal broadcast failed: {error}"
                        );
                        break;
                    }
                    proposed = Some((reactor.current_height, reactor.current_round));
                }

                let pending_transactions = {
                    let mempool = match node.mempool.lock() {
                        Ok(mempool) => mempool,
                        Err(_) => break,
                    };
                    mempool
                        .entries
                        .iter()
                        .filter(|(tx_id, _)| !gossiped_transactions.contains(*tx_id))
                        .map(|(tx_id, entry)| (*tx_id, entry.tx.clone(), entry.sender_pubkey))
                        .collect::<Vec<_>>()
                };
                for (tx_id, transaction, sender_pubkey) in pending_transactions {
                    if reactor
                        .transport
                        .broadcast_transaction(transaction, sender_pubkey)
                        .await
                        .is_ok()
                    {
                        gossiped_transactions.insert(tx_id);
                    }
                }

                tokio::select! {
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    result = reactor.step() => {
                        match result {
                            Ok(Some(_certificate)) => {
                                node.sync_rpc_context();
                                let committed_block = node
                                    .ledger
                                    .lock()
                                    .ok()
                                    .map(|ledger| ledger.latest_block().clone());
                                if let Some(block) = committed_block {
                                    let session = Arc::clone(&publish_session);
                                    let chain_id = node.config.chain_id;
                                    tokio::spawn(async move {
                                        if let Err(error) = publish_committed_block(
                                            &session,
                                            chain_id,
                                            validator_index,
                                            &block,
                                        )
                                        .await
                                        {
                                            eprintln!(
                                                "[AURION CONSENSUS] validator {validator_index} committed block publish failed: {error}"
                                            );
                                        }
                                    });
                                }
                                proposed = None;
                            }
                            Ok(None) => {}
                            Err(error) => {
                                eprintln!(
                                    "[AURION CONSENSUS] validator {validator_index} reactor stopped: {error}"
                                );
                                break;
                            }
                        }
                        for (transaction, sender_pubkey) in reactor.drain_transactions() {
                            let account = node
                                .ledger
                                .lock()
                                .ok()
                                .and_then(|ledger| ledger.accounts.get(&transaction.sender).cloned())
                                .unwrap_or_default();
                            if let Ok(mut mempool) = node.mempool.lock() {
                                let _ = mempool.submit_transaction(
                                    transaction,
                                    &sender_pubkey,
                                    SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .map(|duration| duration.as_secs())
                                        .unwrap_or_default(),
                                    &account,
                                );
                            }
                        }
                    }
                }
            }
        }))
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
        let cert_round = cert.round;
        let certificate_snapshot = cert.clone();
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
        self.rpc_context
            .certificates
            .lock()
            .unwrap()
            .insert(block_height, certificate_snapshot);

        // 4. Siarkan notifikasi real-time via WebSocket dan perbarui metrik
        self.rpc_context.metrics.record_block(
            block_height,
            cert_round,
            block.transactions.len() as u64,
            0,
            1,
        );
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
