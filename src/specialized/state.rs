//! Manajemen State & Blake3 Sparse Merkle Tree (SMT) Layer-3 Specialized Networks.
//! Mematuhi Invariant AUR-ARCH-011 (#![forbid(unsafe_code)]), AUR-ARCH-012 (Zero-Float Quantum u128),
//! AUR-L3-STATE-001 (SMT Root 256-bit), AUR-L3-STATE-002 (State Witness Availability),
//! dan AUR-L3-SEC-001 (Domain Fault Isolation & Atomic Rollback).

use crate::core::{Address, Hash256, Quantum};
use crate::specialized::types::DomainId;
use crate::state::smt::{smt_branch_hash, smt_leaf_hash};
use std::collections::BTreeMap;

/// Ukuran kanonikal representasi biner L3AccountState: 32 + 16 + 8 + 32 + 4 = 92 byte
pub const L3_ACCOUNT_ENCODED_SIZE: usize = 92;

/// State Akun dalam Domain Terspesialisasi (L3)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L3AccountState {
    pub address: Address,
    pub balance: Quantum,
    pub nonce: u64,
    pub storage_root: Hash256,
    pub domain_flags: u32,
}

impl L3AccountState {
    /// Membuat state akun L3 baru
    #[must_use]
    pub fn new(address: Address, balance: Quantum, nonce: u64) -> Self {
        Self {
            address,
            balance,
            nonce,
            storage_root: Hash256::ZERO,
            domain_flags: 0,
        }
    }

    /// Menghitung hash daun (leaf hash) berbasis Blake3 SMT
    #[must_use]
    pub fn compute_account_hash(&self) -> Hash256 {
        let mut val_bytes = [0u8; 16 + 8 + 32 + 4];
        val_bytes[0..16].copy_from_slice(&self.balance.as_u128().to_be_bytes());
        val_bytes[16..24].copy_from_slice(&self.nonce.to_be_bytes());
        val_bytes[24..56].copy_from_slice(self.storage_root.as_bytes());
        val_bytes[56..60].copy_from_slice(&self.domain_flags.to_be_bytes());
        smt_leaf_hash(self.address.as_bytes(), &val_bytes)
    }

    /// Mengodekan akun ke format biner kanonikal 92-byte Big-Endian
    #[must_use]
    pub fn encode_canonical(&self) -> [u8; L3_ACCOUNT_ENCODED_SIZE] {
        let mut buf = [0u8; L3_ACCOUNT_ENCODED_SIZE];
        buf[0..32].copy_from_slice(self.address.as_bytes());
        buf[32..48].copy_from_slice(&self.balance.as_u128().to_be_bytes());
        buf[48..56].copy_from_slice(&self.nonce.to_be_bytes());
        buf[56..88].copy_from_slice(self.storage_root.as_bytes());
        buf[88..92].copy_from_slice(&self.domain_flags.to_be_bytes());
        buf
    }

    /// Mendekode format biner kanonikal 92-byte menjadi L3AccountState
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != L3_ACCOUNT_ENCODED_SIZE {
            return Err("Ukuran data biner L3AccountState harus tepat 92 byte");
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

        let mut flags_bytes = [0u8; 4];
        flags_bytes.copy_from_slice(&bytes[88..92]);
        let domain_flags = u32::from_be_bytes(flags_bytes);

        Ok(Self {
            address,
            balance,
            nonce,
            storage_root,
            domain_flags,
        })
    }
}

/// Bukti Kriptografis Inklusi Akun di SMT L3 (State Witness)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L3AccountProof {
    pub address: Address,
    pub account_hash: Hash256,
    pub leaf_index: usize,
    pub siblings: Vec<Hash256>,
    pub root: Hash256,
}

impl L3AccountProof {
    /// Memverifikasi keabsahan bukti SMT terhadap root state L3
    #[must_use]
    pub fn verify(&self) -> bool {
        let mut current = self.account_hash;
        let mut idx = self.leaf_index;

        for sibling in &self.siblings {
            current = if idx.is_multiple_of(2) {
                smt_branch_hash(&current, sibling)
            } else {
                smt_branch_hash(sibling, &current)
            };
            idx /= 2;
        }

        current == self.root
    }
}

/// Tipe data snapshot state untuk rollback transaksi atomik L3
pub type L3StateSnapshot = (
    BTreeMap<Address, L3AccountState>,
    BTreeMap<(Address, Hash256), Hash256>,
);

/// Penyimpan State Terisolasi untuk Domain Terspesialisasi L3
#[derive(Debug, Clone)]
pub struct L3State {
    pub domain_id: DomainId,
    pub block_number: u64,
    accounts: BTreeMap<Address, L3AccountState>,
    storage: BTreeMap<(Address, Hash256), Hash256>,
    snapshots: Vec<L3StateSnapshot>,
}

