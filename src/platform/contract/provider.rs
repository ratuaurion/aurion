//! Trait [`Provider`]: pembaca state, simulasi lokal (dry-run), dan broadcast.
//!
//! Dry-run **mereplikasi jalur STF secara persis** pada salinan state yang
//! di-*sandbox* (pola yang sama dipakai `consensus::bft::engine` untuk seleksi
//! blok) — tanpa menyentuh konsensus, SMT, atau storage engine.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::codec::CanonicalDecode;
use crate::core::{Address, Hash256, Quantum};
use crate::state::account::Account;
use crate::state::monetary::MonetaryState;
use crate::state::stf::{apply_transaction, derive_contract_address};
use crate::transaction::types::{Transaction, TxType};
use crate::transaction::validator::validate_transaction_stateless;
use crate::vm::context::ExecutionContext;
use crate::vm::engine::{AvmEngine, ExecutionResult};
use crate::vm::verifier::BytecodeVerifier;

use super::error::ContractError;
use crate::wallet::client;

/// Gas limit eksekusi kontrak persis seperti pada STF (`AUR-VM-003`).
const STF_GAS_LIMIT: u64 = 1_000_000;

/// Hasil simulasi lokal (dry-run) satu transaksi terhadap STF.
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

/// Kontrak layanan klien Aurion: baca state, simulasi lokal, dan broadcast.
///
/// Implementasi kanonikal: [`RpcProvider`] (JSON-RPC simpul) dan
/// [`MemoryProvider`] (ledger in-process untuk pengujian/offline).
pub trait Provider {
    /// Chain ID jaringan tujuan.
    ///
    /// # Errors
    /// Provider tidak dapat dijangkau.
    fn chain_id(&self) -> Result<u32, ContractError>;

    /// Ambil snapshot akun (balance, nonce, `code_hash`).
    ///
    /// # Errors
    /// Provider tidak dapat dijangkau / respons tidak valid.
    fn get_account(&self, address: &Address) -> Result<Account, ContractError>;

    /// Waktu Unix saat ini (detik) untuk `valid_until`.
    fn current_time(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Nonce on-chain otomatis untuk pengirim.
    ///
    /// # Errors
    /// Provider tidak dapat dijangkau.
    fn get_nonce(&self, address: &Address) -> Result<u64, ContractError> {
        Ok(self.get_account(address)?.nonce)
    }

    /// Saldo on-chain pengirim.
    ///
    /// # Errors
    /// Provider tidak dapat dijangkau.
    fn get_balance(&self, address: &Address) -> Result<Quantum, ContractError> {
        Ok(self.get_account(address)?.balance)
    }

    /// Siarkan transaksi tercatat (hex kanonikal + pubkey pengirim).
    ///
    /// # Errors
    /// Ditolak mempool / transport gagal.
    fn broadcast(&self, raw_hex: &str, sender_pubkey_hex: &str) -> Result<Hash256, ContractError>;


    /// Simulasi lokal: terapkan `tx` pada **clone** state akun via STF kanonikal.
    ///
    /// # Errors
    /// Hanya untuk kegagalan infrastruktur provider; kegagalan STF dilaporkan
    /// sebagai [`DryRunReport`] dengan `success == false`.
    fn simulate(&self, tx: &Transaction) -> Result<DryRunReport, ContractError> {
        let mut accounts = HashMap::new();
        accounts.insert(tx.sender, self.get_account(&tx.sender)?);
        if tx.recipient != tx.sender {
            accounts.insert(tx.recipient, self.get_account(&tx.recipient)?);
        }
        let mut monetary = MonetaryState::default();
        let proposer = tx.sender;
        let gas_used = self.estimate_gas(tx).unwrap_or(0);
        match apply_transaction(&mut accounts, &mut monetary, &proposer, tx) {
            Ok(receipt) => Ok(DryRunReport {
                success: true,
                gas_used,
                return_data: receipt.return_data,
                reason: None,
                deployed_contract: receipt.deployed_contract,
                storage_changes: receipt.storage_changes.len(),
            }),
            Err(e) => Ok(DryRunReport {
                success: false,
                gas_used: 0,
                return_data: Vec::new(),
                reason: Some(e.to_string()),
                deployed_contract: None,
                storage_changes: 0,
            }),
        }
    }

    /// Estimasi gas dengan **konteks eksekusi identik STF** (gas limit 1.000.000,
    /// storage kosong, block 0, timestamp 0).
    ///
    /// # Errors
    /// Bytecode gagal verifikasi / eksekusi revert atau out-of-gas.
    fn estimate_gas(&self, tx: &Transaction) -> Result<u64, ContractError> {
        match tx.tx_type {
            TxType::ContractDeploy | TxType::ContractCall => {}
            _ => return Ok(0),
        }
        let verified = BytecodeVerifier::verify(&tx.payload)
            .map_err(|e| ContractError::Verification(e.to_string()))?;
        let target = match tx.tx_type {
            TxType::ContractDeploy => derive_contract_address(&tx.sender, tx.nonce),
            _ => tx.recipient,
        };
        let ctx = ExecutionContext::new(
            tx.sender,
            target,
            tx.sender,
            tx.amount,
            STF_GAS_LIMIT,
            0,
            0,
        );
        let empty_storage = HashMap::new();
        match AvmEngine::execute(&verified, ctx, &empty_storage) {
            ExecutionResult::Success { gas_used, .. } => Ok(gas_used),
            ExecutionResult::Revert { reason, .. } => Err(ContractError::SimulationFailed(format!(
                "kontrak revert: {reason}"
            ))),
            ExecutionResult::OutOfGas => Err(ContractError::SimulationFailed(
                "kontrak kehabisan gas".to_string(),
            )),
            ExecutionResult::Error(e) => Err(ContractError::SimulationFailed(e)),
        }
    }
}

/// Provider JSON-RPC menuju simpul Aurion (endpoint `aur_*`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcProvider {
    /// URL RPC, mis. `http://127.0.0.1:8545`.
    pub rpc_url: String,
}

