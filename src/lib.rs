#![forbid(unsafe_code)]

//! Aurion: Ekosistem Blockchain & Mata Uang Kripto Berdaulat.
//! Sesuai Invariant AUR-ARCH-001: Single Sovereign Ecosystem.
//! Terstruktur dalam 5 Domain Fungsional Utama (Domain-Driven Architecture).

// 1. Domain Fungsional Utama Aurion (Domain-Driven Architecture)
pub mod consensus;
pub mod interop;
pub mod platform;
pub mod primitives;
pub mod scaling;
pub mod specialized;
pub mod statemachine;

// 2. Re-export Kanonikal Transparan untuk Kompatibilitas & Integrasi Ruang Kerja Penuh
pub use primitives::{codec, core, crypto, genesis};
pub use statemachine::{state, transaction, vm};
pub use consensus::mempool;
pub use scaling as l2;
pub use specialized as l3;
pub use interop as l4;
pub use platform::{cli, conformance, gateway, runtime, storage, wallet, wire};


