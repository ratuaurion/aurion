//! CommitCertificate dan Quorum Verification BFT konsensus Aurion.

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::consensus::vote::{Vote, PHASE_PRECOMMIT, VOTE_BYTES};
use crate::core::{Address, Hash256};
use thiserror::Error;

pub const VALIDATOR_ENTRY_BYTES: usize = 72;
pub const COMMIT_CERTIFICATE_FIXED_BYTES: usize = 32 + 8 + 8 + 4;
pub const MAX_COMMIT_CERTIFICATE_VOTES: usize = 65_535;

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
    #[error("Mismatched height in vote")]
    MismatchedHeight,
    #[error("Mismatched round in vote")]
    MismatchedRound,
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
            if vote.height != self.height {
                return Err(CertificateError::MismatchedHeight);
            }
            if vote.round != self.round {
                return Err(CertificateError::MismatchedRound);
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

impl CanonicalEncode for CommitCertificate {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        self.block_hash.encode_canonical(buf);
        self.height.encode_canonical(buf);
        self.round.encode_canonical(buf);
        let count = self.precommits.len() as u32;
        count.encode_canonical(buf);
        for vote in &self.precommits {
            vote.encode_canonical(buf);
        }
    }
}

impl CanonicalDecode for CommitCertificate {
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let block_hash = Hash256::decode_canonical(bytes, cursor)?;
        let height = u64::decode_canonical(bytes, cursor)?;
        let round = u64::decode_canonical(bytes, cursor)?;
        let count = u32::decode_canonical(bytes, cursor)?;
        let count = count as usize;
        if count > MAX_COMMIT_CERTIFICATE_VOTES {
            return Err(CodecError::ExcessiveAllocation {
                max: MAX_COMMIT_CERTIFICATE_VOTES,
                requested: count,
            });
        }
        let required_vote_bytes = count * VOTE_BYTES;
        let available = bytes.len().saturating_sub(*cursor);
        if available < required_vote_bytes {
            return Err(CodecError::UnexpectedEof {
                needed: required_vote_bytes,
                available,
            });
        }
        let mut precommits = Vec::with_capacity(count);
        for _ in 0..count {
            precommits.push(Vote::decode_canonical(bytes, cursor)?);
        }

        Ok(CommitCertificate {
            block_hash,
            height,
            round,
            precommits,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Hash256;

    #[test]
    fn canonical_certificate_vector() {
        let certificate = CommitCertificate {
            block_hash: Hash256::from_bytes([0x11; 32]),
            height: 7,
            round: 3,
            precommits: vec![Vote {
                phase: PHASE_PRECOMMIT,
                height: 7,
                round: 3,
                block_hash: Hash256::from_bytes([0x11; 32]),
                validator_index: 2,
                signature: crate::core::Signature::from_bytes([0x22; 64]),
            }],
        };
        let bytes = certificate.to_canonical_bytes();

        assert_eq!(bytes.len(), COMMIT_CERTIFICATE_FIXED_BYTES + VOTE_BYTES);
        assert_eq!(&bytes[..32], &[0x11; 32]);
        assert_eq!(&bytes[32..40], &7u64.to_be_bytes());
        assert_eq!(&bytes[40..48], &3u64.to_be_bytes());
        assert_eq!(&bytes[48..52], &1u32.to_be_bytes());
        assert_eq!(bytes[52], PHASE_PRECOMMIT);
        assert_eq!(
            &bytes[52 + 1 + 8 + 8 + 32..52 + 1 + 8 + 8 + 32 + 4],
            &2u32.to_be_bytes()
        );
        assert_eq!(&bytes[bytes.len() - 64..], &[0x22; 64]);

        let decoded = CommitCertificate::decode_canonical_exact(&bytes).expect("decode vector");
        assert_eq!(decoded, certificate);
    }

    #[test]
    fn decoder_rejects_excessive_precommit_count_before_allocation() {
        let mut bytes = vec![0u8; COMMIT_CERTIFICATE_FIXED_BYTES];
        bytes[48..52].copy_from_slice(&(MAX_COMMIT_CERTIFICATE_VOTES as u32 + 1).to_be_bytes());

        assert_eq!(
            CommitCertificate::decode_canonical_exact(&bytes),
            Err(CodecError::ExcessiveAllocation {
                max: MAX_COMMIT_CERTIFICATE_VOTES,
                requested: MAX_COMMIT_CERTIFICATE_VOTES + 1,
            })
        );
    }
}
