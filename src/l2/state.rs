//! Manajemen State dan Sparse Merkle Tree (SMT) 256-bit Layer-2 Aurion.
//! Mematuhi Invariant L2-ARCH-003 (Zero-Float Quantum), L2-SETTLE-002 (State Commitment), dan AUR-ARCH-011 (#![forbid(unsafe_code)]).

use std::collections::BTreeMap;
use crate::core::{Address, Hash256, Quantum};
use crate::state::smt::{smt_branch_hash, smt_leaf_hash};

/// Ukuran tetap representasi biner kanonikal L2Account: 32 + 16 + 8 + 32 = 88 byte
pub const L2_ACCOUNT_ENCODED_SIZE: usize = 88;

/// Akun Layer-2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2Account {
    pub address: Address,
    pub balance: Quantum,
    pub nonce: u64,
    pub storage_root: Hash256,
}

impl L2Account {
    /// Membuat akun L2 baru
    #[must_use]
    pub fn new(address: Address, balance: Quantum, nonce: u64) -> Self {
        Self {
            address,
            balance,
            nonce,
            storage_root: Hash256::ZERO,
        }
    }

    /// Menghitung komitmen hash daun (leaf hash) dari akun L2 berbasis SMT Blake3
    #[must_use]
    pub fn compute_account_hash(&self) -> Hash256 {
        let mut val_bytes = [0u8; 16 + 8 + 32];
        val_bytes[0..16].copy_from_slice(&self.balance.as_u128().to_be_bytes());
        val_bytes[16..24].copy_from_slice(&self.nonce.to_be_bytes());
        val_bytes[24..56].copy_from_slice(self.storage_root.as_bytes());
        smt_leaf_hash(self.address.as_bytes(), &val_bytes)
    }

    /// Mengodekan akun ke format biner kanonikal 88 byte Big-Endian
    #[must_use]
    pub fn encode_canonical(&self) -> [u8; L2_ACCOUNT_ENCODED_SIZE] {
        let mut buf = [0u8; L2_ACCOUNT_ENCODED_SIZE];
        buf[0..32].copy_from_slice(self.address.as_bytes());
        buf[32..48].copy_from_slice(&self.balance.as_u128().to_be_bytes());
        buf[48..56].copy_from_slice(&self.nonce.to_be_bytes());
        buf[56..88].copy_from_slice(self.storage_root.as_bytes());
        buf
    }

    /// Mendekode 88 byte biner menjadi L2Account
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != L2_ACCOUNT_ENCODED_SIZE {
            return Err("Ukuran calldata L2Account harus tepat 88 byte");
        }

        let mut addr_bytes = [0u8; 32];
        addr_bytes.copy_from_slice(&bytes[0..32]);
        let address = Address::from_bytes(addr_bytes);

        let mut bal_bytes = [0u8; 16];
        bal_bytes.copy_from_slice(&bytes[32..48]);
        let balance = Quantum::new(u128::from_be_bytes(bal_bytes));

        let mut nonce_bytes = [0u8; 8];
        nonce_bytes.copy_from_slice(&bytes[48..56]);
        let nonce = u64::from_be_bytes(nonce_bytes);

        let mut storage_root_bytes = [0u8; 32];
        storage_root_bytes.copy_from_slice(&bytes[56..88]);
        let storage_root = Hash256::from_bytes(storage_root_bytes);

        Ok(Self {
            address,
            balance,
            nonce,
            storage_root,
        })
    }
}

/// Bukti Kriptografis Keberadaan Akun di SMT L2 (Membership Proof)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2AccountProof {
    pub address: Address,
    pub account_hash: Hash256,
    pub leaf_index: usize,
    pub siblings: Vec<Hash256>,
    pub root: Hash256,
}

impl L2AccountProof {
    /// Memverifikasi keabsahan bukti SMT terhadap root state yang dikomitkan
    #[must_use]
    pub fn verify(&self) -> bool {
        let mut current = self.account_hash;
        let mut idx = self.leaf_index;

        for sibling in &self.siblings {
            if idx.is_multiple_of(2) {
                current = smt_branch_hash(&current, sibling);
            } else {
                current = smt_branch_hash(sibling, &current);
            }
            idx /= 2;
        }

        current == self.root
    }
}

