//! Bech32m: Encoding dan decoding alamat akun kanonikal Aurion.

use crate::core::Address;
use crate::crypto::blake3::blake3_derive_key;
use bech32::{primitives::decode::CheckedHrpstring, Bech32m, Hrp};
use thiserror::Error;

pub const DST_ADDRESS: &str = "AURION-ADDRESS-V1";
pub const HRP_MAINNET: &str = "aur";
pub const HRP_TESTNET: &str = "aurt";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Bech32mError {
    #[error("Invalid Bech32m format: {0}")]
    InvalidFormat(String),
    #[error("Invalid address payload length: expected 32, got {0}")]
    InvalidLength(usize),
    #[error("Mismatched HRP: expected {expected}, got {found}")]
    MismatchedHrp { expected: String, found: String },
}

/// Derivasi alamat kanonikal raw (32 bytes) dari kunci publik menggunakan Blake3 derive-key.
pub fn derive_address_from_pubkey(pubkey: &[u8; 32]) -> Address {
    let digest = blake3_derive_key(DST_ADDRESS, pubkey);
    Address(digest.0)
}

/// Encode alamat ke representasi Bech32m resmi.
pub fn encode_address_bech32m(address: &Address, hrp_str: &str) -> Result<String, Bech32mError> {
    let hrp = Hrp::parse(hrp_str).map_err(|e| Bech32mError::InvalidFormat(e.to_string()))?;
    bech32::encode::<Bech32m>(hrp, address.as_bytes())
        .map_err(|e| Bech32mError::InvalidFormat(e.to_string()))
}

/// Decode string alamat Bech32m ke alamat kanonikal 32-byte.
pub fn decode_address_bech32m(encoded: &str, expected_hrp: &str) -> Result<Address, Bech32mError> {
    let parsed = CheckedHrpstring::new::<Bech32m>(encoded)
        .map_err(|e| Bech32mError::InvalidFormat(e.to_string()))?;

    if parsed.hrp().as_str() != expected_hrp {
        return Err(Bech32mError::MismatchedHrp {
            expected: expected_hrp.to_string(),
            found: parsed.hrp().as_str().to_string(),
        });
    }

    let byte_iter = parsed.byte_iter();
    let bytes: Vec<u8> = byte_iter.collect();
    if bytes.len() != 32 {
        return Err(Bech32mError::InvalidLength(bytes.len()));
    }

    let mut raw = [0u8; 32];
    raw.copy_from_slice(&bytes);
    Ok(Address(raw))
}
