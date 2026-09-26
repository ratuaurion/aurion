//! Sandbox State Transition Aurion (STF read-only ter-sandbox).
//!
//! Modul ini adalah **satu-satunya** implementasi pola
//! `Clone State -> Apply Transaction -> Extract Gas/Return Data`
//! (AUR-ARCH-004: dilarang membuat implementasi protokol independen).
//!
//! Dua consumer **wajib menghasilkan hasil identik**:
//! - [`crate::platform::contract`] -- dry-run lokal Contract SDK pra-signing, dan
//! - [`crate::platform::gateway`] -- endpoint `aur_call` / `aur_estimateGas`.
//!
//! Jaminan keras:
//! - **Zero mutasi state nyata.** `dry_run` hanya menulis ke `HashMap` salinan
//!   berukuran konstan; peta akun asli hanya dibaca.
//! - **Gas limit kanonik 1.000.000** (`AUR-VM-003`), identik dengan STF sehingga
//!   hasil simulasi sama dengan hasil eksekusi blok.
//! - **Zero float** (AUR-ARCH-012): seluruh metrik berupa integer `u64`/`u128`.

use std::collections::HashMap;

use thiserror::Error;

use crate::core::{Address, Hash256, Quantum};
use crate::state::account::Account;
use crate::state::monetary::MonetaryState;
use crate::state::stf::{apply_transaction, derive_contract_address};
use crate::transaction::types::{Transaction, TxType};
use crate::vm::context::ExecutionContext;
use crate::vm::engine::{AvmEngine, ExecutionResult};
use crate::vm::verifier::BytecodeVerifier;

/// Batas gas eksekusi kontrak persis seperti pada STF (`AUR-VM-003`).
pub const SANDBOX_GAS_LIMIT: u64 = 1_000_000;

/// Kesalahan simulasi sandbox (bukan kegagalan infrastruktur provider).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SandboxError {
    /// Bytecode gagal verifikasi statis (`AUR-VM-005`).
    #[error("Verifikasi bytecode AVM gagal: {0}")]
    Verification(String),
    /// Kontrak melakukan `REVERT`.
    #[error("kontrak revert: {0}")]
    Revert(String),
    /// Gas habis sebelum eksekusi selesai.
    #[error("kontrak kehabisan gas")]
    OutOfGas,
    /// Kesalahan lain mesin AVM.
    #[error("{0}")]
    Execution(String),
}

/// Hasil simulasi satu transaksi terhadap STF pada state sandbox.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DryRunReport {
    /// Transaksi berhasil diterapkan pada state sandbox (tidak revert).
    pub success: bool,
    /// Gas terpakai hasil replikasi konteks eksekusi STF.
    pub gas_used: u64,
    /// Data kembalian VM (`RETURN`).
    pub return_data: Vec<u8>,
    /// Alasan kegagalan STF bila `success == false`.
    pub reason: Option<String>,
    /// Alamat kontrak yang diprediksi lahir (deploy saja).
    pub deployed_contract: Option<Address>,
    /// Jumlah perubahan storage yang dihasilkan (informasional).
    pub storage_changes: usize,
}

impl DryRunReport {
    /// Ringkasan satu baris untuk prompt clear signing.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.success {
            format!(
                "SUKSES, gas {}, return 0x{}",
                self.gas_used,
                hex::encode(&self.return_data)
            )
        } else {
            format!(
                "GAGAL: {}",
                self.reason.as_deref().unwrap_or("tanpa keterangan")
            )
        }
    }
}

/// Bangun snapshot akun sandbox berukuran **konstan** (`O(1)`).
///
/// Hanya akun `sender` dan `recipient` yang disalin karena `apply_transaction`
/// tidak pernah menyentuh akun lain. Ini pertahanan anti-DoS: kloning seluruh
/// peta akun (O(n)) per permintaan `aur_call` akan menjadi vektor serang.
/// Alamat yang tidak ada dipetakan ke akun default, identik dengan perilaku
/// `get_account` pada kedua Provider.
///
/// # Inputs
/// - `accounts`: peta akun **hanya dibaca** (state nyata tidak pernah dimutasi).
/// - `tx`: transaksi yang akan disimulasikan.
///
/// # Outputs
/// Peta akun baru berisi maksimal dua entri; peta `accounts` tidak berubah.
#[must_use]
pub fn snapshot_accounts(
    accounts: &HashMap<Address, Account>,
    tx: &Transaction,
) -> HashMap<Address, Account> {
    let mut snapshot = HashMap::with_capacity(2);
    snapshot.insert(
        tx.sender,
        accounts.get(&tx.sender).cloned().unwrap_or_default(),
    );
    if tx.recipient != tx.sender {
        snapshot.insert(
            tx.recipient,
            accounts.get(&tx.recipient).cloned().unwrap_or_default(),
        );
    }
    snapshot
}

