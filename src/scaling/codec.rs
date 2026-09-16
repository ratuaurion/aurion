//! Serializer dan Deserializer Pembingkaian Biner Kanonikal (DA) L2 Rollup.
//! Sesuai Dokumen Spesifikasi: docs/Application-Rules-Layer/application/aurion-l2-scaling/02-L2-BATCH-CALLDATA-COMPRESSION-SPECIFICATION.md
//! Mematuhi Invariant: L2-DA-001..002, AUR-ARCH-011 (#![forbid(unsafe_code)]), AUR-ARCH-012 (Zero-Float).

use thiserror::Error;
use crate::core::Hash256;
use crate::crypto::blake3_hash;

/// Magic bytes penanda paket calldata biner Aurion L2: "AUL2"
pub const MAGIC_AUL2: [u8; 4] = [0x41, 0x55, 0x4C, 0x32];

/// Versi biner pembingkaian kanonikal
pub const PROTOCOL_VERSION_1: u8 = 0x01;

/// Ukuran tetap header pembingkaian batch L2 (102 byte)
pub const BATCH_FRAME_HEADER_SIZE: usize = 102;

/// Kesalahan pemrosesan codec calldata L2
#[derive(Debug, Error, PartialEq, Eq)]
pub enum L2CodecError {
    #[error("Magic bytes biner tidak valid: {0:02X?} (diharapkan 'AUL2')")]
    InvalidMagicHeader([u8; 4]),

    #[error("Versi protokol pembingkaian tidak didukung: {0}")]
    UnsupportedVersion(u8),

    #[error("Ukuran frame calldata tidak mencukupi (diharapkan minimal {expected} byte, diterima {actual} byte)")]
    HeaderTooShort { expected: usize, actual: usize },

    #[error("Panjang payload aktual ({actual}) tidak cocok dengan deklarasi header ({expected})")]
    PayloadLengthMismatch { expected: usize, actual: usize },

    #[error("Payload transaksi korup: {0}")]
    CorruptedPayload(&'static str),
}

/// Header Tetap Pembingkaian Batch L2 (102 Byte)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2BatchFrameHeader {
    pub magic: [u8; 4],
    pub version: u8,
    pub compression_flags: u8,
    pub batch_index: u64,
    pub prev_state_root: Hash256,
    pub new_state_root: Hash256,
    pub start_block: u64,
    pub end_block: u64,
    pub tx_count: u32,
    pub payload_len: u32,
}

impl L2BatchFrameHeader {
    /// Mengodekan header tetap ke dalam 102 byte kanonikal Big-Endian
    #[must_use]
    pub fn encode(&self) -> [u8; BATCH_FRAME_HEADER_SIZE] {
        let mut buf = [0u8; BATCH_FRAME_HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.magic);
        buf[4] = self.version;
        buf[5] = self.compression_flags;
        buf[6..14].copy_from_slice(&self.batch_index.to_be_bytes());
        buf[14..46].copy_from_slice(self.prev_state_root.as_bytes());
        buf[46..78].copy_from_slice(self.new_state_root.as_bytes());
        buf[78..86].copy_from_slice(&self.start_block.to_be_bytes());
        buf[86..94].copy_from_slice(&self.end_block.to_be_bytes());
        buf[94..98].copy_from_slice(&self.tx_count.to_be_bytes());
        buf[98..102].copy_from_slice(&self.payload_len.to_be_bytes());
        buf
    }

    /// Mendekode 102 byte biner menjadi L2BatchFrameHeader
    pub fn decode(bytes: &[u8]) -> Result<Self, L2CodecError> {
        if bytes.len() < BATCH_FRAME_HEADER_SIZE {
            return Err(L2CodecError::HeaderTooShort {
                expected: BATCH_FRAME_HEADER_SIZE,
                actual: bytes.len(),
            });
        }

        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[0..4]);
        if magic != MAGIC_AUL2 {
            return Err(L2CodecError::InvalidMagicHeader(magic));
        }

        let version = bytes[4];
        if version != PROTOCOL_VERSION_1 {
            return Err(L2CodecError::UnsupportedVersion(version));
        }

        let compression_flags = bytes[5];

        let mut batch_idx_bytes = [0u8; 8];
        batch_idx_bytes.copy_from_slice(&bytes[6..14]);
        let batch_index = u64::from_be_bytes(batch_idx_bytes);

        let mut prev_root_bytes = [0u8; 32];
        prev_root_bytes.copy_from_slice(&bytes[14..46]);
        let prev_state_root = Hash256::from_bytes(prev_root_bytes);

        let mut new_root_bytes = [0u8; 32];
        new_root_bytes.copy_from_slice(&bytes[46..78]);
        let new_state_root = Hash256::from_bytes(new_root_bytes);

        let mut start_blk_bytes = [0u8; 8];
        start_blk_bytes.copy_from_slice(&bytes[78..86]);
        let start_block = u64::from_be_bytes(start_blk_bytes);

        let mut end_blk_bytes = [0u8; 8];
        end_blk_bytes.copy_from_slice(&bytes[86..94]);
        let end_block = u64::from_be_bytes(end_blk_bytes);

