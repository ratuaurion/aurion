#![forbid(unsafe_code)]

//! Modul State Snapshot & Fast Sync Engine Aurion (NET-011).
//!
//! Mematuhi Invariant:
//! - AUR-ARCH-001: Single Sovereign Primary Executable (/bin/aurion).
//! - AUR-ARCH-005: Canonical Binary Serialization & Deterministic State Verification.
//! - AUR-ARCH-011: Absolute Zero Unsafe Code (#![forbid(unsafe_code)]).
//! - AUR-ARCH-012: Absolute Zero Float Arithmetic (Fixed Precision Quantum u128).

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use thiserror::Error;

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::consensus::bft::certificate::CommitCertificate;
use crate::core::{Address, Hash256};
use crate::crypto::blake3_hash;
use crate::platform::storage::traits::StateStore;
use crate::statemachine::state::account::Account;
use crate::statemachine::state::smt::compute_accounts_state_root;

pub const SNAPSHOT_MAGIC: [u8; 4] = *b"AUSS"; // Aurion State Snapshot
pub const SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("I/O error during snapshot operation: {0}")]
    Io(#[from] std::io::Error),
    #[error("Codec error: {0}")]
    Codec(#[from] CodecError),
    #[error("Invalid snapshot magic: expected AUSS, got {0:?}")]
    InvalidMagic([u8; 4]),
    #[error("Unsupported snapshot version: {0}")]
    UnsupportedVersion(u32),
    #[error("State root mismatch: header specifies {header_root}, computed {computed_root}")]
    StateRootMismatch {
        header_root: Hash256,
        computed_root: Hash256,
    },
    #[error("Block not found at height {0}")]
    BlockNotFound(u64),
    #[error("Storage engine error: {0}")]
    Storage(String),
}

/// Paket snapshot keadaan kanonikal untuk fast-sync dan upgrading simpul multi-region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSnapshot {
    pub magic: [u8; 4],
    pub version: u32,
    pub chain_id: u64,
    pub height: u64,
    pub epoch: u64,
    pub block_hash: Hash256,
    pub state_root: Hash256,
    pub accounts: Vec<(Address, Account)>,
    pub certificate: Option<CommitCertificate>,
}

impl StateSnapshot {
    /// Ekspor state snapshot dari database fisik `StateStore` pada tinggi blok tertentu.
    pub fn create_from_store(
        store: &dyn StateStore,
        height: u64,
        chain_id: u64,
        epoch: u64,
    ) -> Result<Self, SnapshotError> {
        let block = store
            .get_block_by_height(height)
            .map_err(|e| SnapshotError::Storage(e.to_string()))?
            .ok_or(SnapshotError::BlockNotFound(height))?;

        let certificate = store
            .get_certificate(height)
            .map_err(|e| SnapshotError::Storage(e.to_string()))?;

        let all_accounts = store
            .get_all_accounts()
            .map_err(|e| SnapshotError::Storage(e.to_string()))?;

        // Verifikasi keabsahan state root yang dihitung melawan block header
        let computed_root = compute_accounts_state_root(&all_accounts);
        if computed_root != block.header.state_root {
            return Err(SnapshotError::StateRootMismatch {
                header_root: block.header.state_root,
                computed_root,
            });
        }

        // Urutkan akun deterministik berdasarkan alamat untuk stabilitas encoding
        let mut sorted_accounts: Vec<(Address, Account)> = all_accounts.into_iter().collect();
        sorted_accounts.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

        Ok(Self {
            magic: SNAPSHOT_MAGIC,
            version: SNAPSHOT_VERSION,
            chain_id,
            height,
            epoch,
            block_hash: block.header.compute_block_hash(),
            state_root: block.header.state_root,
            accounts: sorted_accounts,
            certificate,
        })
    }

    /// Terapkan state snapshot ke instance `StateStore` baru.
    pub fn apply_to_store(&self, store: &dyn StateStore) -> Result<(), SnapshotError> {
        let accounts_map: std::collections::HashMap<Address, Account> =
            self.accounts.iter().cloned().collect();
        let computed_root = compute_accounts_state_root(&accounts_map);
        if computed_root != self.state_root {
            return Err(SnapshotError::StateRootMismatch {
                header_root: self.state_root,
                computed_root,
            });
        }

        // Terapkan seluruh akun ke database storage
        if let Some(cert) = &self.certificate {
            let dummy_block = crate::consensus::block::Block::new(
                crate::consensus::header::BlockHeader {
                    version: 1,
                    height: self.height,
                    round: 0,
                    timestamp: 1773534000,
                    prev_block_hash: Hash256::from_bytes([0u8; 32]),
                    tx_merkle_root: Hash256::from_bytes([0u8; 32]),
                    state_root: self.state_root,
                },
                Vec::new(),
                Some(cert.clone()),
            );
            store
                .commit_block_atomic(&dummy_block, cert, &self.accounts)
                .map_err(|e| SnapshotError::Storage(e.to_string()))?;
        }

        Ok(())
    }

