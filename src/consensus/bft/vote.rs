//! Vote kanonikal BFT konsensus Aurion (Tepat 117 Bytes).

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::consensus::certificate::ValidatorSet;
use crate::core::{Hash256, Signature};
use crate::crypto::{
    blake3_hash, ed25519_verify_strict, Keypair, DST_BFT_PRECOMMIT, DST_BFT_PREVOTE,
};
use thiserror::Error;

pub const VOTE_BYTES: usize = 117;
pub const PHASE_PREVOTE: u8 = 0x01;
pub const PHASE_PRECOMMIT: u8 = 0x02;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VoteError {
    #[error("Invalid vote phase: expected 0x01 (PREVOTE) or 0x02 (PRECOMMIT), got {0:#04x}")]
    InvalidPhase(u8),
    #[error("Validator index {index} out of bounds (validator set size: {set_size})")]
    ValidatorIndexOutOfBounds { index: u32, set_size: usize },
    #[error("Invalid vote cryptographic signature")]
    InvalidSignature,
}

/// Struktur data Vote konsensus BFT (Tepat 117 Bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vote {
    pub phase: u8,
    pub height: u64,
    pub round: u64,
    pub block_hash: Hash256,
    pub validator_index: u32,
    pub signature: Signature,
}

impl CanonicalEncode for Vote {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        self.phase.encode_canonical(buf);
        self.height.encode_canonical(buf);
        self.round.encode_canonical(buf);
        self.block_hash.encode_canonical(buf);
        self.validator_index.encode_canonical(buf);
        self.signature.encode_canonical(buf);
    }
}

impl CanonicalDecode for Vote {
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let phase = u8::decode_canonical(bytes, cursor)?;
        if phase != PHASE_PREVOTE && phase != PHASE_PRECOMMIT {
            return Err(CodecError::InvalidBoolean(phase));
        }
        let height = u64::decode_canonical(bytes, cursor)?;
        let round = u64::decode_canonical(bytes, cursor)?;
        let block_hash = Hash256::decode_canonical(bytes, cursor)?;
        let validator_index = u32::decode_canonical(bytes, cursor)?;
        let signature = Signature::decode_canonical(bytes, cursor)?;

        Ok(Vote {
            phase,
            height,
            round,
            block_hash,
            validator_index,
            signature,
        })
    }
}

impl Vote {
    /// Buat dan tandatangani vote baru secara kanonikal.
    pub fn new_signed(
        keypair: &Keypair,
        phase: u8,
        height: u64,
        round: u64,
        block_hash: Hash256,
        validator_index: u32,
    ) -> Result<Self, VoteError> {
        if phase != PHASE_PREVOTE && phase != PHASE_PRECOMMIT {
            return Err(VoteError::InvalidPhase(phase));
        }

        let mut vote = Self {
            phase,
            height,
            round,
            block_hash,
            validator_index,
            signature: Signature::from_bytes([0u8; 64]),
        };

        let digest = vote.signing_hash();
        vote.signature = keypair.sign(digest.as_bytes());
        Ok(vote)
    }

    /// Preimage penandatanganan: DST || phase || height || round || block_hash || validator_index.
    pub fn signing_payload(&self) -> Vec<u8> {
        let dst = if self.phase == PHASE_PREVOTE {
            DST_BFT_PREVOTE
        } else {
            DST_BFT_PRECOMMIT
        };

        let mut buf = Vec::with_capacity(dst.len() + 1 + 8 + 8 + 32 + 4);
        buf.extend_from_slice(dst.as_bytes());
        buf.push(self.phase);
        self.height.encode_canonical(&mut buf);
        self.round.encode_canonical(&mut buf);
        self.block_hash.encode_canonical(&mut buf);
        self.validator_index.encode_canonical(&mut buf);
        buf
    }

    #[inline]
    pub fn signing_hash(&self) -> Hash256 {
        blake3_hash(&self.signing_payload())
    }

    /// Verifikasi keabsahan tanda tangan vote terhadap validator set.
    pub fn verify(&self, val_set: &ValidatorSet) -> Result<(), VoteError> {
        let val_entry = val_set.get_validator(self.validator_index).ok_or(
            VoteError::ValidatorIndexOutOfBounds {
                index: self.validator_index,
                set_size: val_set.validators.len(),
            },
        )?;

        let digest = self.signing_hash();
        ed25519_verify_strict(
            &val_entry.consensus_pubkey,
            digest.as_bytes(),
            &self.signature,
        )
        .map_err(|_| VoteError::InvalidSignature)
    }
}
