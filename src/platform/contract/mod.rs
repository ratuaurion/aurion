#![forbid(unsafe_code)]

//! # Contract SDK — Unifikasi Wallet & Smart Contract VM Aurion
//!
//! Modul ini adalah lapisan perekat ("glue") yang menyatukan **Wallet** (kunci,
//! clear signing) dan **VM/Execution** (AVM, STF) di dalam satu binary
//! `/bin/aurion` (AUR-ARCH-001), sehingga interaksi kontrak menjadi otomatis:
//!
//! ```text
//! User Intent -> Contract Metadata -> Auto Tx Builder (nonce/gas)
//!            -> Local VM Simulation (Dry-Run STF) -> Wallet Clear Signing
//!            -> Auto Broadcast
//! ```
//!
//! ## Konsep inti
//! - [`Provider`]: baca state, **simulasi lokal** (dry-run replikasi STF pada
//!   clone state), dan broadcast (`RpcProvider` / `MemoryProvider`).
//! - [`Signer`]: clear signing terstruktur — intent manusiawi wajib cocok dengan
//!   transaksi sebelum tanda tangan Ed25519 ([`ContractIntent::verify_against`]).
//! - [`ContractInstance`]: mengikat alamat + metadata + Provider + Signer;
//!   [`ContractInstance::call`] menjalankan seluruh pipeline dalam satu panggilan.
//! - [`ContractMetadata`]: struktur ABI/refleksi **minimal dan aman** yang
//!   mengikat kontrak ke `code_hash` on-chain (AUR-VM-006) dengan selector
//!   Blake3 4-byte kanonikal.
//!
//! ## Batasan keras yang dipatuhi
//! - Tidak ada perubahan BFT, block production, SMT, atau storage engine.
//! - Semantik STF/AVM tidak berubah (dry-run hanya *mereplikasi* `apply_transaction`).
//! - Zero unsafe (AUR-ARCH-011), zero float (AUR-ARCH-012), payload maksimal 24 KB.

pub mod calldata;
pub mod error;
pub mod instance;
pub mod intent;
pub mod metadata;
pub mod provider;
pub mod signer;

pub use calldata::{
    encode_call_frame, encode_call_payload, encode_deploy_payload, relocate_jumps, validate_payload,
};
pub use error::ContractError;
pub use instance::{
    auto_call_fee, auto_deploy_fee, CallOptions, CallOutcome, ContractInstance, DeployOutcome,
    DeployRequest, DEFAULT_TTL_SECS, MIN_TX_FEE_QUANTA,
};
pub use intent::{ContractIntent, IntentAction};
pub use metadata::{
    format_quanta, selector_for, AbiParam, AbiType, AbiValue, ContractMetadata, MethodAbi,
    METADATA_SCHEMA, MAX_ABI_INPUTS,
};
pub use provider::{DryRunReport, MemoryProvider, Provider, RpcProvider};
pub use signer::{ApprovalMode, KeystoreSigner, Signer};
