#![forbid(unsafe_code)]

//! Modul Aurion Layer-3 Specialized Networks (`aurion::specialized`).
//! Mematuhi Invariant Rule 18 (L3 Ecosystem Expansion Blueprint),
//! AUR-ARCH-001 (Single Sovereign Ecosystem), AUR-ARCH-011 (#![forbid(unsafe_code)]),
//! AUR-ARCH-012 (Zero-Float Quantum u128), AUR-L3-ARCH-001 (L1 Sovereignty Root),
//! dan AUR-L3-SEC-001 (Domain Fault Isolation).

pub mod state;
pub mod types;

pub use state::{L3AccountProof, L3AccountState, L3State, L3_ACCOUNT_ENCODED_SIZE};
pub use types::{
    DomainId, DomainMetadata, L3Block, L3Checkpoint, L3CodecError, L3Receipt, L3SecurityModel,
    L3Transaction, DST_L3_CHECKPOINT, DST_L3_TX, L3_BLOCK_HEADER_SIZE, L3_CHECKPOINT_BASE_SIZE,
    L3_RECEIPT_BASE_SIZE, L3_TX_BASE_SIZE, MAX_L3_PROOF_SIZE, MAX_L3_TX_PAYLOAD_SIZE,
};
