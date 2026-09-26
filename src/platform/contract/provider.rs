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
use crate::state::sandbox::{dry_run, estimate_gas as sandbox_estimate_gas, SandboxError};
use crate::transaction::types::Transaction;
use crate::transaction::validator::validate_transaction_stateless;

use super::error::ContractError;
use crate::wallet::client;

/// Re-export kanonik dari [`crate::state::sandbox`]: `contract::provider::DryRunReport`
/// dan `state::DryRunReport` **menunjuk tipe yang sama** (AUR-ARCH-005), sehingga
/// simulasi lokal SDK dan `aur_call` di simpul dijamin identik.
pub use crate::state::sandbox::{
    DryRunReport, SANDBOX_GAS_LIMIT, SANDBOX_GAS_LIMIT as STF_GAS_LIMIT,
};

impl From<SandboxError> for ContractError {
    fn from(e: SandboxError) -> Self {
        match e {
            SandboxError::Verification(m) => Self::Verification(m),
            SandboxError::Revert(m) | SandboxError::Execution(m) => Self::SimulationFailed(m),
            SandboxError::OutOfGas => {
                Self::SimulationFailed("kontrak kehabisan gas".to_string())
            }
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


    /// Simulasi read-only terhadap `tx` memakai **sandbox STF kanonik**
    /// (`state::sandbox::dry_run`) pada salinan akun berukuran konstan.
    ///
    /// State asli tidak pernah dimutasi. Implementasi default melakukan
    /// simulasi **lokal**; [`RpcProvider`] meng-override-nya untuk memakai
    /// endpoint `aur_call` simpul agar hasil identik dengan node yang
    /// benar-benar memvalidasi.
    ///
    /// # Inputs
    /// - `tx`: transaksi kanonikal yang akan disimulasikan.
    ///
    /// # Outputs
    /// Laporan gas, data kembalian, dan hasil deploy.
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
        Ok(dry_run(&accounts, tx))
    }

    /// Estimasi gas dengan **konteks eksekusi identik STF** (gas limit 1.000.000,
    /// storage kosong, block 0, timestamp 0).
    ///
    /// # Inputs
    /// - `tx`: transaksi kanonikal yang akan diestimasi.
    ///
    /// # Outputs
    /// Gas terpakai, atau `0` untuk tipe non-kontrak.
    ///
    /// # Errors
    /// Bytecode gagal verifikasi / eksekusi revert atau out-of-gas.
    fn estimate_gas(&self, tx: &Transaction) -> Result<u64, ContractError> {
        Ok(sandbox_estimate_gas(tx)?)
    }

    /// Ambil metadata kontrak (ABI + runtime) dari provider.
    ///
    /// Default: `Ok(None)` — metadata bersifat **off-chain** dan hanya
    /// tersedia bila operator/`RpcProvider` mendaftarkannya ke simpul.
    ///
    /// # Inputs
    /// - `code_hash`: `code_hash` on-chain kontrak (`blake3(payload deploy)`).
    ///
    /// # Outputs
    /// Metadata terdaftar, atau `None` bila tidak ada registry.
    ///
    /// # Errors
    /// Registry mengembalikan metadata yang gagal divalidasi.
    fn fetch_metadata(
        &self,
        _code_hash: &Hash256,
    ) -> Result<Option<super::metadata::ContractMetadata>, ContractError> {
        Ok(None)
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

    /// Panggil `aur_call` (dry-run ter-sandbox di simpul).
    ///
    /// Transaksi **belum ditandatangani** (`Signature::ZERO`) tetap valid:
    /// simpul tidak memverifikasi tanda tangan untuk simulasi.
    ///
    /// # Inputs
    /// - `tx`: transaksi kanonikal yang akan disimulasikan.
    ///
    /// # Outputs
    /// Laporan dry-run dari sandbox STF simpul.
    ///
    /// # Errors
    /// Transport/parsing gagal, atau simpul menolak parameter.
    pub fn call(&self, tx: &Transaction) -> Result<DryRunReport, ContractError> {
        let raw_hex = hex::encode(encode_raw(tx));
        client::contract_call(&self.rpc_url, &raw_hex)
            .map_err(|e| ContractError::Provider(e.to_string()))
    }
}

/// Serialisasi kanonikal transaksi untuk transport `aur_call`.
fn encode_raw(tx: &Transaction) -> Vec<u8> {
    use crate::codec::CanonicalEncode;
    let mut buf = Vec::new();
    tx.encode_canonical(&mut buf);
    buf
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

    /// Dry-run **di sisi simpul** lewat `aur_call`; jatuh ke simulasi lokal bila
    /// simpul lama belum mendukung endpoint tersebut (backward compatibility).
    fn simulate(&self, tx: &Transaction) -> Result<DryRunReport, ContractError> {
        match self.call(tx) {
            Ok(report) => Ok(report),
            Err(_) => {
                // Fallback: simulasi lokal dengan state yang di-fetch via RPC.
                let mut accounts = HashMap::new();
                accounts.insert(tx.sender, self.get_account(&tx.sender)?);
                if tx.recipient != tx.sender {
                    accounts.insert(tx.recipient, self.get_account(&tx.recipient)?);
                }
                Ok(dry_run(&accounts, tx))
            }
        }
    }

    /// Estimasi gas **di sisi simpul** lewat `aur_estimateGas`; jatuh ke
    /// estimasi lokal bila endpoint belum tersedia.
    fn estimate_gas(&self, tx: &Transaction) -> Result<u64, ContractError> {
        let raw_hex = hex::encode(encode_raw(tx));
        match client::estimate_gas(&self.rpc_url, &raw_hex) {
            Ok(value) => Ok(value),
            Err(_) => Ok(sandbox_estimate_gas(tx)?),
        }
    }

    /// Ambil metadata kontrak dari registry off-chain simpul
    /// (`aur_getContractMetadata`).
    fn fetch_metadata(
        &self,
        code_hash: &Hash256,
    ) -> Result<Option<super::metadata::ContractMetadata>, ContractError> {
        match client::get_contract_metadata(&self.rpc_url, &code_hash.to_hex()) {
            Ok(json) => super::metadata::ContractMetadata::from_json(json.as_str()).map(Some),
            // Metadata off-chain bersifat opsional: simpul yang tidak memiliki
            // registry akan mengembalikan galat, SDK lalu memakai metadata lokal.
            Err(_) => Ok(None),
        }
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
        let proposer = tx.sender;
        crate::state::stf::apply_transaction(&mut accounts, &mut monetary, &proposer, &tx)
            .map_err(|e| ContractError::Broadcast(format!("STF menolak transaksi: {e}")))?;
        drop(monetary);
        drop(accounts);
        let tx_id = tx.compute_tx_id();
        self.ledger.lock().expect("ledger lock").push(tx);
        Ok(tx_id)
    }
}