/// Simulasi read-only: replikasi **persis** `apply_transaction` pada snapshot
/// akun sandbox, lalu ekstrak gas, data kembalian, dan alamat kontrak baru.
///
/// State asli tidak pernah dimutasi; seluruh efek hanya terjadi pada salinan
/// lokal yang dibuang ketika fungsi selesai. Kegagalan STF dilaporkan sebagai
/// `success == false` (bukan `Err`) agar pemanggil dapat menampilkannya.
///
/// Gas yang dilaporkan adalah hasil eksekusi AVM dengan konteks identik STF
/// (gas limit 1.000.000, storage kosong, block 0, timestamp 0).
///
/// # Inputs
/// - `accounts`: peta akun on-chain (hanya dibaca).
/// - `tx`: transaksi kanonikal yang akan disimulasikan.
///
/// # Outputs
/// Laporan simulasi lengkap dengan gas, data kembalian, dan hasil deploy.
#[must_use]
pub fn dry_run(accounts: &HashMap<Address, Account>, tx: &Transaction) -> DryRunReport {
    let mut snapshot = snapshot_accounts(accounts, tx);
    let mut monetary = MonetaryState::default();
    let proposer = tx.sender;
    let gas_used = estimate_gas(tx).unwrap_or(0);

    match apply_transaction(&mut snapshot, &mut monetary, &proposer, tx) {
        Ok(receipt) => DryRunReport {
            success: true,
            gas_used,
            return_data: receipt.return_data,
            reason: None,
            deployed_contract: receipt.deployed_contract,
            storage_changes: receipt.storage_changes.len(),
        },
        Err(e) => DryRunReport {
            success: false,
            gas_used: 0,
            return_data: Vec::new(),
            reason: Some(e.to_string()),
            deployed_contract: None,
            storage_changes: 0,
        },
    }
}

/// Estimasi gas dengan **konteks eksekusi identik STF** (gas limit 1.000.000,
/// storage kosong, block 0, timestamp 0). Tipe non-kontrak menghasilkan `Ok(0)`.
///
/// # Inputs
/// - `tx`: transaksi kanonikal yang akan diestimasi.
///
/// # Outputs
/// - `Ok(gas)`: gas terpakai saat bytecode berhasil dijalankan.
/// - `Err(SandboxError)`: verifikasi gagal, revert, out-of-gas, atau error AVM.
///
/// # Errors
/// Bytecode gagal verifikasi, kontrak revert, kehabisan gas, atau error AVM.
pub fn estimate_gas(tx: &Transaction) -> Result<u64, SandboxError> {
    match tx.tx_type {
        TxType::ContractDeploy | TxType::ContractCall => {}
        _ => return Ok(0),
    }
    let verified = BytecodeVerifier::verify(&tx.payload)
        .map_err(|e| SandboxError::Verification(e.to_string()))?;
    let target = match tx.tx_type {
        TxType::ContractDeploy => derive_contract_address(&tx.sender, tx.nonce),
        _ => tx.recipient,
    };
    let ctx = ExecutionContext::new(
        tx.sender,
        target,
        tx.sender,
        tx.amount,
        SANDBOX_GAS_LIMIT,
        0,
        0,
    );
    let empty_storage = HashMap::new();
    match AvmEngine::execute(&verified, ctx, &empty_storage) {
        ExecutionResult::Success { gas_used, .. } => Ok(gas_used),
        ExecutionResult::Revert { reason, .. } => Err(SandboxError::Revert(reason)),
        ExecutionResult::OutOfGas => Err(SandboxError::OutOfGas),
        ExecutionResult::Error(e) => Err(SandboxError::Execution(e)),
    }
}

/// Digest Blake3 payload kontrak -- dipakai sebagai `code_hash` binding pada
/// intent clear-signing dan registry metadata off-chain.
///
/// # Inputs
/// - `payload`: payload transaksi (konstruktor saat deploy).
///
/// # Outputs
/// Digest 32-byte `blake3(payload)`.
#[must_use]
pub fn payload_code_hash(payload: &[u8]) -> Hash256 {
    crate::crypto::blake3_hash(payload)
}

