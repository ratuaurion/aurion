//! Manajemen State dan Sparse Merkle Tree (SMT) Layer-2 Aurion.
//! Mematuhi Invariant L2-ARCH-003 (Zero-Float) dan L2-SETTLE-002 (State Commitment).

use std::collections::BTreeMap;
use crate::core::{Address, Hash256, Quantum};
use crate::crypto::blake3_hash;

/// Akun Layer-2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2Account {
    pub address: Address,
    pub balance: Quantum,
    pub nonce: u64,
}

/// Mesin Penyimpanan State L2
#[derive(Debug, Default, Clone)]
pub struct L2StateStore {
    accounts: BTreeMap<Address, L2Account>,
}

impl L2StateStore {
    /// Membuat store state L2 baru dalam kondisi kosong
    #[must_use]
    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
        }
    }

    /// Mengambil data akun jika ada
    #[must_use]
    pub fn get_account(&self, address: &Address) -> Option<&L2Account> {
        self.accounts.get(address)
    }

    /// Memperbarui atau menyimpan data akun
    pub fn set_account(&mut self, account: L2Account) {
        self.accounts.insert(account.address, account);
    }

    /// Menghitung komitmen State Root L2 secara deterministik menggunakan Blake3
    #[must_use]
    pub fn compute_state_root(&self) -> Hash256 {
        if self.accounts.is_empty() {
            return Hash256::ZERO;
        }

        let mut hasher_data = Vec::new();
        // BTreeMap menjamin urutan deterministik berdasarkan Address
        for (addr, acc) in &self.accounts {
            hasher_data.extend_from_slice(addr.as_bytes());
            hasher_data.extend_from_slice(&acc.balance.as_u128().to_be_bytes());
            hasher_data.extend_from_slice(&acc.nonce.to_be_bytes());
        }

        blake3_hash(&hasher_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2_state_store_determinism() {
        let mut store1 = L2StateStore::new();
        let mut store2 = L2StateStore::new();

        let addr_a = Address::from_bytes([1u8; 32]);
        let addr_b = Address::from_bytes([2u8; 32]);

        let acc_a = L2Account {
            address: addr_a,
            balance: Quantum::new(100_000_000),
            nonce: 0,
        };
        let acc_b = L2Account {
            address: addr_b,
            balance: Quantum::new(200_000_000),
            nonce: 5,
        };

        // Simpan dalam urutan berbeda
        store1.set_account(acc_a.clone());
        store1.set_account(acc_b.clone());

        store2.set_account(acc_b);
        store2.set_account(acc_a);

        assert_eq!(store1.compute_state_root(), store2.compute_state_root());
    }

    #[test]
    fn test_empty_store_root_is_zero() {
        let store = L2StateStore::new();
        assert_eq!(store.compute_state_root(), Hash256::ZERO);
    }
}
