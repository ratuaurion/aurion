//! Blake3: Primitif hashing dan fungsi derivasi kunci (KDF) Aurion.

use crate::core::Hash256;

/// Hitung hash Blake3 standar 256-bit (32 bytes).
#[inline]
pub fn blake3_hash(data: &[u8]) -> Hash256 {
    let digest = blake3::hash(data);
    Hash256(*digest.as_bytes())
}

/// Mode derive-key Blake3 dengan konteks domain eksplisit.
#[inline]
pub fn blake3_derive_key(context: &str, key_material: &[u8]) -> Hash256 {
    let digest = blake3::derive_key(context, key_material);
    Hash256(digest)
}