/// Total biaya minimum (`amount + fee`) yang ditahan STF sebelum eksekusi.
///
/// # Inputs
/// - `tx`: transaksi kanonikal.
///
/// # Outputs
/// - `Ok(n)`: total Quanta yang dibutuhkan.
/// - `Err(String)`: overflow aritmetika `Quantum` (u128).
///
/// # Errors
/// Overflow pada penjumlahan `amount + fee`.
pub fn required_balance(tx: &Transaction) -> Result<Quantum, String> {
    tx.amount.checked_add(tx.fee).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Signature;
    use crate::vm::opcode::Opcode;

    fn contract_call(sender: Address, recipient: Address, payload: Vec<u8>) -> Transaction {
        Transaction {
            version: 1,
            chain_id: 1001,
            tx_type: TxType::ContractCall,
            flags: 0,
            sender,
            recipient,
            nonce: 0,
            amount: Quantum::ZERO,
            fee: Quantum::new(10_000),
            valid_until: 0,
            payload,
            signature: Signature::ZERO,
        }
    }

    /// Runtime AVM: `MSTORE(0, 42); RETURN(0, 32)` -> kembalian 0x..2A.
    fn echo_runtime() -> Vec<u8> {
        vec![
            Opcode::Push1 as u8,
            0x2A,
            Opcode::Push1 as u8,
            0x00,
            Opcode::MStore as u8,
            Opcode::Push1 as u8,
            0x20,
            Opcode::Push1 as u8,
            0x00,
            Opcode::Return as u8,
        ]
    }

    #[test]
    fn dry_run_does_not_mutate_source_state() {
        let sender = Address::from_bytes([0x11; 32]);
        let contract = Address::from_bytes([0x22; 32]);
        let code_hash = crate::crypto::blake3_hash(b"constructor");
        let mut accounts = HashMap::new();
        accounts.insert(sender, Account::new(Quantum::new(1_000_000_000), 0));
        accounts.insert(
            contract,
            Account::new_contract(Quantum::ZERO, 0, code_hash, Hash256::ZERO),
        );
        let before = accounts.clone();

        let tx = contract_call(sender, contract, echo_runtime());
        let report = dry_run(&accounts, &tx);

        assert!(report.success, "reason: {:?}", report.reason);
        assert_eq!(accounts, before, "state nyata wajib tidak termutasi");
        assert_eq!(report.return_data.len(), 32);
        assert_eq!(report.return_data[31], 0x2A);
    }

    #[test]
    fn dry_run_reports_failure_instead_of_erroring() {
        let sender = Address::from_bytes([0x33; 32]);
        let plain = Address::from_bytes([0x44; 32]);
        let mut accounts = HashMap::new();
        accounts.insert(sender, Account::new(Quantum::new(1_000_000_000), 0));
        accounts.insert(plain, Account::new(Quantum::new(1_000), 0));
        let before = accounts.clone();

        // Rekening tujuan bukan kontrak -> STF menolak dengan NotAContract.
        let tx = contract_call(sender, plain, echo_runtime());
        let report = dry_run(&accounts, &tx);

        assert!(!report.success);
        assert!(report.reason.is_some());
        assert_eq!(accounts, before, "state nyata tetap utuh");
    }

    #[test]
    fn estimate_gas_is_zero_for_plain_transfer() {
        let mut tx = contract_call(Address::ZERO, Address::ZERO, vec![]);
        tx.tx_type = TxType::Transfer;
        assert_eq!(estimate_gas(&tx), Ok(0));
    }

    #[test]
    fn estimate_gas_rejects_invalid_bytecode() {
        let sender = Address::from_bytes([0x55; 32]);
        let contract = Address::from_bytes([0x66; 32]);
        // 0xFF tidak dipetakan ke opcode AVM mana pun (0xFE = `Invalid`).
        let tx = contract_call(sender, contract, vec![0xFF]);
        assert!(matches!(
            estimate_gas(&tx),
            Err(SandboxError::Verification(_))
        ));
    }

    #[test]
    fn snapshot_is_bounded_to_two_accounts() {
        let sender = Address::from_bytes([0x77; 32]);
        let recipient = Address::from_bytes([0x88; 32]);
        let mut accounts = HashMap::new();
        for i in 0u8..10 {
            accounts.insert(Address::from_bytes([i; 32]), Account::default());
        }
        accounts.insert(sender, Account::new(Quantum::new(5), 3));
        accounts.insert(recipient, Account::new(Quantum::new(7), 4));

        let tx = contract_call(sender, recipient, echo_runtime());
        let snapshot = snapshot_accounts(&accounts, &tx);

        assert_eq!(snapshot.len(), 2, "snapshot harus O(1), bukan O(n)");
        assert_eq!(snapshot.get(&sender).map(|a| a.nonce), Some(3));
        assert_eq!(snapshot.get(&recipient).map(|a| a.nonce), Some(4));
    }
}
