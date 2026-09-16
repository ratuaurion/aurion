//! Domain Konsensus & Pengurutan Transaksi Aurion.
//! Menampung konsensus BFT single-slot finality dan mempool prioritas transaksi.

pub mod bft;
pub mod mempool;

pub use bft::*;
