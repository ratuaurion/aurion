//! Implementasi engine penyimpanan fisik berbasis `redb` murni Rust.
//! Memenuhi standar ACID, MVCC, write-ahead logging (WAL), dan zero unsafe.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

use crate::codec::{CanonicalDecode, CanonicalEncode};
use crate::consensus::block::Block;
use crate::consensus::certificate::CommitCertificate;
use crate::core::{Address, Hash256};
use crate::state::Account;
use crate::storage::traits::{StateStore, StorageError};

const ACCOUNTS_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("accounts");
const BLOCKS_BY_HEIGHT: TableDefinition<u64, &[u8]> = TableDefinition::new("blocks_by_height");
const BLOCKS_BY_HASH: TableDefinition<&[u8], u64> = TableDefinition::new("blocks_by_hash");
const CERTIFICATES_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("certificates");
const METADATA_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("metadata");

#[derive(Clone)]
pub struct RedbStorageEngine {
    db: Arc<Database>,
}

impl RedbStorageEngine {
    /// Membuka database `redb` yang ada, atau membuatnya jika belum ada.
    pub fn open_or_create(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let is_existing_and_non_empty =
            p.exists() && std::fs::metadata(p).map(|m| m.len() > 0).unwrap_or(false);

        let db = if is_existing_and_non_empty {
            Database::open(p).map_err(|e| StorageError::Database(e.to_string()))?
        } else {
            Database::create(p).map_err(|e| StorageError::Database(e.to_string()))?
        };

        // Inisialisasi tabel kosong jika belum ada
        let write_txn = db
            .begin_write()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        {
            let _ = write_txn
                .open_table(ACCOUNTS_TABLE)
                .map_err(|e| StorageError::Database(e.to_string()))?;
            let _ = write_txn
                .open_table(BLOCKS_BY_HEIGHT)
                .map_err(|e| StorageError::Database(e.to_string()))?;
            let _ = write_txn
                .open_table(BLOCKS_BY_HASH)
                .map_err(|e| StorageError::Database(e.to_string()))?;
            let _ = write_txn
                .open_table(CERTIFICATES_TABLE)
                .map_err(|e| StorageError::Database(e.to_string()))?;
            let _ = write_txn
                .open_table(METADATA_TABLE)
                .map_err(|e| StorageError::Database(e.to_string()))?;
        }
        write_txn
            .commit()
            .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(Self { db: Arc::new(db) })
    }
}

impl StateStore for RedbStorageEngine {
    fn get_account(&self, address: &Address) -> Result<Option<Account>, StorageError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn
            .open_table(ACCOUNTS_TABLE)
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let result = table
            .get(address.as_bytes().as_slice())
            .map_err(|e| StorageError::Database(e.to_string()))?;

