//! BlockHeader kanonikal Aurion (Tepat 124 Bytes).

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::core::Hash256;
use crate::crypto::{blake3_derive_key, DST_BLOCK_ID};

pub const BLOCK_HEADER_BYTES: usize = 124;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockHeader {
    pub version: u32,
    pub height: u64,
    pub round: u64,
    pub timestamp: u64,
    pub prev_block_hash: Hash256,
    pub tx_merkle_root: Hash256,
    pub state_root: Hash256,
}

impl CanonicalEncode for BlockHeader {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        self.version.encode_canonical(buf);
        self.height.encode_canonical(buf);
        self.round.encode_canonical(buf);
        self.timestamp.encode_canonical(buf);
        self.prev_block_hash.encode_canonical(buf);
        self.tx_merkle_root.encode_canonical(buf);
        self.state_root.encode_canonical(buf);
    }
}

impl CanonicalDecode for BlockHeader {
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let version = u32::decode_canonical(bytes, cursor)?;
        let height = u64::decode_canonical(bytes, cursor)?;
        let round = u64::decode_canonical(bytes, cursor)?;
        let timestamp = u64::decode_canonical(bytes, cursor)?;
        let prev_block_hash = Hash256::decode_canonical(bytes, cursor)?;
        let tx_merkle_root = Hash256::decode_canonical(bytes, cursor)?;
        let state_root = Hash256::decode_canonical(bytes, cursor)?;

        Ok(BlockHeader {
            version,
            height,
            round,
            timestamp,
            prev_block_hash,
            tx_merkle_root,
            state_root,
        })
    }
}

impl BlockHeader {
    /// Hitung Block ID / Header Hash kanonikal menggunakan Blake3 derive-key "AURION-BLOCK-ID-V1".
    pub fn compute_block_hash(&self) -> Hash256 {
        let mut raw = Vec::with_capacity(BLOCK_HEADER_BYTES);
        self.encode_canonical(&mut raw);
        blake3_derive_key(DST_BLOCK_ID, &raw)
    }
}
