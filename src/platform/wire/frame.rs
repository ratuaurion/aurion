//! Wire framing protokol jaringan P2P Aurion (Header 52 Bytes + Magic 0x41555230).

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::core::Hash256;
use crate::crypto::blake3_hash;
use thiserror::Error;

pub const WIRE_MAGIC: [u8; 4] = [0x41, 0x55, 0x52, 0x30]; // "AUR0"
pub const WIRE_FRAME_HEADER_BYTES: usize = 52;
pub const MAX_WIRE_PAYLOAD_BYTES: usize = 8 * 1024 * 1024; // 8 MB

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WireError {
    #[error("Invalid protocol magic: expected AUR0, got {0:?}")]
    InvalidMagic([u8; 4]),
    #[error("Payload exceeds maximum network frame size of 8MB: {0} bytes")]
    PayloadTooLarge(usize),
    #[error("Checksum mismatch: expected {expected}, got {computed}")]
    ChecksumMismatch {
        expected: Hash256,
        computed: Hash256,
    },
    #[error("Codec error: {0}")]
    Codec(#[from] CodecError),
}

/// Header frame jaringan P2P berukuran tepat 52 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireFrameHeader {
    pub magic: [u8; 4],
    pub message_type: u16,
    pub reserved: u16,
    pub payload_len: u32,
    pub reserved2: [u8; 8],
    pub payload_checksum: Hash256,
}

impl CanonicalEncode for WireFrameHeader {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.magic);
        self.message_type.encode_canonical(buf);
        self.reserved.encode_canonical(buf);
        self.payload_len.encode_canonical(buf);
        buf.extend_from_slice(&self.reserved2);
        self.payload_checksum.encode_canonical(buf);
    }
}

impl CanonicalDecode for WireFrameHeader {
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let mut magic = [0u8; 4];
        if bytes.len().saturating_sub(*cursor) < 4 {
            return Err(CodecError::UnexpectedEof {
                needed: 4,
                available: bytes.len().saturating_sub(*cursor),
            });
        }
        magic.copy_from_slice(&bytes[*cursor..*cursor + 4]);
        *cursor += 4;

        let message_type = u16::decode_canonical(bytes, cursor)?;
        let reserved = u16::decode_canonical(bytes, cursor)?;
        let payload_len = u32::decode_canonical(bytes, cursor)?;

        let mut reserved2 = [0u8; 8];
        if bytes.len().saturating_sub(*cursor) < 8 {
            return Err(CodecError::UnexpectedEof {
                needed: 8,
                available: bytes.len().saturating_sub(*cursor),
            });
        }
        reserved2.copy_from_slice(&bytes[*cursor..*cursor + 8]);
        *cursor += 8;

        let payload_checksum = Hash256::decode_canonical(bytes, cursor)?;

        Ok(WireFrameHeader {
            magic,
            message_type,
            reserved,
            payload_len,
            reserved2,
            payload_checksum,
        })
    }
}

/// Serialisasi frame jaringan lengkap (Header 52B + Payload).
pub fn serialize_network_frame(
    message_type: u16,
    payload: &[u8],
) -> Result<Vec<u8>, WireError> {
    if payload.len() > MAX_WIRE_PAYLOAD_BYTES {
        return Err(WireError::PayloadTooLarge(payload.len()));
    }

    let checksum = blake3_hash(payload);
    let header = WireFrameHeader {
        magic: WIRE_MAGIC,
        message_type,
        reserved: 0,
        payload_len: payload.len() as u32,
        reserved2: [0u8; 8],
        payload_checksum: checksum,
    };

    let mut buf = Vec::with_capacity(WIRE_FRAME_HEADER_BYTES + payload.len());
    header.encode_canonical(&mut buf);
    buf.extend_from_slice(payload);
    Ok(buf)
}

/// Deserialisasi dan verifikasi integritas frame jaringan P2P.
pub fn parse_network_frame(raw_frame: &[u8]) -> Result<(WireFrameHeader, &[u8]), WireError> {
    let mut cursor = 0;
    let header = WireFrameHeader::decode_canonical(raw_frame, &mut cursor)?;

    if header.magic != WIRE_MAGIC {
        return Err(WireError::InvalidMagic(header.magic));
    }

    let payload_len = header.payload_len as usize;
    if payload_len > MAX_WIRE_PAYLOAD_BYTES {
        return Err(WireError::PayloadTooLarge(payload_len));
    }

    let remaining = raw_frame.len().saturating_sub(cursor);
    if remaining != payload_len {
        return Err(WireError::Codec(CodecError::UnexpectedEof {
            needed: payload_len,
            available: remaining,
        }));
    }

    let payload = &raw_frame[cursor..];
    let computed_checksum = blake3_hash(payload);
    if computed_checksum != header.payload_checksum {
        return Err(WireError::ChecksumMismatch {
            expected: header.payload_checksum,
            computed: computed_checksum,
        });
    }

    Ok((header, payload))
}
