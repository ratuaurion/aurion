//! Trait abstraksi penyimpanan state dan blok Aurion.
//! Memisahkan semantik konsensus dan STF dari engine penyimpanan fisik.

use std::collections::HashMap;
use thiserror::Error;

use crate::codec::CodecError;
use crate::consensus::block::Block;
use crate::consensus::certificate::CommitCertificate;
use crate::core::{Address, Hash256};
use crate::state::Account;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Codec serialization error: {0}")]
    Codec(#[from] CodecError),

    #[error("Storage corruption detected: {0}")]
    Corruption(String),

    #[error("Item not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Trait abstraksi untuk penyimpanan state dan riwayat rantai blok Aurion.
pub trait StateStore: Send + Sync {
    /// Mengambil akun berdasarkan alamat.
    fn get_account(&self, address: &Address) -> Result<Option<Account>, StorageError>;

    /// Mengambil seluruh akun terkini (untuk verifikasi akar state SMT).
    fn get_all_accounts(&self) -> Result<HashMap<Address, Account>, StorageError>;

    /// Mengambil blok kanonikal berdasarkan tinggi rantai.
    fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, StorageError>;

    /// Mengambil blok kanonikal berdasarkan hash blok.
    fn get_block_by_hash(&self, hash: &Hash256) -> Result<Option<Block>, StorageError>;

    /// Mengambil sertifikat komit konsensus BFT berdasarkan tinggi rantai.
    fn get_certificate(&self, height: u64) -> Result<Option<CommitCertificate>, StorageError>;

    /// Mengambil tinggi rantai terbaru yang tersimpan.
    fn get_latest_height(&self) -> Result<Option<u64>, StorageError>;

    /// Mengambil nilai metadata arbitrary berdasarkan kunci.
    fn get_metadata(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;

    /// Menyimpan blok, sertifikat, dan akun yang diperbarui secara atomik.
    fn commit_block_atomic(
        &self,
        block: &Block,
        certificate: &CommitCertificate,
        updated_accounts: &[(Address, Account)],
    ) -> Result<(), StorageError>;
}