    /// Menghitung checksum Blake3 256-bit dari seluruh muatan snapshot.
    pub fn compute_checksum(&self) -> Hash256 {
        let mut buf = Vec::new();
        self.encode_canonical(&mut buf);
        blake3_hash(&buf)
    }

    /// Tulis snapshot ke file fisik di disk.
    pub fn write_to_file(&self, path: &Path) -> Result<(), SnapshotError> {
        let mut buf = Vec::new();
        self.encode_canonical(&mut buf);
        let mut file = File::create(path)?;
        file.write_all(&buf)?;
        file.flush()?;
        Ok(())
    }

    /// Baca snapshot dari file fisik di disk.
    pub fn read_from_file(path: &Path) -> Result<Self, SnapshotError> {
        let mut file = File::open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Self::decode_canonical(&bytes)
    }
}

impl CanonicalEncode for StateSnapshot {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.magic);
        self.version.encode_canonical(buf);
        self.chain_id.encode_canonical(buf);
        self.height.encode_canonical(buf);
        self.epoch.encode_canonical(buf);
        self.block_hash.encode_canonical(buf);
        self.state_root.encode_canonical(buf);

        // Encode accounts
        (self.accounts.len() as u64).encode_canonical(buf);
        for (addr, acct) in &self.accounts {
            addr.encode_canonical(buf);
            acct.encode_canonical(buf);
        }

        // Encode certificate presence
        match &self.certificate {
            Some(cert) => {
                1u8.encode_canonical(buf);
                cert.block_hash.encode_canonical(buf);
                cert.height.encode_canonical(buf);
                cert.round.encode_canonical(buf);
                (cert.precommits.len() as u64).encode_canonical(buf);
                for vote in &cert.precommits {
                    vote.encode_canonical(buf);
                }
            }
            None => 0u8.encode_canonical(buf),
        }
    }
}

impl StateSnapshot {
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, SnapshotError> {
        let mut cursor = 0;
        if bytes.len() < 4 {
            return Err(SnapshotError::Codec(CodecError::UnexpectedEof {
                needed: 4,
                available: bytes.len(),
            }));
        }
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[0..4]);
        cursor += 4;

        if magic != SNAPSHOT_MAGIC {
            return Err(SnapshotError::InvalidMagic(magic));
        }

        let version = u32::decode_canonical(bytes, &mut cursor)?;
        if version != SNAPSHOT_VERSION {
            return Err(SnapshotError::UnsupportedVersion(version));
        }

        let chain_id = u64::decode_canonical(bytes, &mut cursor)?;
        let height = u64::decode_canonical(bytes, &mut cursor)?;
        let epoch = u64::decode_canonical(bytes, &mut cursor)?;
        let block_hash = Hash256::decode_canonical(bytes, &mut cursor)?;
        let state_root = Hash256::decode_canonical(bytes, &mut cursor)?;

        let accounts_len = u64::decode_canonical(bytes, &mut cursor)?;
        let mut accounts = Vec::with_capacity(accounts_len as usize);
        for _ in 0..accounts_len {
            let addr = Address::decode_canonical(bytes, &mut cursor)?;
            let acct = Account::decode_canonical(bytes, &mut cursor)?;
            accounts.push((addr, acct));
        }

        let has_cert = u8::decode_canonical(bytes, &mut cursor)?;
        let certificate = if has_cert == 1 {
            let cert_block_hash = Hash256::decode_canonical(bytes, &mut cursor)?;
            let cert_height = u64::decode_canonical(bytes, &mut cursor)?;
            let cert_round = u64::decode_canonical(bytes, &mut cursor)?;
            let votes_len = u64::decode_canonical(bytes, &mut cursor)?;
            let mut precommits = Vec::with_capacity(votes_len as usize);
            for _ in 0..votes_len {
                let vote = crate::consensus::vote::Vote::decode_canonical(bytes, &mut cursor)?;
                precommits.push(vote);
            }
            Some(CommitCertificate {
                block_hash: cert_block_hash,
                height: cert_height,
                round: cert_round,
                precommits,
            })
        } else {
            None
        };

        Ok(Self {
            magic,
            version,
            chain_id,
            height,
            epoch,
            block_hash,
            state_root,
            accounts,
            certificate,
        })
    }
}