impl RpcProvider {
    /// Buat provider baru menuju `rpc_url`.
    #[must_use]
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
        }
    }
}

impl Provider for RpcProvider {
    fn chain_id(&self) -> Result<u32, ContractError> {
        client::get_chain_id(&self.rpc_url).map_err(|e| ContractError::Provider(e.to_string()))
    }

    fn get_account(&self, address: &Address) -> Result<Account, ContractError> {
        let bech32m = crate::crypto::encode_address_bech32m(address, "aur")
            .map_err(|e| ContractError::InvalidAddress(e.to_string()))?;
        let rpc_account = client::get_account(&self.rpc_url, &bech32m)
            .map_err(|e| ContractError::Provider(e.to_string()))?;
        Ok(Account {
            balance: Quantum::new(rpc_account.balance),
            nonce: rpc_account.nonce,
            code_hash: rpc_account.code_hash.map(Hash256::from_bytes),
            storage_root: None,
        })
    }

    fn broadcast(&self, raw_hex: &str, sender_pubkey_hex: &str) -> Result<Hash256, ContractError> {
        let tx_id_hex = client::broadcast_raw_tx(&self.rpc_url, raw_hex, sender_pubkey_hex)
            .map_err(|e| ContractError::Broadcast(e.to_string()))?;
        let bytes = hex::decode(tx_id_hex.trim_start_matches("0x"))
            .map_err(|e| ContractError::Broadcast(format!("TxID tidak valid: {e}")))?;
        if bytes.len() != 32 {
            return Err(ContractError::Broadcast(format!(
                "TxID harus 32 byte, diterima {}",
                bytes.len()
            )));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Hash256(arr))
    }
}

/// Provider ledger in-process: state akun dibagikan lewat `Arc<Mutex<..>>` dan
/// `broadcast` menerapkan transaksi **langsung** (finalitas instan) — untuk
/// pengujian, contoh, dan penggunaan offline dalam satu binary.
#[derive(Clone)]
pub struct MemoryProvider {
    chain_id: u32,
    accounts: Arc<Mutex<HashMap<Address, Account>>>,
    monetary: Arc<Mutex<MonetaryState>>,
    time: Arc<AtomicU64>,
    ledger: Arc<Mutex<Vec<Transaction>>>,
}