/// Mesin Penyimpanan State L2 Berbasis Sparse Merkle Tree
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

    /// Menghitung komitmen State Root L2 secara deterministik menggunakan SMT Blake3
    #[must_use]
    pub fn compute_state_root(&self) -> Hash256 {
        if self.accounts.is_empty() {
            return Hash256::ZERO;
        }

        // BTreeMap menjamin urutan deterministik berdasarkan Address
        let mut current_level: Vec<Hash256> = self
            .accounts
            .values()
            .map(L2Account::compute_account_hash)
            .collect();

        while current_level.len() > 1 {
            let mut next_level = Vec::with_capacity(current_level.len().div_ceil(2));
            for chunk in current_level.chunks(2) {
                let left = &chunk[0];
                let right = if chunk.len() > 1 { &chunk[1] } else { &chunk[0] };
                next_level.push(smt_branch_hash(left, right));
            }
            current_level = next_level;
        }

        current_level[0]
    }

    /// Menghasilkan bukti inklusi Merkle (Membership Proof) untuk akun tertentu
    pub fn generate_account_proof(&self, address: &Address) -> Result<L2AccountProof, &'static str> {
        let account = self
            .accounts
            .get(address)
            .ok_or("Akun tidak ditemukan dalam state store")?;

        let account_hash = account.compute_account_hash();
        let target_idx = self
            .accounts
            .keys()
            .position(|k| k == address)
            .ok_or("Posisi akun tidak ditemukan")?;

        let mut current_level: Vec<Hash256> = self
            .accounts
            .values()
            .map(L2Account::compute_account_hash)
            .collect();

        let mut siblings = Vec::new();
        let mut idx = target_idx;

        while current_level.len() > 1 {
            let sibling_idx = if idx.is_multiple_of(2) {
                if idx + 1 < current_level.len() {
                    idx + 1
                } else {
                    idx
                }
            } else {
                idx - 1
            };

            siblings.push(current_level[sibling_idx]);

            let mut next_level = Vec::with_capacity(current_level.len().div_ceil(2));
            for chunk in current_level.chunks(2) {
                let left = &chunk[0];
                let right = if chunk.len() > 1 { &chunk[1] } else { &chunk[0] };
                next_level.push(smt_branch_hash(left, right));
            }
            current_level = next_level;
            idx /= 2;
        }

        let root = current_level[0];

        Ok(L2AccountProof {
            address: *address,
            account_hash,
            leaf_index: target_idx,
            siblings,
            root,
        })
    }

    /// Jumlah akun aktif dalam state store
    #[must_use]
    pub fn len(&self) -> usize {
        self.accounts.len()
    }

    /// Status kosong
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2_account_canonical_roundtrip() {
        let account = L2Account {
            address: Address::from_bytes([0x77; 32]),
            balance: Quantum::new(500_000_000),
            nonce: 12,
            storage_root: Hash256::from_bytes([0x88; 32]),
        };

        let encoded = account.encode_canonical();
        assert_eq!(encoded.len(), L2_ACCOUNT_ENCODED_SIZE);

        let decoded = L2Account::decode_canonical(&encoded).expect("Decode L2Account gagal");
        assert_eq!(decoded, account);
        assert_eq!(decoded.compute_account_hash(), account.compute_account_hash());
    }

    #[test]
    fn test_l2_state_store_determinism_and_membership_proof() {
        let mut store1 = L2StateStore::new();
        let mut store2 = L2StateStore::new();

        let addr_a = Address::from_bytes([1u8; 32]);
        let addr_b = Address::from_bytes([2u8; 32]);
        let addr_c = Address::from_bytes([3u8; 32]);

        let acc_a = L2Account::new(addr_a, Quantum::new(100_000_000), 0);
        let acc_b = L2Account::new(addr_b, Quantum::new(200_000_000), 5);
        let acc_c = L2Account::new(addr_c, Quantum::new(300_000_000), 9);

        // Masukkan dalam urutan acak
        store1.set_account(acc_a.clone());
        store1.set_account(acc_b.clone());
        store1.set_account(acc_c.clone());

        store2.set_account(acc_c);
        store2.set_account(acc_a);
        store2.set_account(acc_b);

        // State root harus deterministik dan identik
        let root1 = store1.compute_state_root();
        let root2 = store2.compute_state_root();
        assert_eq!(root1, root2);
        assert_ne!(root1, Hash256::ZERO);

        // Hasilkan bukti inklusi untuk akun B
        let proof_b = store1.generate_account_proof(&addr_b).expect("Generate proof gagal");
        assert_eq!(proof_b.address, addr_b);
        assert_eq!(proof_b.root, root1);
        assert!(proof_b.verify());

        // Bukti palsu harus ditolak
        let mut bad_proof = proof_b.clone();
        bad_proof.account_hash = Hash256::from_bytes([0xFF; 32]);
        assert!(!bad_proof.verify());
    }

    #[test]
    fn test_empty_store_root_is_zero() {
        let store = L2StateStore::new();
        assert_eq!(store.compute_state_root(), Hash256::ZERO);
    }
}
