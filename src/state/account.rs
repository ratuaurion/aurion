//! Struktur data akun on-chain Aurion.

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::core::Quantum;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Account {
    pub balance: Quantum,
    pub nonce: u64,
}

impl Account {
    pub fn new(balance: Quantum, nonce: u64) -> Self {
        Self { balance, nonce }
    }
}

impl CanonicalEncode for Account {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        self.balance.encode_canonical(buf);
        self.nonce.encode_canonical(buf);
    }
}

impl CanonicalDecode for Account {
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let balance = Quantum::decode_canonical(bytes, cursor)?;
        let nonce = u64::decode_canonical(bytes, cursor)?;
        Ok(Account { balance, nonce })
    }
}