        match result {
            Some(entry) => {
                let bytes = entry.value();
                let mut cursor = 0;
                let account = Account::decode_canonical(bytes, &mut cursor)?;
                Ok(Some(account))
            }
            None => Ok(None),
        }
    }

    fn get_all_accounts(&self) -> Result<HashMap<Address, Account>, StorageError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn
            .open_table(ACCOUNTS_TABLE)
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let mut accounts = HashMap::new();

        for entry_res in table
            .iter()
            .map_err(|e| StorageError::Database(e.to_string()))?
        {
            let (k, v) = entry_res.map_err(|e| StorageError::Database(e.to_string()))?;
            let key_bytes = k.value();
            let val_bytes = v.value();
            if key_bytes.len() == 32 {
                let mut raw = [0u8; 32];
                raw.copy_from_slice(key_bytes);
                let addr = Address::from_bytes(raw);
                let mut cursor = 0;
                let account = Account::decode_canonical(val_bytes, &mut cursor)?;
                accounts.insert(addr, account);
            }
        }

        Ok(accounts)
    }

    fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, StorageError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn
            .open_table(BLOCKS_BY_HEIGHT)
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let result = table
            .get(height)
            .map_err(|e| StorageError::Database(e.to_string()))?;

        match result {
            Some(entry) => {
                let bytes = entry.value();
                let mut cursor = 0;
                let block = Block::decode_canonical(bytes, &mut cursor)?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    fn get_block_by_hash(&self, hash: &Hash256) -> Result<Option<Block>, StorageError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let hash_table = read_txn
            .open_table(BLOCKS_BY_HASH)
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let height_entry = hash_table
            .get(hash.as_bytes().as_slice())
            .map_err(|e| StorageError::Database(e.to_string()))?;

        match height_entry {
            Some(h) => self.get_block_by_height(h.value()),
            None => Ok(None),
        }
    }

    fn get_certificate(&self, height: u64) -> Result<Option<CommitCertificate>, StorageError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn
            .open_table(CERTIFICATES_TABLE)
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let result = table
            .get(height)
            .map_err(|e| StorageError::Database(e.to_string()))?;

        match result {
            Some(entry) => {
                let bytes = entry.value();
                let mut cursor = 0;
                let cert = CommitCertificate::decode_canonical(bytes, &mut cursor)?;
                Ok(Some(cert))
            }
            None => Ok(None),
        }
    }

    fn get_latest_height(&self) -> Result<Option<u64>, StorageError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn
            .open_table(METADATA_TABLE)
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let entry = table
            .get("latest_height")
            .map_err(|e| StorageError::Database(e.to_string()))?;

        match entry {
            Some(val) => {
                let bytes = val.value();
                if bytes.len() == 8 {
                    let mut raw = [0u8; 8];
                    raw.copy_from_slice(bytes);
                    Ok(Some(u64::from_be_bytes(raw)))
                } else {
                    Err(StorageError::Corruption(
                        "Invalid latest_height byte length".to_string(),
                    ))
                }
            }
            None => Ok(None),
        }
    }

    fn get_metadata(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let table = read_txn
            .open_table(METADATA_TABLE)
            .map_err(|e| StorageError::Database(e.to_string()))?;
        let entry = table
            .get(key)
            .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(entry.map(|v| v.value().to_vec()))
    }

    fn commit_block_atomic(
        &self,
        block: &Block,
        certificate: &CommitCertificate,
        updated_accounts: &[(Address, Account)],
    ) -> Result<(), StorageError> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| StorageError::Database(e.to_string()))?;

        // 1. Update accounts table
        {
            let mut acc_table = write_txn
                .open_table(ACCOUNTS_TABLE)
                .map_err(|e| StorageError::Database(e.to_string()))?;
            for (addr, account) in updated_accounts {
                let mut buf = Vec::new();
                account.encode_canonical(&mut buf);
                acc_table
                    .insert(addr.as_bytes().as_slice(), buf.as_slice())
                    .map_err(|e| StorageError::Database(e.to_string()))?;
            }
        }

        // 2. Insert canonical block by height
        {
            let mut block_table = write_txn
                .open_table(BLOCKS_BY_HEIGHT)
                .map_err(|e| StorageError::Database(e.to_string()))?;
            let mut block_buf = Vec::new();
            block.encode_canonical(&mut block_buf);
            block_table
                .insert(block.header.height, block_buf.as_slice())
                .map_err(|e| StorageError::Database(e.to_string()))?;
        }

        // 3. Insert block hash index
        {
            let mut hash_table = write_txn
                .open_table(BLOCKS_BY_HASH)
                .map_err(|e| StorageError::Database(e.to_string()))?;
            let block_hash = block.hash();
            hash_table
                .insert(block_hash.as_bytes().as_slice(), block.header.height)
                .map_err(|e| StorageError::Database(e.to_string()))?;
        }

        // 4. Insert commit certificate
        {
            let mut cert_table = write_txn
                .open_table(CERTIFICATES_TABLE)
                .map_err(|e| StorageError::Database(e.to_string()))?;
            let mut cert_buf = Vec::new();
            certificate.encode_canonical(&mut cert_buf);
            cert_table
                .insert(block.header.height, cert_buf.as_slice())
                .map_err(|e| StorageError::Database(e.to_string()))?;
        }

        // 5. Update latest_height in metadata
        {
            let mut meta_table = write_txn
                .open_table(METADATA_TABLE)
                .map_err(|e| StorageError::Database(e.to_string()))?;
            let height_bytes = block.header.height.to_be_bytes();
            meta_table
                .insert("latest_height", height_bytes.as_slice())
                .map_err(|e| StorageError::Database(e.to_string()))?;
            let block_hash = block.hash();
            meta_table
                .insert("latest_block_hash", block_hash.as_bytes().as_slice())
                .map_err(|e| StorageError::Database(e.to_string()))?;
            meta_table
                .insert("latest_state_root", &block.header.state_root.as_bytes()[..])
                .map_err(|e| StorageError::Database(e.to_string()))?;
        }

        // Atomic commit to disk via WAL
        write_txn
            .commit()
            .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }
}
