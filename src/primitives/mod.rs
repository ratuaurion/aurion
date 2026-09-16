//! Domain Primitif Protokol & Kriptografi Kanonikal Aurion.
//! Memuat tipe data bersama (Address, Hash256, Quantum u128), kriptografi Blake3 & Ed25519,
//! serialisasi biner big-endian kanonikal, serta parameter pembentukan genesis.

pub mod codec;
pub mod core;
pub mod crypto;
pub mod genesis;