impl L3State {
    /// Membuat instance state baru untuk domain L3
    #[must_use]
    pub fn new(domain_id: DomainId) -> Self {
        Self {
            domain_id,
            block_number: 0,
            accounts: BTreeMap::new(),
            storage: BTreeMap::new(),
            snapshots: Vec::new(),
        }
    }

    /// Mengambil state akun jika ada
    #[must_use]
    pub fn get_account(&self, address: &Address) -> Option<&L3AccountState> {
        self.accounts.get(address)
    }

    /// Mengambil saldo akun (0 jika belum terdaftar)
    #[must_use]
    pub fn get_balance(&self, address: &Address) -> Quantum {
        self.accounts
            .get(address)
            .map_or(Quantum::ZERO, |a| a.balance)
    }

    /// Mengambil nonce akun (0 jika belum terdaftar)
    #[must_use]
    pub fn get_nonce(&self, address: &Address) -> u64 {
        self.accounts.get(address).map_or(0, |a| a.nonce)
    }

    /// Menambah saldo akun secara aman (Zero-Float)
    pub fn credit(&mut self, address: &Address, amount: Quantum) -> Result<(), &'static str> {
        let acct = self
            .accounts
            .entry(*address)
            .or_insert_with(|| L3AccountState::new(*address, Quantum::ZERO, 0));
        acct.balance = acct
            .balance
            .checked_add(amount)
            .map_err(|_| "Overflow saldo akun L3")?;
        Ok(())
    }

    /// Mengurangi saldo akun secara aman (Zero-Float)
    pub fn debit(&mut self, address: &Address, amount: Quantum) -> Result<(), &'static str> {
        let acct = self
            .accounts
            .get_mut(address)
            .ok_or("Akun L3 tidak ditemukan untuk didebit")?;
        acct.balance = acct
            .balance
            .checked_sub(amount)
            .map_err(|_| "Saldo akun L3 tidak mencukupi")?;
        Ok(())
    }

    /// Menaikkan nonce akun sebesar 1
    pub fn increment_nonce(&mut self, address: &Address) -> Result<u64, &'static str> {
        let acct = self
            .accounts
            .entry(*address)
            .or_insert_with(|| L3AccountState::new(*address, Quantum::ZERO, 0));
        acct.nonce = acct.nonce.checked_add(1).ok_or("Overflow nonce akun L3")?;
        Ok(acct.nonce)
    }

    /// Mengambil nilai storage kontrak pada slot tertentu
    #[must_use]
    pub fn get_storage(&self, address: &Address, key: &Hash256) -> Hash256 {
        self.storage
            .get(&(*address, *key))
            .copied()
            .unwrap_or(Hash256::ZERO)
    }

    /// Menulis nilai storage kontrak pada slot tertentu dan memperbarui storage_root
    pub fn set_storage(&mut self, address: &Address, key: Hash256, value: Hash256) {
        if value == Hash256::ZERO {
            self.storage.remove(&(*address, key));
        } else {
            self.storage.insert((*address, key), value);
        }

        // Perbarui storage_root akun
        let leaf = smt_leaf_hash(key.as_bytes(), value.as_bytes());
        if let Some(acct) = self.accounts.get_mut(address) {
            acct.storage_root = leaf;
        } else {
            let mut acct = L3AccountState::new(*address, Quantum::ZERO, 0);
            acct.storage_root = leaf;
            self.accounts.insert(*address, acct);
        }
    }

    /// Membuat checkpoint snapshot state untuk isolasi transaksi & rollback
    pub fn snapshot(&mut self) -> usize {
        let id = self.snapshots.len();
        self.snapshots
            .push((self.accounts.clone(), self.storage.clone()));
        id
    }

    /// Mengembalikan state ke snapshot tertentu jika terjadi revert / kegagalan
    pub fn revert_to_snapshot(&mut self, snapshot_id: usize) -> Result<(), &'static str> {
        if snapshot_id >= self.snapshots.len() {
            return Err("Snapshot ID L3 tidak valid");
        }
        let (saved_accounts, saved_storage) = self.snapshots.remove(snapshot_id);
        self.accounts = saved_accounts;
        self.storage = saved_storage;
        self.snapshots.truncate(snapshot_id);
        Ok(())
    }

    /// Mengonfirmasi dan menghapus snapshot (transaksi berhasil dieksekusi)
    pub fn commit_snapshot(&mut self, snapshot_id: usize) -> Result<(), &'static str> {
        if snapshot_id >= self.snapshots.len() {
            return Err("Snapshot ID L3 tidak valid");
        }
        self.snapshots.truncate(snapshot_id);
        Ok(())
    }

    /// Menghitung State Root Blake3 SMT kanonikal dari seluruh akun domain L3
    #[must_use]
    pub fn compute_state_root(&self) -> Hash256 {
        if self.accounts.is_empty() {
            return Hash256::ZERO;
        }

        let mut current_level: Vec<Hash256> = self
            .accounts
            .values()
            .map(L3AccountState::compute_account_hash)
            .collect();

        while current_level.len() > 1 {
            let mut next_level = Vec::with_capacity(current_level.len().div_ceil(2));
            for chunk in current_level.chunks(2) {
                let left = &chunk[0];
                let right = if chunk.len() > 1 {
                    &chunk[1]
                } else {
                    &chunk[0]
                };
                next_level.push(smt_branch_hash(left, right));
            }
            current_level = next_level;
        }

        current_level[0]
    }

    /// Menghasilkan bukti inklusi akun (State Witness Proof) untuk verifikasi di L2
    #[must_use]
    pub fn generate_account_proof(&self, address: &Address) -> Option<L3AccountProof> {
        let target_acct = self.accounts.get(address)?;
        let target_hash = target_acct.compute_account_hash();

        let addrs: Vec<&Address> = self.accounts.keys().collect();
        let target_idx = addrs.iter().position(|&a| a == address)?;

        let mut current_hashes: Vec<Hash256> = self
            .accounts
            .values()
            .map(L3AccountState::compute_account_hash)
            .collect();

        let mut siblings = Vec::new();
        let mut idx = target_idx;

        while current_hashes.len() > 1 {
            let sibling_idx = if idx.is_multiple_of(2) {
                if idx + 1 < current_hashes.len() {
                    idx + 1
                } else {
                    idx // duplikasi jika ganjil
                }
            } else {
                idx - 1
            };

            siblings.push(current_hashes[sibling_idx]);

            let mut next_hashes = Vec::with_capacity(current_hashes.len().div_ceil(2));
            for chunk in current_hashes.chunks(2) {
                let left = &chunk[0];
                let right = if chunk.len() > 1 {
                    &chunk[1]
                } else {
                    &chunk[0]
                };
                next_hashes.push(smt_branch_hash(left, right));
            }
            current_hashes = next_hashes;
            idx /= 2;
        }

        Some(L3AccountProof {
            address: *address,
            account_hash: target_hash,
            leaf_index: target_idx,
            siblings,
            root: current_hashes[0],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l3_account_state_canonical_roundtrip() {
        let addr = Address::from_bytes([0x42; 32]);
        let acct = L3AccountState {
            address: addr,
            balance: Quantum::new(1_000_000_000), // 10 AUR
            nonce: 15,
            storage_root: Hash256::from_bytes([0x77; 32]),
            domain_flags: 0x0001_0002,
        };

        let encoded = acct.encode_canonical();
        assert_eq!(encoded.len(), L3_ACCOUNT_ENCODED_SIZE);

        let decoded = L3AccountState::decode_canonical(&encoded).unwrap();
        assert_eq!(acct, decoded);
        assert_eq!(acct.compute_account_hash(), decoded.compute_account_hash());
    }

    #[test]
    fn test_l3_state_credit_debit_and_rollback() {
        let domain_id = DomainId::DEX_DEFAULT;
        let mut state = L3State::new(domain_id);
        let alice = Address::from_bytes([0x01; 32]);

        state.credit(&alice, Quantum::new(500)).unwrap();
        assert_eq!(state.get_balance(&alice), Quantum::new(500));

        // Buat snapshot sebelum transaksi baru
        let snap_id = state.snapshot();

        // Operasi yang nanti akan di-revert
        state.debit(&alice, Quantum::new(200)).unwrap();
        assert_eq!(state.get_balance(&alice), Quantum::new(300));
        state.increment_nonce(&alice).unwrap();
        assert_eq!(state.get_nonce(&alice), 1);

        // Revert snapshot
        state.revert_to_snapshot(snap_id).unwrap();
        assert_eq!(state.get_balance(&alice), Quantum::new(500));
        assert_eq!(state.get_nonce(&alice), 0);
    }

    #[test]
    fn test_l3_state_smt_root_and_membership_proof() {
        let domain_id = DomainId::APP_CHAIN_DEFAULT;
        let mut state = L3State::new(domain_id);

        let addr1 = Address::from_bytes([0x10; 32]);
        let addr2 = Address::from_bytes([0x20; 32]);
        let addr3 = Address::from_bytes([0x30; 32]);

        state.credit(&addr1, Quantum::new(100)).unwrap();
        state.credit(&addr2, Quantum::new(200)).unwrap();
        state.credit(&addr3, Quantum::new(300)).unwrap();

        let root = state.compute_state_root();
        assert_ne!(root, Hash256::ZERO);

        // Verifikasi membership proof untuk setiap akun
        for addr in [&addr1, &addr2, &addr3] {
            let proof = state
                .generate_account_proof(addr)
                .expect("Proof harus berhasil dibuat");
            assert_eq!(proof.root, root);
            assert!(
                proof.verify(),
                "Proof verification harus lolos untuk {:?}",
                addr
            );
        }
    }
}
