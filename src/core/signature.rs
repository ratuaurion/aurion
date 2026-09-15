//! Signature: Tanda tangan digital Ed25519 kanonikal 64-byte.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Signature(pub [u8; 64]);

impl Signature {
    pub const ZERO: Self = Signature([0u8; 64]);

    pub const fn from_bytes(bytes: [u8; 64]) -> Self {
        Signature(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}