impl MemoryProvider {
    /// Provider baru dengan chain ID tertentu dan waktu berjalan nyata.
    #[must_use]
    pub fn new(chain_id: u32) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            chain_id,
            accounts: Arc::new(Mutex::new(HashMap::new())),
            monetary: Arc::new(Mutex::new(MonetaryState::default())),
            time: Arc::new(AtomicU64::new(now)),
            ledger: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Provider baru dengan satu akun ber saldo awal `balance`.
    #[must_use]
    pub fn with_account(chain_id: u32, address: &Address, balance: Quantum) -> Self {
        let provider = Self::new(chain_id);
        provider
            .accounts
            .lock()
            .expect("accounts lock")
            .insert(*address, Account::new(balance, 0));
        provider
    }

    /// Atur waktu provider (detik Unix) — untuk pengujian `valid_until`.
    pub fn set_time(&self, unix_secs: u64) {
        self.time.store(unix_secs, Ordering::SeqCst);
    }

    /// Snapshot akun untuk asersi pengujian.
    #[must_use]
    pub fn account(&self, address: &Address) -> Option<Account> {
        self.accounts.lock().expect("accounts lock").get(address).cloned()
    }

    /// Daftar transaksi yang pernah di-broadcast (urutan penerimaan).
    #[must_use]
    pub fn ledger(&self) -> Vec<Transaction> {
        self.ledger.lock().expect("ledger lock").clone()
    }
}

impl std::fmt::Debug for MemoryProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryProvider")
            .field("chain_id", &self.chain_id)
            .finish_non_exhaustive()
    }
}

impl Provider for MemoryProvider {
    fn chain_id(&self) -> Result<u32, ContractError> {
        Ok(self.chain_id)
    }

    fn get_account(&self, address: &Address) -> Result<Account, ContractError> {
        Ok(self
            .accounts
            .lock()
            .expect("accounts lock")
            .get(address)
            .cloned()
            .unwrap_or_default())
    }

    fn current_time(&self) -> u64 {
        self.time.load(Ordering::SeqCst)
    }

    fn broadcast(&self, raw_hex: &str, sender_pubkey_hex: &str) -> Result<Hash256, ContractError> {
        let raw = hex::decode(raw_hex)
            .map_err(|e| ContractError::Broadcast(format!("raw hex tidak valid: {e}")))?;
        let pubkey = hex::decode(sender_pubkey_hex)
            .map_err(|e| ContractError::Broadcast(format!("pubkey hex tidak valid: {e}")))?;
        if pubkey.len() != 32 {
            return Err(ContractError::Broadcast(
                "pubkey harus 32 byte".to_string(),
            ));
        }
        let mut sender_pubkey = [0u8; 32];
        sender_pubkey.copy_from_slice(&pubkey);

        let mut cursor = 0;
        let tx = Transaction::decode_canonical(&raw, &mut cursor)
            .map_err(|e| ContractError::Broadcast(format!("decode transaksi gagal: {e:?}")))?;

        if tx.chain_id != self.chain_id {
            return Err(ContractError::Broadcast(format!(
                "chain ID transaksi {} tidak cocok dengan provider {}",
                tx.chain_id, self.chain_id
            )));
        }
        let now = self.current_time();
        if tx.valid_until != 0 && tx.valid_until <= now {
            return Err(ContractError::Broadcast(format!(
                "transaksi kedaluwarsa (valid_until {} <= {now})",
                tx.valid_until
            )));
        }
        validate_transaction_stateless(&tx, &sender_pubkey)
            .map_err(|e| ContractError::StatelessValidation(e.to_string()))?;

        // Terapkan langsung pada ledger lokal (finalitas instan, untuk pengujian).
        let mut accounts = self.accounts.lock().expect("accounts lock");
        let mut monetary = self.monetary.lock().expect("monetary lock");
        apply_transaction(&mut accounts, &mut monetary, &tx.sender, &tx)
            .map_err(|e| ContractError::Broadcast(format!("STF menolak transaksi: {e}")))?;
        drop(monetary);
        drop(accounts);
        let tx_id = tx.compute_tx_id();
        self.ledger.lock().expect("ledger lock").push(tx);
        Ok(tx_id)
    }
}
