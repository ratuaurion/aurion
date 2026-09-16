//! Modul Kriptografi Kanonikal Aurion.

pub mod bech32m;
pub mod blake3;
pub mod ed25519;

pub use bech32m::{
    decode_address_bech32m, derive_address_from_pubkey, encode_address_bech32m,
    Bech32mError, DST_ADDRESS, HRP_MAINNET, HRP_TESTNET,
};
pub use blake3::{blake3_derive_key, blake3_hash};
pub use ed25519::{ed25519_verify_strict, Ed25519Error, Keypair};

// Domain Separation Tags (DST) Universal
pub const DST_TX: &str = "AURION-TX-V1";
pub const DST_BFT_PROPOSAL: &str = "AURION-BFT-PROPOSAL-V1";
pub const DST_BFT_PREVOTE: &str = "AURION-BFT-PREVOTE-V1";
pub const DST_BFT_PRECOMMIT: &str = "AURION-BFT-PRECOMMIT-V1";
pub const DST_PROVENANCE_RPI: &str = "AURION-PROVENANCE-RPI-V1";
pub const DST_SMT_BRANCH: &str = "AURION-SMT-BRANCH-V1";
pub const DST_SMT_LEAF: &str = "AURION-SMT-LEAF-V1";
pub const DST_TX_ID: &str = "AURION-TX-ID-V1";
pub const DST_BLOCK_ID: &str = "AURION-BLOCK-ID-V1";
