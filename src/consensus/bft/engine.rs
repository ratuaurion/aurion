#![forbid(unsafe_code)]

//! Mesin Konsensus BFT Single-Slot Finality Aurion.
//! Mengatur pemilihan proposer, perakitan proposal blok, voting dua fase,
//! pembentukan sertifikat komitmen kuorum, dan finalisasi state ke ledger.

use crate::consensus::block::Block;
use crate::consensus::certificate::{CertificateError, CommitCertificate, ValidatorSet};
use crate::consensus::header::BlockHeader;
use crate::consensus::vote::{Vote, VoteError, PHASE_PRECOMMIT, PHASE_PREVOTE};
use crate::core::{Address, Hash256};
use crate::crypto::{blake3_hash, Keypair};
use crate::mempool::MempoolEngine;
use crate::state::chain::{ChainError, ChainLedger};
use crate::state::smt::compute_accounts_state_root;
use crate::state::stf::apply_transaction;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BftEngineError {
    #[error("Node is not configured as an active validator: missing keypair or index")]
    NotAValidator,
    #[error("Ledger execution error: {0}")]
    Ledger(#[from] ChainError),
    #[error("Commit certificate error: {0}")]
    Certificate(#[from] CertificateError),
    #[error("Vote signing or verification error: {0}")]
    Vote(#[from] VoteError),
}

/// Mesin konsensus BFT Aurion.
pub struct BftEngine {
    pub validator_keypair: Option<Keypair>,
    pub validator_index: Option<u32>,
}

impl BftEngine {
    /// Buat instance BftEngine baru.
    pub fn new(validator_keypair: Option<Keypair>, validator_index: Option<u32>) -> Self {
        Self {
            validator_keypair,
            validator_index,
        }
    }

    /// Apakah simpul ini memiliki kredensial validator aktif.
    #[inline]
    pub fn is_validator(&self) -> bool {
        self.validator_keypair.is_some() && self.validator_index.is_some()
    }

    /// Pemilihan Proposer Deterministik untuk (Height, Round).
    /// Menggunakan seed Blake3(prev_hash || height || round).
    pub fn select_proposer(
        val_set: &ValidatorSet,
        height: u64,
        round: u64,
        prev_hash: &Hash256,
    ) -> u32 {
        if val_set.validators.is_empty() {
            return 0;
        }

        let mut seed_input = Vec::with_capacity(32 + 8 + 8);
        seed_input.extend_from_slice(prev_hash.as_bytes());
        seed_input.extend_from_slice(&height.to_be_bytes());
        seed_input.extend_from_slice(&round.to_be_bytes());
        let seed = blake3_hash(&seed_input);

        let mut seed_u64_bytes = [0u8; 8];
        seed_u64_bytes.copy_from_slice(&seed.as_bytes()[..8]);
        let seed_u64 = u64::from_be_bytes(seed_u64_bytes);

        (seed_u64 % val_set.validators.len() as u64) as u32
    }

    /// Merakit proposal blok baru dari transaksi valid di mempool.
    pub fn assemble_block_proposal(
        &self,
        ledger: &ChainLedger,
        mempool: &MempoolEngine,
        round: u64,
        timestamp: u64,
        miner: &Address,
        max_payload_bytes: usize,
    ) -> Block {
        let candidates = mempool.pack_block_candidate(max_payload_bytes);

        // Dry-run STF untuk memfilter transaksi yang benar-benar sah dieksekusi
        let mut dry_run_accounts = ledger.accounts.clone();
        let mut dry_run_monetary = ledger.monetary.clone();
        let mut valid_txs = Vec::with_capacity(candidates.len());

        for tx in candidates {
            if apply_transaction(&mut dry_run_accounts, &mut dry_run_monetary, miner, &tx).is_ok() {
                valid_txs.push(tx);
            }
        }

        let tx_merkle_root = Block::calculate_tx_merkle_root(&valid_txs);
        let state_root = compute_accounts_state_root(&dry_run_accounts);

        let prev_block = ledger.latest_block();
        let header = BlockHeader {
            version: 1,
            height: prev_block.height() + 1,
            round,
            timestamp: std::cmp::max(timestamp, prev_block.header.timestamp + 1),
            prev_block_hash: prev_block.hash(),
            tx_merkle_root,
            state_root,
        };

        Block::new(header, valid_txs, None)
    }

    /// Menghasilkan suara Fase 1: Prevote untuk kandidat blok.
    pub fn produce_prevote(
        &self,
        block_hash: Hash256,
        height: u64,
        round: u64,
    ) -> Result<Vote, BftEngineError> {
        let keypair = self
            .validator_keypair
            .as_ref()
            .ok_or(BftEngineError::NotAValidator)?;
        let index = self.validator_index.ok_or(BftEngineError::NotAValidator)?;

        Vote::new_signed(keypair, PHASE_PREVOTE, height, round, block_hash, index)
            .map_err(BftEngineError::Vote)
    }

    /// Menghasilkan suara Fase 2: Precommit setelah melihat kuorum Prevote (Polka).
    pub fn produce_precommit(
        &self,
        block_hash: Hash256,
        height: u64,
        round: u64,
    ) -> Result<Vote, BftEngineError> {
        let keypair = self
            .validator_keypair
            .as_ref()
            .ok_or(BftEngineError::NotAValidator)?;
        let index = self.validator_index.ok_or(BftEngineError::NotAValidator)?;

        Vote::new_signed(keypair, PHASE_PRECOMMIT, height, round, block_hash, index)
            .map_err(BftEngineError::Vote)
    }

    /// Mengagregasi suara Precommit menjadi CommitCertificate yang sah.
    pub fn create_commit_certificate(
        &self,
        val_set: &ValidatorSet,
        block_hash: Hash256,
        height: u64,
        round: u64,
        precommits: Vec<Vote>,
    ) -> Result<CommitCertificate, BftEngineError> {
        let cert = CommitCertificate {
            block_hash,
            height,
            round,
            precommits,
        };
        cert.verify(val_set)?;
        Ok(cert)
    }

    /// Memfinalisasi blok dengan sertifikat komitmen dan menerapkannya ke ledger.
    pub fn commit_block(
        &self,
        ledger: &mut ChainLedger,
        mempool: &mut MempoolEngine,
        mut block: Block,
        cert: CommitCertificate,
        miner: &Address,
    ) -> Result<(), BftEngineError> {
        block.commit_certificate = Some(cert);
        ledger.apply_block(block.clone(), miner)?;

        // Bersihkan transaksi yang difinalisasi dari mempool
        let tx_ids: Vec<Hash256> = block.transactions.iter().map(|tx| tx.compute_tx_id()).collect();
        mempool.remove_finalized(&tx_ids);

        Ok(())
    }
}
