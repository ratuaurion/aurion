#![forbid(unsafe_code)]

//! Blok kanonikal Aurion: Header, Transaksi, dan Sertifikat Komitmen BFT.
//! Mematuhi Konstitusi Konsensus dan Protokol Wire Aurion.

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::consensus::certificate::{CommitCertificate, ValidatorSet};
use crate::consensus::header::BlockHeader;
use crate::core::Hash256;
use crate::crypto::{blake3_derive_key, ed25519_verify_strict, Keypair};
use crate::genesis::builder::GENESIS_CHAIN_ID;
use crate::transaction::types::Transaction;
use thiserror::Error;

pub const DST_MERKLE_BRANCH: &str = "AURION-MERKLE-BRANCH-V1";
pub const DST_BLOCK_PROPOSAL: &str = "AURION-PROPOSAL-V1";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProposalEnvelopeError {
    #[error("proposer index {index} is outside validator set of size {set_size}")]
    ProposerIndexOutOfBounds { index: u32, set_size: usize },
    #[error("proposal chain ID {actual} is not canonical ({expected})")]
    InvalidChainId { actual: u32, expected: u32 },
    #[error("proposal proposer is not the deterministic leader")]
    InvalidProposer,
    #[error("proposal signature verification failed")]
    InvalidSignature,
}

/// Wire envelope yang mengikat proposal ke proposer dan domain chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockProposalEnvelope {
    pub block: Block,
    pub proposer_index: u32,
    pub signature: [u8; 64],
}

impl CanonicalEncode for BlockProposalEnvelope {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        self.block.encode_canonical(buf);
        self.proposer_index.encode_canonical(buf);
        crate::core::Signature::from_bytes(self.signature).encode_canonical(buf);
    }
}

impl CanonicalDecode for BlockProposalEnvelope {
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let block = Block::decode_canonical(bytes, cursor)?;
        let proposer_index = u32::decode_canonical(bytes, cursor)?;
        let signature = crate::core::Signature::decode_canonical(bytes, cursor)?;
        Ok(Self {
            block,
            proposer_index,
            signature: *signature.as_bytes(),
        })
    }
}

impl BlockProposalEnvelope {
    pub fn new_signed(block: Block, chain_id: u32, proposer_index: u32, keypair: &Keypair) -> Self {
        let signature = keypair.sign(&Self::signing_payload(&block, chain_id));
        Self {
            block,
            proposer_index,
            signature: *signature.as_bytes(),
        }
    }

    pub fn signing_payload(block: &Block, chain_id: u32) -> Vec<u8> {
        let mut payload = Vec::with_capacity(8 + 4 + 8 + 8 + 32);
        payload.extend_from_slice(DST_BLOCK_PROPOSAL.as_bytes());
        payload.extend_from_slice(&chain_id.to_be_bytes());
        payload.extend_from_slice(&block.header.height.to_be_bytes());
        payload.extend_from_slice(&block.header.round.to_be_bytes());
        payload.extend_from_slice(block.hash().as_bytes());
        payload
    }

    pub fn verify(
        &self,
        chain_id: u32,
        validator_set: &ValidatorSet,
    ) -> Result<(), ProposalEnvelopeError> {
        if chain_id != GENESIS_CHAIN_ID {
            return Err(ProposalEnvelopeError::InvalidChainId {
                actual: chain_id,
                expected: GENESIS_CHAIN_ID,
            });
        }

        let validator = validator_set.get_validator(self.proposer_index).ok_or(
            ProposalEnvelopeError::ProposerIndexOutOfBounds {
                index: self.proposer_index,
                set_size: validator_set.validators.len(),
            },
        )?;

        let expected = crate::consensus::bft::engine::BftEngine::select_proposer(
            validator_set,
            self.block.header.height,
            self.block.header.round,
            &self.block.header.prev_block_hash,
        );
        if expected != self.proposer_index {
            return Err(ProposalEnvelopeError::InvalidProposer);
        }

        let signature = crate::core::Signature::from_bytes(self.signature);
        ed25519_verify_strict(
            &validator.consensus_pubkey,
            &Self::signing_payload(&self.block, chain_id),
            &signature,
        )
        .map_err(|_| ProposalEnvelopeError::InvalidSignature)
    }
}

/// Blok kanonikal Aurion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub commit_certificate: Option<CommitCertificate>,
}

impl CanonicalEncode for Block {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        self.header.encode_canonical(buf);
        let tx_count = self.transactions.len() as u32;
        tx_count.encode_canonical(buf);
        for tx in &self.transactions {
            tx.encode_canonical(buf);
        }
        match &self.commit_certificate {
            Some(cert) => {
                1u8.encode_canonical(buf);
                cert.encode_canonical(buf);
            }
            None => {
                0u8.encode_canonical(buf);
            }
        }
    }
}

