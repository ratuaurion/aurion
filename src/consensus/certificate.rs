//! CommitCertificate dan Quorum Verification BFT konsensus Aurion.

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::core::{Address, Hash256};
use crate::consensus::vote::{Vote, PHASE_PRECOMMIT};
use thiserror::Error;

pub const VALIDATOR_ENTRY_BYTES: usize = 72;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CertificateError {
    #[error("Quorum not reached: accumulated {accumulated} voting weight, required {required}")]
    QuorumNotReached { accumulated: u64, required: u64 },
    #[error("Invalid vote in certificate: validator index {0} out of bounds")]
    UnknownValidator(u32),
    #[error("Duplicate vote detected for validator index {0}")]
    DuplicateVote(u32),
    #[error("Mismatched block hash in vote")]
    MismatchedBlockHash,
    #[error("Invalid vote phase: expected Precommit (0x02), got {0:#04x}")]
    InvalidPhase(u8),
    #[error("Vote signature verification failed for validator index {0}")]
    InvalidSignature(u32),
}

/// Entri validator pada himpunan konsensus (Tepat 72 Bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorEntry {
    pub validator_id: Address,
    pub consensus_pubkey: [u8; 32],
    pub voting_weight: u64,
}

impl CanonicalEncode for ValidatorEntry {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        self.validator_id.encode_canonical(buf);
        buf.extend_from_slice(&self.consensus_pubkey);
        self.voting_weight.encode_canonical(buf);
    }
}

impl CanonicalDecode for ValidatorEntry {
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let validator_id = Address::decode_canonical(bytes, cursor)?;
        let mut consensus_pubkey = [0u8; 32];
        if bytes.len().saturating_sub(*cursor) < 32 {
            return Err(CodecError::UnexpectedEof {
                needed: 32,
                available: bytes.len().saturating_sub(*cursor),
            });
        }
        consensus_pubkey.copy_from_slice(&bytes[*cursor..*cursor + 32]);
        *cursor += 32;
        let voting_weight = u64::decode_canonical(bytes, cursor)?;

        Ok(ValidatorEntry {
            validator_id,
            consensus_pubkey,
            voting_weight,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ValidatorSet {
    pub validators: Vec<ValidatorEntry>,
}

impl ValidatorSet {
    pub fn new(validators: Vec<ValidatorEntry>) -> Self {
        Self { validators }
    }

    pub fn total_voting_power(&self) -> u64 {
        self.validators.iter().map(|v| v.voting_weight).sum()
    }

    pub fn quorum_threshold(&self) -> u64 {
        let total = self.total_voting_power();
        // Strict > 2/3: (total * 2) / 3 + 1
        (total * 2) / 3 + 1
    }

    pub fn get_validator(&self, index: u32) -> Option<&ValidatorEntry> {
        self.validators.get(index as usize)
    }
}

/// Bukti komitmen finalitas BFT yang memuat kuorum tanda tangan precommit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitCertificate {
    pub block_hash: Hash256,
    pub height: u64,
    pub round: u64,
    pub precommits: Vec<Vote>,
}

impl CommitCertificate {
    /// Verifikasi kuorum sertifikat terhadap validator set kanonikal.
    pub fn verify(&self, val_set: &ValidatorSet) -> Result<(), CertificateError> {
        let required = val_set.quorum_threshold();
        let mut accumulated: u64 = 0;
        let mut seen_indices = Vec::new();

        for vote in &self.precommits {
            if vote.phase != PHASE_PRECOMMIT {
                return Err(CertificateError::InvalidPhase(vote.phase));
            }

            if vote.block_hash != self.block_hash {
                return Err(CertificateError::MismatchedBlockHash);
            }

            if seen_indices.contains(&vote.validator_index) {
                return Err(CertificateError::DuplicateVote(vote.validator_index));
            }

            let val_entry = val_set
                .get_validator(vote.validator_index)
                .ok_or(CertificateError::UnknownValidator(vote.validator_index))?;

            vote.verify(val_set)
                .map_err(|_| CertificateError::InvalidSignature(vote.validator_index))?;

            seen_indices.push(vote.validator_index);
            accumulated = accumulated.saturating_add(val_entry.voting_weight);
        }

        if accumulated < required {
            return Err(CertificateError::QuorumNotReached {
                accumulated,
                required,
            });
        }

        Ok(())
    }
}
