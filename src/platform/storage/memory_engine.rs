//! Implementasi in-memory StateStore untuk pengetesan cepat tanpa dependensi berkas disk.

use std::collections::HashMap;
use std::sync::RwLock;

use crate::consensus::block::Block;
use crate::consensus::certificate::CommitCertificate;
use crate::core::{Address, Hash256};
use crate::state::Account;
use crate::storage::traits::{StateStore, StorageError};

#[derive(Default)]
pub struct MemoryStorageEngine {
    accounts: RwLock<HashMap<Address, Account>>,
    blocks_by_height: RwLock<HashMap<u64, Block>>,
    blocks_by_hash: RwLock<HashMap<Hash256, u64>>,
    certificates: RwLock<HashMap<u64, CommitCertificate>>,
    metadata: RwLock<HashMap<String, Vec<u8>>>,
}

impl MemoryStorageEngine {
    pub fn new() -> Self {
        Self::default()
    }
}

impl StateStore for MemoryStorageEngine {
    fn get_account(&self, address: &Address) -> Result<Option<Account>, StorageError> {
        let accs = self.accounts.read().unwrap();
        Ok(accs.get(address).cloned())
    }

    fn get_all_accounts(&self) -> Result<HashMap<Address, Account>, StorageError> {
        let accs = self.accounts.read().unwrap();
        Ok(accs.clone())
    }

    fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, StorageError> {
        let blocks = self.blocks_by_height.read().unwrap();
        Ok(blocks.get(&height).cloned())
    }

    fn get_block_by_hash(&self, hash: &Hash256) -> Result<Option<Block>, StorageError> {
        let hashes = self.blocks_by_hash.read().unwrap();
        if let Some(height) = hashes.get(hash) {
            self.get_block_by_height(*height)
        } else {
            Ok(None)
        }
    }

    fn get_certificate(&self, height: u64) -> Result<Option<CommitCertificate>, StorageError> {
        let certs = self.certificates.read().unwrap();
        Ok(certs.get(&height).cloned())
    }

    fn get_latest_height(&self) -> Result<Option<u64>, StorageError> {
        let meta = self.metadata.read().unwrap();
        if let Some(val) = meta.get("latest_height") {
            if val.len() == 8 {
                let mut raw = [0u8; 8];
                raw.copy_from_slice(val);
                Ok(Some(u64::from_be_bytes(raw)))
            } else {
                Err(StorageError::Corruption("Invalid latest_height byte length".to_string()))
            }
        } else {
            Ok(None)
        }
    }

    fn get_metadata(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let meta = self.metadata.read().unwrap();
        Ok(meta.get(key).cloned())
    }

    fn commit_block_atomic(
        &self,
        block: &Block,
        certificate: &CommitCertificate,
        updated_accounts: &[(Address, Account)],
    ) -> Result<(), StorageError> {
        let mut accs = self.accounts.write().unwrap();
        let mut blocks = self.blocks_by_height.write().unwrap();
        let mut hashes = self.blocks_by_hash.write().unwrap();
        let mut certs = self.certificates.write().unwrap();
        let mut meta = self.metadata.write().unwrap();

        for (addr, acc) in updated_accounts {
            accs.insert(*addr, acc.clone());
        }

        blocks.insert(block.header.height, block.clone());
        hashes.insert(block.hash(), block.header.height);
        certs.insert(block.header.height, certificate.clone());
        meta.insert("latest_height".to_string(), block.header.height.to_be_bytes().to_vec());

        Ok(())
    }
}
