#![forbid(unsafe_code)]

//! Modul Manajemen Epoch dan Rotasi Validator Multi-Region Aurion (NET-011).
//!
//! Mematuhi Invariant:
//! - AUR-ARCH-001: Single Sovereign Ecosystem & Monolithic Executable (/bin/aurion).
//! - AUR-ARCH-005: Canonical Shared Types & Deterministic Validation.
//! - AUR-CONS-*: Round-Based BFT Consensus with >2/3 Quorum Finality.

use crate::consensus::bft::certificate::{CertificateError, CommitCertificate, ValidatorEntry, ValidatorSet};
use crate::core::Address;
use thiserror::Error;

/// Panjang epoch kanonikal untuk Mainnet/produksi.
pub const CANONICAL_EPOCH_BLOCKS: u64 = 10_000;

/// Panjang epoch khusus fixture test/dev.
pub const DEV_EPOCH_BLOCKS: u64 = 10;

/// Pilih panjang epoch berdasarkan mode runtime.
#[inline]
pub const fn resolve_epoch_blocks(is_dev_mode: bool) -> u64 {
    if is_dev_mode {
        DEV_EPOCH_BLOCKS
    } else {
        CANONICAL_EPOCH_BLOCKS
    }
}

/// Alias kompatibilitas untuk caller yang belum menerima konfigurasi runtime.
pub const DEFAULT_EPOCH_BLOCKS: u64 = CANONICAL_EPOCH_BLOCKS;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EpochError {
    #[error("Height {height} is not an epoch boundary (expected multiple of {epoch_len})")]
    NotAnEpochBoundary { height: u64, epoch_len: u64 },
    #[error("Validator set cannot be empty after rotation")]
    EmptyValidatorSet,
    #[error("Validator {0} not found in active set")]
    ValidatorNotFound(Address),
    #[error("Validator {0} is already in the active set")]
    DuplicateValidator(Address),
    #[error("Certificate error during epoch transition: {0}")]
    Certificate(#[from] CertificateError),
    #[error("Total voting power {0} does not satisfy strict quorum")]
    InsufficientVotingPower(u64),
}

/// Metadata dan jadwal giliran kepemimpinan validator pada suatu Epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochInfo {
    pub epoch_id: u64,
    pub start_height: u64,
    pub end_height: u64,
    pub active_validators: ValidatorSet,
}

impl EpochInfo {
    pub fn new(epoch_id: u64, epoch_len: u64, active_validators: ValidatorSet) -> Self {
        let start_height = epoch_id * epoch_len;
        let end_height = start_height + epoch_len.saturating_sub(1);
        Self {
            epoch_id,
            start_height,
            end_height,
            active_validators,
        }
    }

    pub fn contains_height(&self, height: u64) -> bool {
        height >= self.start_height && height <= self.end_height
    }
}

/// Bukti transisi antar-epoch yang memuat validator lama, validator baru, dan sertifikat komit batas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochTransition {
    pub from_epoch: u64,
    pub to_epoch: u64,
    pub boundary_height: u64,
    pub previous_validator_set: ValidatorSet,
    pub new_validator_set: ValidatorSet,
    pub transition_certificate: CommitCertificate,
}

impl EpochTransition {
    /// Buat dan validasi transisi epoch deterministik.
    pub fn create_and_verify(
        from_epoch: u64,
        boundary_height: u64,
        epoch_len: u64,
        previous_set: ValidatorSet,
        joining: Vec<ValidatorEntry>,
        leaving: &[Address],
        certificate: CommitCertificate,
    ) -> Result<Self, EpochError> {
        if !is_epoch_boundary(boundary_height, epoch_len) {
            return Err(EpochError::NotAnEpochBoundary {
                height: boundary_height,
                epoch_len,
            });
        }

        // 1. Verifikasi sertifikat komit batas terhadap validator set lama
        certificate.verify(&previous_set)?;

        // 2. Putar validator set
        let new_validator_set = rotate_validator_set(&previous_set, joining, leaving)?;

        Ok(Self {
            from_epoch,
            to_epoch: from_epoch + 1,
            boundary_height,
            previous_validator_set: previous_set,
            new_validator_set,
            transition_certificate: certificate,
        })
    }
}

/// Menghitung ID epoch untuk tinggi blok tertentu.
#[inline]
pub fn compute_epoch_id(height: u64, epoch_len: u64) -> u64 {
    if epoch_len == 0 {
        return 0;
    }
    height / epoch_len
}

/// Memeriksa apakah suatu tinggi blok merupakan batas pergantian epoch.
#[inline]
pub fn is_epoch_boundary(height: u64, epoch_len: u64) -> bool {
    if epoch_len == 0 || height == 0 {
        return false;
    }
    height.is_multiple_of(epoch_len)
}

/// Merotasi himpunan validator: mengeluarkan validator `leaving` dan menambahkan `joining`.
pub fn rotate_validator_set(
    current: &ValidatorSet,
    joining: Vec<ValidatorEntry>,
    leaving: &[Address],
) -> Result<ValidatorSet, EpochError> {
    let mut updated = Vec::new();

    // Pertahankan validator yang tidak ada di daftar leaving
    for v in &current.validators {
        if !leaving.contains(&v.validator_id) {
            updated.push(v.clone());
        }
    }

    // Tambahkan validator yang bergabung
    for j in joining {
        if updated.iter().any(|v| v.validator_id == j.validator_id) {
            return Err(EpochError::DuplicateValidator(j.validator_id));
        }
        updated.push(j);
    }

    if updated.is_empty() {
        return Err(EpochError::EmptyValidatorSet);
    }

    let new_set = ValidatorSet::new(updated);
    if new_set.total_voting_power() == 0 {
        return Err(EpochError::InsufficientVotingPower(0));
    }

    Ok(new_set)
}
