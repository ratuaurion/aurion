//! CanonicalEncode: Serialisasi kanonikal deterministik big-endian.

use crate::core::{Address, Hash256, Quantum, Signature};

pub trait CanonicalEncode {
    fn encode_canonical(&self, buf: &mut Vec<u8>);

    fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.encode_canonical(&mut buf);
        buf
    }
}

impl CanonicalEncode for u8 {
    #[inline]
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        buf.push(*self);
    }
}

impl CanonicalEncode for u16 {
    #[inline]
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.to_be_bytes());
    }
}

impl CanonicalEncode for u32 {
    #[inline]
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.to_be_bytes());
    }
}

impl CanonicalEncode for u64 {
    #[inline]
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.to_be_bytes());
    }
}

impl CanonicalEncode for u128 {
    #[inline]
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.to_be_bytes());
    }
}

impl CanonicalEncode for bool {
    #[inline]
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        buf.push(if *self { 1 } else { 0 });
    }
}

impl CanonicalEncode for Hash256 {
    #[inline]
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(self.as_bytes());
    }
}

impl CanonicalEncode for Address {
    #[inline]
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(self.as_bytes());
    }
}

impl CanonicalEncode for Signature {
    #[inline]
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(self.as_bytes());
    }
}

impl CanonicalEncode for Quantum {
    #[inline]
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        self.as_u128().encode_canonical(buf);
    }
}

impl CanonicalEncode for [u8] {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        let len = self.len() as u32;
        len.encode_canonical(buf);
        buf.extend_from_slice(self);
    }
}

impl CanonicalEncode for Vec<u8> {
    #[inline]
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        self.as_slice().encode_canonical(buf);
    }
}
