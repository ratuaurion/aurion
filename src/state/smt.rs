//! Sparse Merkle Tree (SMT) hashing primitives menggunakan Blake3.

use crate::core::Hash256;
use crate::crypto::{blake3_derive_key, DST_SMT_BRANCH, DST_SMT_LEAF};

/// Hitung hash daun (leaf hash) dari key 32-byte dan value raw.
pub fn smt_leaf_hash(key: &[u8; 32], value: &[u8]) -> Hash256 {
    let mut payload = Vec::with_capacity(32 + value.len());
    payload.extend_from_slice(key);
    payload.extend_from_slice(value);
    blake3_derive_key(DST_SMT_LEAF, &payload)
}

/// Hitung hash cabang (branch hash) dari dua child hash 32-byte.
pub fn smt_branch_hash(left: &Hash256, right: &Hash256) -> Hash256 {
    let mut payload = [0u8; 64];
    payload[..32].copy_from_slice(left.as_bytes());
    payload[32..].copy_from_slice(right.as_bytes());
    blake3_derive_key(DST_SMT_BRANCH, &payload)
}

use crate::core::Address;
use crate::state::account::Account;
use std::collections::HashMap;

/// Hitung root state deterministik dari seluruh akun yang terdaftar.
pub fn compute_accounts_state_root(accounts: &HashMap<Address, Account>) -> Hash256 {
    if accounts.is_empty() {
        return Hash256::ZERO;
    }

    let mut sorted_addrs: Vec<&Address> = accounts.keys().collect();
    sorted_addrs.sort_by_key(|a| a.as_bytes());

    let mut current_level: Vec<Hash256> = sorted_addrs
        .into_iter()
        .map(|addr| {
            let acct = &accounts[addr];
            let mut val_bytes = [0u8; 24];
            val_bytes[..16].copy_from_slice(&acct.balance.as_u128().to_be_bytes());
            val_bytes[16..].copy_from_slice(&acct.nonce.to_be_bytes());
            smt_leaf_hash(addr.as_bytes(), &val_bytes)
        })
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

