//! Domain Mesin Status (StateMachine) & Eksekusi Aurion.
//! Memuat ledger status akun, mutasi saldo moneter, mesin eksekusi Aurion VM (AVM),
//! serta pipeline penerimaan dan verifikasi transaksi berdaulat.

pub mod state;
pub mod transaction;
pub mod vm;