impl CanonicalDecode for Block {
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let header = BlockHeader::decode_canonical(bytes, cursor)?;
        let tx_count = u32::decode_canonical(bytes, cursor)?;
        let mut transactions = Vec::with_capacity(tx_count as usize);
        for _ in 0..tx_count {
            transactions.push(Transaction::decode_canonical(bytes, cursor)?);
        }
        let has_cert = u8::decode_canonical(bytes, cursor)?;
        let commit_certificate = match has_cert {
            0 => None,
            1 => Some(CommitCertificate::decode_canonical(bytes, cursor)?),
            other => return Err(CodecError::InvalidBoolean(other)),
        };

        Ok(Block {
            header,
            transactions,
            commit_certificate,
        })
    }
}

impl Block {
    /// Membuat instance blok baru.
    pub fn new(
        header: BlockHeader,
        transactions: Vec<Transaction>,
        commit_certificate: Option<CommitCertificate>,
    ) -> Self {
        Self {
            header,
            transactions,
            commit_certificate,
        }
    }

    /// Hash blok kanonikal (dihitung dari header).
    #[inline]
    pub fn hash(&self) -> Hash256 {
        self.header.compute_block_hash()
    }

    /// Nomor tinggi blok.
    #[inline]
    pub fn height(&self) -> u64 {
        self.header.height
    }

    /// Hitung Merkle Root transaksi secara deterministik menggunakan Blake3 binary tree.
    pub fn calculate_tx_merkle_root(transactions: &[Transaction]) -> Hash256 {
        if transactions.is_empty() {
            return Hash256::ZERO;
        }

        let mut current_level: Vec<Hash256> =
            transactions.iter().map(|tx| tx.compute_tx_id()).collect();

        while current_level.len() > 1 {
            let mut next_level = Vec::with_capacity(current_level.len().div_ceil(2));
            for chunk in current_level.chunks(2) {
                let left = chunk[0];
                let right = if chunk.len() > 1 { chunk[1] } else { chunk[0] };

                let mut pair = [0u8; 64];
                pair[..32].copy_from_slice(left.as_bytes());
                pair[32..].copy_from_slice(right.as_bytes());
                next_level.push(blake3_derive_key(DST_MERKLE_BRANCH, &pair));
            }
            current_level = next_level;
        }

        current_level[0]
    }

    /// Verifikasi apakah `tx_merkle_root` pada header cocok dengan transaksi di dalam blok.
    pub fn verify_tx_merkle_root(&self) -> bool {
        self.header.tx_merkle_root == Self::calculate_tx_merkle_root(&self.transactions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Hash256;

    #[test]
    fn test_empty_merkle_root_is_zero() {
        let root = Block::calculate_tx_merkle_root(&[]);
        assert_eq!(root, Hash256::ZERO);
    }

    #[test]
    fn test_block_canonical_encode_decode_roundtrip() {
        let header = BlockHeader {
            version: 1,
            height: 10,
            round: 0,
            timestamp: 1773532800,
            prev_block_hash: Hash256::from_bytes([2u8; 32]),
            tx_merkle_root: Hash256::ZERO,
            state_root: Hash256::from_bytes([3u8; 32]),
        };

        let block = Block::new(header, Vec::new(), None);
        let mut buf = Vec::new();
        block.encode_canonical(&mut buf);

        let mut cursor = 0;
        let decoded = Block::decode_canonical(&buf, &mut cursor).expect("Decode block failed");
        assert_eq!(cursor, buf.len());
        assert_eq!(block, decoded);
    }

    #[test]
    fn test_proposal_envelope_canonical_roundtrip() {
        let block = Block::new(
            BlockHeader {
                version: 1,
                height: 7,
                round: 2,
                timestamp: 1773532800,
                prev_block_hash: Hash256::from_bytes([4u8; 32]),
                tx_merkle_root: Hash256::ZERO,
                state_root: Hash256::from_bytes([5u8; 32]),
            },
            Vec::new(),
            None,
        );
        let envelope = BlockProposalEnvelope {
            block,
            proposer_index: 3,
            signature: [9u8; 64],
        };
        let mut encoded = Vec::new();
        envelope.encode_canonical(&mut encoded);
        let decoded = BlockProposalEnvelope::decode_canonical_exact(&encoded)
            .expect("proposal envelope must decode canonically");
        assert_eq!(decoded, envelope);
    }
}