        let mut tx_cnt_bytes = [0u8; 4];
        tx_cnt_bytes.copy_from_slice(&bytes[94..98]);
        let tx_count = u32::from_be_bytes(tx_cnt_bytes);

        let mut payload_len_bytes = [0u8; 4];
        payload_len_bytes.copy_from_slice(&bytes[98..102]);
        let payload_len = u32::from_be_bytes(payload_len_bytes);

        Ok(Self {
            magic,
            version,
            compression_flags,
            batch_index,
            prev_state_root,
            new_state_root,
            start_block,
            end_block,
            tx_count,
            payload_len,
        })
    }
}

/// Paket Calldata Batch Lengkap untuk Diposting ke Layer-1 DA
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2BatchFrame {
    pub header: L2BatchFrameHeader,
    pub payload: Vec<u8>,
}

impl L2BatchFrame {
    /// Membuat batch frame baru dengan payload transaksi
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        batch_index: u64,
        prev_state_root: Hash256,
        new_state_root: Hash256,
        start_block: u64,
        end_block: u64,
        tx_count: u32,
        compression_flags: u8,
        payload: Vec<u8>,
    ) -> Self {
        let payload_len = u32::try_from(payload.len()).unwrap_or(u32::MAX);
        let header = L2BatchFrameHeader {
            magic: MAGIC_AUL2,
            version: PROTOCOL_VERSION_1,
            compression_flags,
            batch_index,
            prev_state_root,
            new_state_root,
            start_block,
            end_block,
            tx_count,
            payload_len,
        };
        Self { header, payload }
    }

    /// Mengodekan seluruh frame (Header + Payload) ke dalam biner calldata kanonikal
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(BATCH_FRAME_HEADER_SIZE + self.payload.len());
        encoded.extend_from_slice(&self.header.encode());
        encoded.extend_from_slice(&self.payload);
        encoded
    }

    /// Mendekode data calldata biner dari Layer-1 menjadi L2BatchFrame
    pub fn decode(bytes: &[u8]) -> Result<Self, L2CodecError> {
        let header = L2BatchFrameHeader::decode(bytes)?;
        let expected_total_size = BATCH_FRAME_HEADER_SIZE + header.payload_len as usize;

        if bytes.len() != expected_total_size {
            return Err(L2CodecError::PayloadLengthMismatch {
                expected: header.payload_len as usize,
                actual: bytes.len().saturating_sub(BATCH_FRAME_HEADER_SIZE),
            });
        }

        let payload = bytes[BATCH_FRAME_HEADER_SIZE..].to_vec();
        Ok(Self { header, payload })
    }

    /// Menghitung intisari hash komitmen Data Availability (DA) berbasis Blake3
    #[must_use]
    pub fn compute_da_hash(&self) -> Hash256 {
        let full_frame = self.encode();
        blake3_hash(&full_frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_frame_encode_decode_roundtrip() {
        let prev_root = Hash256::from_bytes([0x11; 32]);
        let new_root = Hash256::from_bytes([0x22; 32]);
        let payload = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04];

        let frame = L2BatchFrame::new(
            1,
            prev_root,
            new_root,
            10,
            20,
            2,
            0x00,
            payload,
        );

        let encoded = frame.encode();
        assert_eq!(encoded.len(), BATCH_FRAME_HEADER_SIZE + 8);
        assert_eq!(&encoded[0..4], &MAGIC_AUL2);

        let decoded = L2BatchFrame::decode(&encoded).expect("Decode harus berhasil");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn test_invalid_magic_header_rejected() {
        let mut corrupted = [0u8; BATCH_FRAME_HEADER_SIZE];
        corrupted[0..4].copy_from_slice(b"NOPE");
        corrupted[4] = PROTOCOL_VERSION_1;

        let err = L2BatchFrame::decode(&corrupted).expect_err("Harus gagal pada magic header salah");
        assert_eq!(err, L2CodecError::InvalidMagicHeader(*b"NOPE"));
    }

    #[test]
    fn test_payload_length_mismatch_rejected() {
        let prev_root = Hash256::from_bytes([0x00; 32]);
        let new_root = Hash256::from_bytes([0x01; 32]);
        let payload = vec![1, 2, 3, 4];

        let frame = L2BatchFrame::new(1, prev_root, new_root, 1, 2, 1, 0x00, payload);
        let mut encoded = frame.encode();
        // Pangkas 2 byte dari payload
        encoded.truncate(encoded.len() - 2);

        let err = L2BatchFrame::decode(&encoded).expect_err("Harus gagal saat panjang tidak cocok");
        assert_eq!(
            err,
            L2CodecError::PayloadLengthMismatch {
                expected: 4,
                actual: 2,
            }
        );
    }

    #[test]
    fn test_da_hash_determinism() {
        let frame = L2BatchFrame::new(
            5,
            Hash256::from_bytes([0xAA; 32]),
            Hash256::from_bytes([0xBB; 32]),
            50,
            60,
            10,
            0x00,
            vec![1, 2, 3],
        );

        let h1 = frame.compute_da_hash();
        let h2 = frame.compute_da_hash();
        assert_eq!(h1, h2);
        assert_ne!(h1, Hash256::ZERO);
    }
}
