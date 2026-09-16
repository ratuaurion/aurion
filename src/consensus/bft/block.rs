#![forbid(unsafe_code)]

//! Blok kanonikal Aurion: Header, Transaksi, dan Sertifikat Komitmen BFT.
//! Mematuhi Konstitusi Konsensus dan Protokol Wire Aurion.

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::consensus::certificate::CommitCertificate;
use crate::consensus::header::BlockHeader;
use crate::core::Hash256;
use crate::crypto::blake3_derive_key;
use crate::transaction::types::Transaction;


pub const DST_MERKLE_BRANCH: &str = "AURION-MERKLE-BRANCH-V1";

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

        let mut current_level: Vec<Hash256> = transactions
            .iter()
            .map(|tx| tx.compute_tx_id())
            .collect();

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
}
