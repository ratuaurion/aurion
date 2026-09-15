//! Struktur data akun on-chain Aurion.
//! Mematuhi Invariant AUR-VM-006 (Contract Account State Model).

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::core::{Hash256, Quantum};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Account {
    pub balance: Quantum,
    pub nonce: u64,
    pub code_hash: Option<Hash256>,
    pub storage_root: Option<Hash256>,
}

impl Account {
    pub fn new(balance: Quantum, nonce: u64) -> Self {
        Self {
            balance,
            nonce,
            code_hash: None,
            storage_root: None,
        }
    }

    pub fn new_contract(
        balance: Quantum,
        nonce: u64,
        code_hash: Hash256,
        storage_root: Hash256,
    ) -> Self {
        Self {
            balance,
            nonce,
            code_hash: Some(code_hash),
            storage_root: Some(storage_root),
        }
    }

    #[inline]
    pub fn is_contract(&self) -> bool {
        self.code_hash.is_some()
    }
}

impl CanonicalEncode for Account {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        self.balance.encode_canonical(buf);
        self.nonce.encode_canonical(buf);
        match &self.code_hash {
            Some(h) => {
                1u8.encode_canonical(buf);
                h.encode_canonical(buf);
            }
            None => 0u8.encode_canonical(buf),
        }
        match &self.storage_root {
            Some(h) => {
                1u8.encode_canonical(buf);
                h.encode_canonical(buf);
            }
            None => 0u8.encode_canonical(buf),
        }
    }
}

impl CanonicalDecode for Account {
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let balance = Quantum::decode_canonical(bytes, cursor)?;
        let nonce = u64::decode_canonical(bytes, cursor)?;
        let has_code = u8::decode_canonical(bytes, cursor)?;
        let code_hash = if has_code != 0 {
            Some(Hash256::decode_canonical(bytes, cursor)?)
        } else {
            None
        };
        let has_storage = u8::decode_canonical(bytes, cursor)?;
        let storage_root = if has_storage != 0 {
            Some(Hash256::decode_canonical(bytes, cursor)?)
        } else {
            None
        };
        Ok(Account {
            balance,
            nonce,
            code_hash,
            storage_root,
        })
    }
}
