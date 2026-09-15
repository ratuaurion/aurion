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
