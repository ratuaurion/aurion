//! Aurion Layer-2 (L2) Scaling & Horizon Expansion Module.
//!
//! Modul ini menyediakan fondasi eksekusi off-chain berkinerja tinggi,
//! kompresi batch transaksi, mesin status L2, sekuenser, relayer lintas-layer,
//! dan antarmuka jembatan penyelesaian (settlement bridge) ke Layer-1.
//!
//! Seluruh komponen L2 tunduk pada invariant mutlak protokol:
//! - L2-ARCH-003 / AUR-ARCH-012: Zero Floating-Point Arithmetic (Seluruh nilai kuantitas menggunakan `Quantum`).
//! - AUR-ARCH-011: Absolute Zero Unsafe Code (`#![forbid(unsafe_code)]`).
//! - L2-ARCH-005: Algoritma kriptografi standar Aurion (Blake3 dan Ed25519).

pub mod bridge;
pub mod relayer;
pub mod sequencer;
pub mod state;
pub mod types;
pub mod vm;
