#![forbid(unsafe_code)]

//! ChainLedger: Pengelola Rantai Blok, Silsilah Blok, dan Transisi State Atomik Aurion.
//! Mematuhi Konstitusi Konsensus dan Spesifikasi State Transition Aurion.

use crate::consensus::block::Block;
use crate::consensus::certificate::{CertificateError, ValidatorSet};
use crate::consensus::header::BlockHeader;
use crate::core::{Address, Hash256, Quantum};
use crate::genesis::builder::GenesisInitialization;
use crate::state::account::Account;
use crate::state::monetary::{calculate_block_subsidy, MonetaryState};
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
    #[error("Storage engine error: {0}")]
    Storage(String),
}

/// Ledger kanonikal rantai blok Aurion.
pub struct ChainLedger {
    pub genesis_header: BlockHeader,
    pub blocks: Vec<Block>,
    pub block_by_hash: HashMap<Hash256, u64>,
    pub accounts: HashMap<Address, Account>,
    pub monetary: MonetaryState,
    pub validator_set: ValidatorSet,
    pub store: Option<std::sync::Arc<dyn crate::storage::StateStore>>,
}

impl ChainLedger {
    /// Inisialisasi ChainLedger baru dari GenesisInitialization tanpa storage persisten (in-memory).
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
            store: None,
        }
    }

    /// Inisialisasi ChainLedger dengan storage engine persisten (redb).
    /// Jika storage sudah berisi blok, pulihkan (recover) state secara otomatis dan verifikasi state root.
    pub fn from_genesis_with_store(
        genesis: GenesisInitialization,
        store: std::sync::Arc<dyn crate::storage::StateStore>,
    ) -> Result<Self, ChainError> {
        if let Some(latest_h) = store.get_latest_height().map_err(|e| ChainError::Storage(e.to_string()))? {
            // RECOVERY PATH: Muat state dan blok dari disk
            let accounts = store.get_all_accounts().map_err(|e| ChainError::Storage(e.to_string()))?;
            let latest_block = store
                .get_block_by_height(latest_h)
                .map_err(|e| ChainError::Storage(e.to_string()))?
                .ok_or_else(|| ChainError::Storage(format!("Blok pada tinggi {} hilang dari storage", latest_h)))?;

            let expected_hash = store
                .get_metadata("latest_block_hash")
                .map_err(|e| ChainError::Storage(e.to_string()))?
                .ok_or_else(|| ChainError::Storage("metadata latest_block_hash hilang".into()))?;
            let expected_hash: [u8; 32] = expected_hash
                .try_into()
                .map_err(|_| ChainError::Storage("metadata latest_block_hash invalid".into()))?;
            if crate::primitives::core::Hash256::from_bytes(expected_hash) != latest_block.hash() {
                return Err(ChainError::Storage("metadata latest_block_hash tidak cocok".into()));
            }

            let expected_root = store
                .get_metadata("latest_state_root")
                .map_err(|e| ChainError::Storage(e.to_string()))?
                .ok_or_else(|| ChainError::Storage("metadata latest_state_root hilang".into()))?;
            let expected_root: [u8; 32] = expected_root
                .try_into()
                .map_err(|_| ChainError::Storage("metadata latest_state_root invalid".into()))?;
            if crate::primitives::core::Hash256::from_bytes(expected_root) != latest_block.header.state_root {
                return Err(ChainError::Storage("metadata latest_state_root tidak cocok".into()));
            }

            if latest_h > 0 {
                let certificate = store
                    .get_certificate(latest_h)
                    .map_err(|e| ChainError::Storage(e.to_string()))?
                    .ok_or_else(|| ChainError::Storage("sertifikat blok terakhir hilang".into()))?;
                certificate
                    .verify(&genesis.validator_set)
                    .map_err(|e| ChainError::Storage(format!("sertifikat blok terakhir invalid: {}", e)))?;
                if certificate.block_hash != latest_block.hash() || certificate.height != latest_h {
                    return Err(ChainError::Storage("sertifikat tidak cocok dengan blok terakhir".into()));
                }
            }

            let computed_root = compute_accounts_state_root(&accounts);
            if computed_root != latest_block.header.state_root {
                return Err(ChainError::InvalidStateRoot {
                    expected: latest_block.header.state_root,
                    got: computed_root,
                });
            }

            let mut blocks = Vec::with_capacity((latest_h + 1) as usize);
            let mut block_by_hash = HashMap::new();
            for h in 0..=latest_h {
                if let Some(b) = store.get_block_by_height(h).map_err(|e| ChainError::Storage(e.to_string()))? {
                    block_by_hash.insert(b.hash(), h);
                    blocks.push(b);
                }
            }

            Ok(Self {
                genesis_header: genesis.header,
                blocks,
                block_by_hash,
                accounts,
                monetary: genesis.monetary,
                validator_set: genesis.validator_set,
                store: Some(store),
            })
        } else {
            // INITIALIZATION PATH: Storage kosong, komit Genesis blok secara atomik
            let genesis_block = Block::new(genesis.header.clone(), Vec::new(), None);
            let genesis_hash = genesis_block.hash();
            let mut block_by_hash = HashMap::new();
            block_by_hash.insert(genesis_hash, 0);

            let initial_accounts: Vec<(Address, Account)> =
                genesis.accounts.iter().map(|(k, v)| (*k, v.clone())).collect();
            let dummy_cert = crate::consensus::certificate::CommitCertificate {
                height: 0,
                round: 0,
                block_hash: genesis_hash,
                precommits: Vec::new(),
            };

            store
                .commit_block_atomic(&genesis_block, &dummy_cert, &initial_accounts)
                .map_err(|e| ChainError::Storage(e.to_string()))?;

            Ok(Self {
                genesis_header: genesis.header,
                blocks: vec![genesis_block],
                block_by_hash,
                accounts: genesis.accounts,
                monetary: genesis.monetary,
                validator_set: genesis.validator_set,
                store: Some(store),
            })
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

    /// Validate a proposal against the current ledger without mutating state.
    pub fn validate_block_proposal(
        &self,
        block: &Block,
        proposer: &Address,
    ) -> Result<(), ChainError> {
        let prev_block = self.latest_block();
        let expected_height = prev_block.height() + 1;
        if block.height() != expected_height {
            return Err(ChainError::InvalidHeight {
                expected: expected_height,
                got: block.height(),
            });
        }
        if block.header.prev_block_hash != prev_block.hash() {
            return Err(ChainError::InvalidParentHash {
                expected: prev_block.hash(),
                got: block.header.prev_block_hash,
            });
        }
        if block.header.timestamp <= prev_block.header.timestamp {
            return Err(ChainError::InvalidTimestamp {
                prev: prev_block.header.timestamp,
                got: block.header.timestamp,
            });
        }
        if !block.verify_tx_merkle_root() {
            return Err(ChainError::InvalidTxMerkleRoot);
        }

        let mut accounts = self.accounts.clone();
        let mut monetary = self.monetary.clone();
        let subsidy = calculate_block_subsidy(block.height());
        if !subsidy.is_zero() {
            monetary
                .apply_issuance(subsidy)
                .map_err(|e| ChainError::StateTransition(StateTransitionError::Monetary(e.to_string())))?;
            let proposer_account = accounts.entry(*proposer).or_default();
            proposer_account.balance = proposer_account
                .balance
                .checked_add(subsidy)
                .map_err(|e| ChainError::StateTransition(StateTransitionError::Monetary(e.to_string())))?;
        }
        for tx in &block.transactions {
            apply_transaction(&mut accounts, &mut monetary, proposer, tx)
                .map_err(ChainError::StateTransition)?;
        }
        let expected_state_root = compute_accounts_state_root(&accounts);
        if block.header.state_root != expected_state_root {
            return Err(ChainError::InvalidStateRoot {
                expected: expected_state_root,
                got: block.header.state_root,
            });
        }
        Ok(())
    }

    /// Eksekusi dan komit blok baru ke dalam ledger secara atomik.
    pub fn apply_block(&mut self, block: Block, proposer: &Address) -> Result<(), ChainError> {
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

        // 6. Eksekusi State Transition Function (STF)
        let mut accounts_clone = self.accounts.clone();
        let mut monetary_clone = self.monetary.clone();

        // 6a. Penerbitan hadiah blok kanonikal ke produser blok / proposer
        let height = block.height();
        let subsidy = calculate_block_subsidy(height);
        if !subsidy.is_zero() {
            monetary_clone
                .apply_issuance(subsidy)
                .map_err(|e| ChainError::StateTransition(StateTransitionError::Monetary(e.to_string())))?;

            let proposer_acct = accounts_clone.entry(*proposer).or_default();
            proposer_acct.balance = proposer_acct
                .balance
                .checked_add(subsidy)
                .map_err(|e| ChainError::StateTransition(StateTransitionError::Monetary(e.to_string())))?;
        }

        // 6b. Eksekusi transaksi di dalam blok (100% fee ke proposer)
        for tx in &block.transactions {
            apply_transaction(&mut accounts_clone, &mut monetary_clone, proposer, tx)
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

        // 8. Komit ke persistent storage secara atomik (jika storage engine aktif)
        if let Some(store) = &self.store {
            let updated_accounts: Vec<(Address, Account)> =
                accounts_clone.iter().map(|(k, v)| (*k, v.clone())).collect();
            store
                .commit_block_atomic(&block, cert, &updated_accounts)
                .map_err(|e| ChainError::Storage(e.to_string()))?;
        }

        // 9. Komit ke in-memory cache ledger (Semua validasi dan persistensi disk lolos 100%)
        let block_hash = block.hash();
        let block_height = block.height();

        self.accounts = accounts_clone;
        self.monetary = monetary_clone;
        self.block_by_hash.insert(block_hash, block_height);
        self.blocks.push(block);

        Ok(())
    }
}
