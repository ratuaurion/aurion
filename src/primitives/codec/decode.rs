//! CanonicalDecode: Deserialisasi kanonikal dengan penolakan trailing bytes ketat.

use crate::core::{Address, Hash256, Quantum, Signature};
use thiserror::Error;

pub const MAX_ALLOWED_ALLOCATION_BYTES: usize = 16 * 1024 * 1024; // 16 MB

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CodecError {
    #[error("Unexpected end of buffer: needed {needed} bytes, available {available}")]
    UnexpectedEof { needed: usize, available: usize },
    #[error("Trailing unconsumed bytes detected in stream: {0} bytes remaining")]
    TrailingBytes(usize),
    #[error("Invalid boolean encoding: expected 0 or 1, got {0}")]
    InvalidBoolean(u8),
    #[error("Allocation exceeds hard limit of 16MB: requested {0} bytes")]
    ExcessiveAllocation(usize),
}

pub trait CanonicalDecode: Sized {
    fn decode_canonical_exact(bytes: &[u8]) -> Result<Self, CodecError> {
        let mut cursor = 0;
        let val = Self::decode_canonical(bytes, &mut cursor)?;
        if cursor != bytes.len() {
            return Err(CodecError::TrailingBytes(bytes.len() - cursor));
        }
        Ok(val)
    }

    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError>;
}

#[inline]
fn ensure_available(bytes: &[u8], cursor: usize, needed: usize) -> Result<(), CodecError> {
    let avail = bytes.len().saturating_sub(cursor);
    if avail < needed {
        return Err(CodecError::UnexpectedEof {
            needed,
            available: avail,
        });
    }
    Ok(())
}

impl CanonicalDecode for u8 {
    #[inline]
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        ensure_available(bytes, *cursor, 1)?;
        let val = bytes[*cursor];
        *cursor += 1;
        Ok(val)
    }
}

impl CanonicalDecode for u16 {
    #[inline]
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        ensure_available(bytes, *cursor, 2)?;
        let mut arr = [0u8; 2];
        arr.copy_from_slice(&bytes[*cursor..*cursor + 2]);
        *cursor += 2;
        Ok(u16::from_be_bytes(arr))
    }
}

impl CanonicalDecode for u32 {
    #[inline]
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        ensure_available(bytes, *cursor, 4)?;
        let mut arr = [0u8; 4];
        arr.copy_from_slice(&bytes[*cursor..*cursor + 4]);
        *cursor += 4;
        Ok(u32::from_be_bytes(arr))
    }
}

impl CanonicalDecode for u64 {
    #[inline]
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        ensure_available(bytes, *cursor, 8)?;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(&bytes[*cursor..*cursor + 8]);
        *cursor += 8;
        Ok(u64::from_be_bytes(arr))
    }
}

impl CanonicalDecode for u128 {
    #[inline]
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        ensure_available(bytes, *cursor, 16)?;
        let mut arr = [0u8; 16];
        arr.copy_from_slice(&bytes[*cursor..*cursor + 16]);
        *cursor += 16;
        Ok(u128::from_be_bytes(arr))
    }
}

impl CanonicalDecode for bool {
    #[inline]
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let byte = u8::decode_canonical(bytes, cursor)?;
        match byte {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(CodecError::InvalidBoolean(other)),
        }
    }
}

impl CanonicalDecode for Hash256 {
    #[inline]
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        ensure_available(bytes, *cursor, 32)?;
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes[*cursor..*cursor + 32]);
        *cursor += 32;
        Ok(Hash256(arr))
    }
}

impl CanonicalDecode for Address {
    #[inline]
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        ensure_available(bytes, *cursor, 32)?;
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes[*cursor..*cursor + 32]);
        *cursor += 32;
        Ok(Address(arr))
    }
}

impl CanonicalDecode for Signature {
    #[inline]
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        ensure_available(bytes, *cursor, 64)?;
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes[*cursor..*cursor + 64]);
        *cursor += 64;
        Ok(Signature(arr))
    }
}

impl CanonicalDecode for Quantum {
    #[inline]
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let val = u128::decode_canonical(bytes, cursor)?;
        Ok(Quantum(val))
    }
}

impl CanonicalDecode for Vec<u8> {
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let len = u32::decode_canonical(bytes, cursor)? as usize;
        if len > MAX_ALLOWED_ALLOCATION_BYTES {
            return Err(CodecError::ExcessiveAllocation(len));
        }
        ensure_available(bytes, *cursor, len)?;
        let slice = &bytes[*cursor..*cursor + len];
        *cursor += len;
        Ok(slice.to_vec())
    }
}
