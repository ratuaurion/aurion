#![forbid(unsafe_code)]

//! ChainLedger: Pengelola Rantai Blok, Silsilah Blok, dan Transisi State Atomik Aurion.
//! Mematuhi Konstitusi Konsensus dan Spesifikasi State Transition Aurion.

use crate::consensus::block::Block;
use crate::consensus::certificate::{CertificateError, ValidatorSet};
use crate::consensus::header::BlockHeader;
use crate::core::{Address, Hash256, Quantum};
use crate::genesis::builder::GenesisInitialization;
use crate::state::account::Account;
use crate::state::monetary::MonetaryState;
use crate::state::smt::compute_accounts_state_root;
use crate::state::stf::{apply_transaction, StateTransitionError};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ChainError {
    #[error("Invalid block height: expected {expected}, got {got}")]
    InvalidHeight { expected: u64, got: u64 },
    #[error("Invalid parent block hash: expected {expected}, got {got}")]
    InvalidParentHash { expected: Hash256, got: Hash256 },
    #[error("Invalid timestamp: must be strictly greater than previous block timestamp ({prev}), got {got}")]
    InvalidTimestamp { prev: u64, got: u64 },
    #[error("Transaction Merkle root verification failed")]
    InvalidTxMerkleRoot,
    #[error("State root verification failed: expected {expected}, got {got}")]
    InvalidStateRoot { expected: Hash256, got: Hash256 },
    #[error("Missing commit certificate on non-genesis block")]
    MissingCommitCertificate,
    #[error("Commit certificate height or hash does not match block")]
    MismatchedCertificate,
    #[error("Invalid commit certificate: {0}")]
    InvalidCommitCertificate(CertificateError),
    #[error("State transition error: {0}")]
    StateTransition(StateTransitionError),
}

/// Ledger kanonikal rantai blok Aurion.
pub struct ChainLedger {
    pub genesis_header: BlockHeader,
    pub blocks: Vec<Block>,
    pub block_by_hash: HashMap<Hash256, u64>,
    pub accounts: HashMap<Address, Account>,
    pub monetary: MonetaryState,
    pub validator_set: ValidatorSet,
}

impl ChainLedger {
    /// Inisialisasi ChainLedger baru dari GenesisInitialization.
    pub fn from_genesis(genesis: GenesisInitialization) -> Self {
        let genesis_block = Block::new(genesis.header.clone(), Vec::new(), None);
        let genesis_hash = genesis_block.hash();

        let mut block_by_hash = HashMap::new();
        block_by_hash.insert(genesis_hash, 0);

        Self {
            genesis_header: genesis.header,
            blocks: vec![genesis_block],
            block_by_hash,
            accounts: genesis.accounts,
            monetary: genesis.monetary,
            validator_set: genesis.validator_set,
        }
    }

    /// Ambil blok terakhir di rantai.
    pub fn latest_block(&self) -> &Block {
        self.blocks.last().expect("Chain cannot be empty")
    }

    /// Tinggi blok terbaru.
    pub fn latest_height(&self) -> u64 {
        self.latest_block().height()
    }

    /// Tinggi blok yang telah final. Pada Aurion-BFT Single-Slot Finality,
    /// setiap blok yang sah dikomit adalah final seketika.
    pub fn finalized_height(&self) -> u64 {
        self.latest_height()
    }

    /// Cari blok berdasarkan nomor tinggi.
    pub fn get_block_by_height(&self, height: u64) -> Option<&Block> {
        self.blocks.get(height as usize)
    }

    /// Cari blok berdasarkan hash.
    pub fn get_block_by_hash(&self, hash: &Hash256) -> Option<&Block> {
        self.block_by_hash
            .get(hash)
            .and_then(|h| self.get_block_by_height(*h))
    }

    /// Ambil akun berdasarkan alamat.
    pub fn get_account(&self, address: &Address) -> Option<&Account> {
        self.accounts.get(address)
    }

    /// Ambil saldo akun.
    pub fn get_balance(&self, address: &Address) -> Quantum {
        self.accounts
            .get(address)
            .map(|a| a.balance)
            .unwrap_or(Quantum::ZERO)
    }

    /// Ambil nonce akun.
    pub fn get_nonce(&self, address: &Address) -> u64 {
        self.accounts.get(address).map(|a| a.nonce).unwrap_or(0)
    }

    /// Hitung state root saat ini dari himpunan akun aktif.
    pub fn compute_current_state_root(&self) -> Hash256 {
        compute_accounts_state_root(&self.accounts)
    }

    /// Eksekusi dan komit blok baru ke dalam ledger secara atomik.
    pub fn apply_block(&mut self, block: Block, miner: &Address) -> Result<(), ChainError> {
        let prev_block = self.latest_block();
        let expected_height = prev_block.height() + 1;

        // 1. Validasi nomor tinggi
        if block.height() != expected_height {
            return Err(ChainError::InvalidHeight {
                expected: expected_height,
                got: block.height(),
            });
        }

        // 2. Validasi silsilah induk (Parent Hash)
        let expected_parent = prev_block.hash();
        if block.header.prev_block_hash != expected_parent {
            return Err(ChainError::InvalidParentHash {
                expected: expected_parent,
                got: block.header.prev_block_hash,
            });
        }

        // 3. Validasi timestamp monotonik
        if block.header.timestamp <= prev_block.header.timestamp {
            return Err(ChainError::InvalidTimestamp {
                prev: prev_block.header.timestamp,
                got: block.header.timestamp,
            });
        }

        // 4. Validasi Merkle root transaksi
        if !block.verify_tx_merkle_root() {
            return Err(ChainError::InvalidTxMerkleRoot);
        }

        // 5. Validasi Commit Certificate BFT
        let cert = block
            .commit_certificate
            .as_ref()
            .ok_or(ChainError::MissingCommitCertificate)?;

        if cert.height != block.height() || cert.block_hash != block.hash() {
            return Err(ChainError::MismatchedCertificate);
        }

        cert.verify(&self.validator_set)
            .map_err(ChainError::InvalidCommitCertificate)?;

        // 6. Eksekusi State Transition Function (STF) untuk semua transaksi
        let mut accounts_clone = self.accounts.clone();
        let mut monetary_clone = self.monetary.clone();

        for tx in &block.transactions {
            apply_transaction(&mut accounts_clone, &mut monetary_clone, miner, tx)
                .map_err(ChainError::StateTransition)?;
        }

        // 7. Validasi State Root setelah eksekusi seluruh transaksi
        let expected_state_root = compute_accounts_state_root(&accounts_clone);
        if block.header.state_root != expected_state_root {
            return Err(ChainError::InvalidStateRoot {
                expected: expected_state_root,
                got: block.header.state_root,
            });
        }

        // 8. Komit ke ledger (Semua validasi lolos 100%)
        let block_hash = block.hash();
        let block_height = block.height();

        self.accounts = accounts_clone;
        self.monetary = monetary_clone;
        self.block_by_hash.insert(block_hash, block_height);
        self.blocks.push(block);

        Ok(())
    }
}
