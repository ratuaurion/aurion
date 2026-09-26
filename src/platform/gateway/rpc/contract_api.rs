//! Endpoint Kontrak Cerdas pada RPC Gateway Aurion (`aur_*`).
//!
//! Menyediakan pasangan sisi-simpul dari pipeline Contract SDK
//! (`src/platform/contract/`):
//!
//! | Endpoint SDK (klien)   | Endpoint simpul (modul ini) |
//! |-----------------------|------------------------------|
//! | `Provider::simulate`  | `aur_call`                  |
//! | `Provider::estimate_gas` | `aur_estimateGas`         |
//! | `Provider::fetch_metadata` | `aur_getContractMetadata` |
//! | `aur_getCode` (binding)   | `aur_getCode`           |
//!
//! ## Pola eksekusi (WAJIB — identik dengan dry-run SDK)
//!
//! ```text
//! 1. Clone state  -> snapshot_accounts() : O(1), hanya sender + recipient
//! 2. Apply tx     -> state::stf::apply_transaction() pada snapshot lokal
//! 3. Extract      -> gas_used, return_data, deployed_contract
//! ```
//!
//! State **`self.accounts` asli tidak pernah dimutasi** dan tidak pernah
//! dikunci selama eksekusi bytecode: snapshot diambil di bawah lock, lalu lock
//! dilepas sebelum VM berjalan. Ini mencegah `aur_call` memblokir
//! `sync_rpc_context` maupun lalu lintas BFT (AUR-ISSUE-011).
//!
//! ## Pertahanan DoS berlapis
//!
//! 1. **Gas limit kanonik 1.000.000** (`SANDBOX_GAS_LIMIT`, identik STF) —
//!    opcode loop tak berujung keluar sebagai `OutOfGas`.
//! 2. **Batas payload 24 KB** — decoder kanonikal menolak sisanya.
//! 3. **Token bucket** integer-murni: 20 permintaan/detik, burst 40.
//!    Mengembalikan `-32004` (`RATE_LIMIT_EXCEEDED`) saat habis.
//! 4. **Deadline wall-clock** 2 detik per eksekusi; eksekusi yang melampaui
//!    deadline dilaporkan sebagai kegagalan (eksekusi bersifat terisolasi pada
//!    snapshot, sehingga tidak ada efek samping).
//!
//! Seluruh aritmetika waktu dan kuota memakai integer `u64`/`u128`
//! (AUR-ARCH-012, nol floating-point).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::codec::CanonicalDecode;
use crate::core::{Address, Hash256};
use crate::crypto::{decode_address_bech32m, encode_address_bech32m};
use crate::gateway::rpc::errors::{
    internal_error, invalid_params, rate_limit_exceeded, resource_not_found, tx_rejected,
};
use crate::gateway::rpc::methods::RpcContext;
use crate::gateway::rpc::types::JsonRpcError;
use crate::state::sandbox::{dry_run, estimate_gas, DryRunReport, SANDBOX_GAS_LIMIT};
use crate::transaction::types::Transaction;

/// Kuota token bucket: token yang disimulasikan per detik (integer).
pub const CALL_RATE_PER_SEC: u64 = 20;
/// Kapasitas burst token bucket (pemicik puncak pada eth_getLogs).
pub const CALL_BURST: u64 = 40;
/// Deadline wall-clock per eksekusi dry-run (detik).
pub const CALL_DEADLINE_SECS: u64 = 2;

/// Pembatas laju token bucket tanpa floating-point.
///
/// refill = `elapsed_secs * rate`, dijepit pada `burst`. Seluruh aritmetika
/// `u64`/`u128` (AUR-ARCH-012).
#[derive(Debug)]
pub struct CallRateLimiter {
    state: Mutex<BucketState>,
    rate_per_sec: u64,
    burst: u64,
}

#[derive(Debug)]
struct BucketState {
    tokens: u64,
    last_refill_unix_ms: u128,
}

impl CallRateLimiter {
    /// Bucket baru dengan kapasitas penuh.
    #[must_use]
    pub fn new(rate_per_sec: u64, burst: u64) -> Self {
        Self {
            state: Mutex::new(BucketState {
                tokens: burst,
                last_refill_unix_ms: now_millis(),
            }),
            rate_per_sec,
            burst,
        }
    }

    /// Coba ambil satu token; `false` berarti kuota habis.
    pub fn try_acquire(&self) -> bool {
        let now = now_millis();
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if now > guard.last_refill_unix_ms {
            let elapsed_ms = now - guard.last_refill_unix_ms;
            // refill per detik tanpa float: floor(elapsed_ms * rate / 1000).
            let refill = (elapsed_ms * u128::from(self.rate_per_sec)) / 1_000u128;
            let refill = u64::try_from(refill).unwrap_or(u64::MAX);
            guard.tokens = guard.tokens.saturating_add(refill).min(self.burst);
            guard.last_refill_unix_ms = now;
        }
        if guard.tokens == 0 {
            return false;
        }
        guard.tokens -= 1;
        true
    }
}

impl Default for CallRateLimiter {
    fn default() -> Self {
        Self::new(CALL_RATE_PER_SEC, CALL_BURST)
    }
}

/// Milidetik monotolik sejak epoch (sumber tunggal waktu, tanpa float).
fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}


/// Registry metadata kontrak **off-chain** per simpul.
///
/// Metadata (ABI + runtime) sengaja TIDAK disimpan on-chain: `Account` hanya
/// menyimpan `code_hash`/`storage_root` (AUR-VM-006) dan menambahkannya ke
/// state root akan mengubah `state_root` blok — melanggar batasan "DO NOT alter
/// consensus/block production". Konsekuensinya metadata diindeks di memori
/// proses dan bersifat *best-effort*: hilang saat restart dan tidak tercermin
/// dalam konsensus.
#[derive(Debug, Default)]
pub struct MetadataRegistry {
    entries: Mutex<HashMap<Hash256, String>>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl MetadataRegistry {
    /// Daftarkan metadata mentah (JSON) untuk `code_hash`.
    pub fn register(&self, code_hash: Hash256, raw_json: String) {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(code_hash, raw_json);
    }

    /// Ambil metadata untuk `code_hash` bila terdaftar.
    #[must_use]
    pub fn get(&self, code_hash: &Hash256) -> Option<String> {
        let found = self
            .entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(code_hash)
            .cloned();
        if found.is_some() {
            self.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }
        found
    }

    /// Jumlah entri terdaftar.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Apakah registry kosong.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Decode transaksi kanonikal dari hex untuk simulasi/broadcast.
///
/// # Inputs
/// - `raw_hex`: serialisasi kanonikal `Transaction` (dari `encode_canonical`).
///
/// # Outputs
/// - `Ok(tx)`: transaksi ter-decode.
/// - `Err`: hex rusak, pendek, atau tipe transaksi tak dikenal.
///
/// # Errors
/// Hex tidak valid atau melampaui batas payload 24 KB.
pub fn decode_raw_tx(raw_hex: &str) -> Result<Transaction, JsonRpcError> {
    let cleaned = raw_hex.trim().trim_start_matches("0x");
    let bytes = hex::decode(cleaned)
        .map_err(|e| invalid_params(format!("raw transaction hex tidak valid: {e}")))?;
    let mut cursor = 0usize;
    Transaction::decode_canonical(&bytes, &mut cursor)
        .map_err(|e| invalid_params(format!("decode transaksi kanonikal gagal: {e:?}")))
}

/// Parse parameter opsional `params[i]` sebagai integer `u64`.
fn optional_u64(params: &[String], index: usize, name: &str) -> Result<Option<u64>, JsonRpcError> {
    match params.get(index) {
        None => Ok(None),
        Some(raw) => raw
            .trim()
            .parse::<u64>()
            .map(Some)
            .map_err(|_| invalid_params(format!("Parameter '{name}' harus integer u64: '{raw}'"))),
    }
}

/// Validasi bahwa `tx` dapat disimulasikan oleh `aur_call`/`aur_estimateGas`.
///
/// Tanda tangan **tidak** diverifikasi: simulasi read-only atas transaksi yang
/// belum ditandatangani memang sah dan diperlukan pra-clear-signing.
///
/// # Inputs
/// - `ctx`: konteks RPC simpul.
/// - `tx`: transaksi kanonikal hasil decode.
/// - `requested_gas`: gas limit opsional yang diminta klien.
///
/// # Outputs
/// - `Ok(())`: permintaan sah.
/// - `Err`: parameter tidak valid.
///
/// # Errors
/// Chain ID simpul tidak cocok, atau gas limit diminta melebihi batas kanonik.
fn validate_call_request(
    ctx: &RpcContext,
    tx: &Transaction,
    requested_gas: Option<u64>,
) -> Result<(), JsonRpcError> {
    if tx.version != 1 {
        return Err(invalid_params(format!("version {} != 1", tx.version)));
    }
    if tx.chain_id != ctx.chain_id {
        return Err(invalid_params(format!(
            "chain ID transaksi {} tidak cocok dengan jaringan {}",
            tx.chain_id, ctx.chain_id
        )));
    }
    if let Some(limit) = requested_gas {
        if limit == 0 || limit > SANDBOX_GAS_LIMIT {
            return Err(invalid_params(format!(
                "gas limit {limit} di luar rentang 1..={SANDBOX_GAS_LIMIT}"
            )));
        }
    }
    Ok(())
}

/// Ambil snapshot akun sandbox dan **lepaskan lock sebelum** eksekusi VM.
///
/// Lock `accounts` hanya disimpan selama penggabungan dua entri. Menahannya
/// selama bytecode berjalan akan memblokir `sync_rpc_context` dan seluruh lalu
/// lintas lain pada simpul (AUR-ISSUE-011).
fn take_sandbox_snapshot(
    ctx: &RpcContext,
    tx: &Transaction,
) -> HashMap<Address, crate::state::Account> {
    let guard = ctx.accounts.lock().unwrap_or_else(|e| e.into_inner());
    crate::state::sandbox::snapshot_accounts(&guard, tx)
}

/// Render [`DryRunReport`] menjadi objek JSON deterministik.
///
/// Seluruh nilai kuantitatif berupa integer (AUR-ARCH-012).
#[must_use]
pub fn render_dry_run(report: &DryRunReport, gas_limit: u64) -> String {
    let reason_json = match &report.reason {
        Some(r) => json_quote(r),
        None => "null".to_string(),
    };
    let deployed_json = match &report.deployed_contract {
        Some(a) => json_quote(&encode_address_bech32m(a, "aur").unwrap_or_else(|_| a.to_hex())),
        None => "null".to_string(),
    };
    format!(
        r#"{{"success":{},"gas_used":{},"return_data":"{}","reason":{reason_json},"deployed_contract":{deployed_json},"storage_changes":{},"gas_limit":{gas_limit}}}"#,
        report.success,
        report.gas_used,
        hex::encode(&report.return_data),
        report.storage_changes
    )
}

/// Escape string untuk embedding JSON (tanpa dependensi eksternal).
#[must_use]
pub fn json_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Parse `code_hash` hex 32-byte (opsional prefiks `0x`).
///
/// # Inputs
/// - `raw`: string hex 32-byte.
///
/// # Outputs
/// Digest 32-byte, atau galat `-32602`.
///
/// # Errors
/// Hex rusak atau panjang bukan 32 byte.
pub fn parse_code_hash_hex(raw: &str) -> Result<Hash256, JsonRpcError> {
    let cleaned = raw.trim().trim_start_matches("0x");
    let bytes = hex::decode(cleaned)
        .map_err(|e| invalid_params(format!("code_hash hex tidak valid: {e}")))?;
    if bytes.len() != 32 {
        return Err(invalid_params(format!(
            "code_hash harus 32 byte, diterima {}",
            bytes.len()
        )));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(Hash256(arr))
}

/// Parse alamat Bech32m HRP `aur` atau hex 32-byte.
///
/// # Inputs
/// - `raw`: Bech32m `aur1...` atau hex 32-byte.
///
/// # Outputs
/// Alamat 32-byte, atau galat `-32602`.
///
/// # Errors
/// Bukan Bech32m `aur` yang valid maupun hex 32-byte.
pub fn parse_address(raw: &str) -> Result<Address, JsonRpcError> {
    if let Ok(address) = decode_address_bech32m(raw, "aur") {
        return Ok(address);
    }
    let cleaned = raw.trim().trim_start_matches("0x");
    let bytes = hex::decode(cleaned).map_err(|e| {
        invalid_params(format!("address '{raw}' bukan Bech32m 'aur' maupun hex: {e}"))
    })?;
    if bytes.len() != 32 {
        return Err(invalid_params(format!(
            "address hex harus 32 byte, diterima {}",
            bytes.len()
        )));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(Address(arr))
}

/// `aur_call` -- dry-run read-only kontrak pada **sandbox** (tidak mutasi state).
///
/// Parameter: `[raw_tx_hex]` atau `[raw_tx_hex, gas_limit]`.
/// `raw_tx_hex` adalah serialisasi kanonikal `Transaction` (`encode_canonical`);
/// transaksi yang belum bertanda tangan tetap diterima karena simulasi bersifat
/// read-only dan justru dibutuhkan sebelum clear signing.
///
/// Pola eksekusi (WAJIB identik dengan dry-run SDK):
/// `Clone State -> Apply Transaction -> Extract Gas/Return Data`.
///
/// # Inputs
/// - `ctx`: konteks RPC simpul.
/// - `params`: parameter JSON-RPC.
/// - `current_time`: waktu Unix simpul (diteruskan untuk konsistensi antarmuka).
///
/// # Outputs
/// Objek JSON dengan `success`, `gas_used`, `return_data`, `reason`,
/// `deployed_contract`, `storage_changes`, dan `gas_limit`.
///
/// # Errors
/// `-32602` parameter salah, `-32004` kuota habis, `-32001` eksekusi gagal.
pub fn handle_call(
    ctx: &RpcContext,
    params: &[String],
    current_time: u64,
) -> Result<String, JsonRpcError> {
    let raw_hex = params
        .first()
        .ok_or_else(|| invalid_params("Missing raw transaction hex parameter"))?;
    let requested_gas = optional_u64(params, 1, "gas_limit")?;

    if !ctx.call_limiter.try_acquire() {
        return Err(rate_limit_exceeded(
            "Kuota simulasi kontrak habis (token bucket). Coba lagi sebentar.",
        ));
    }

    let tx = decode_raw_tx(raw_hex)?;
    validate_call_request(ctx, &tx, requested_gas)?;
    let _ = current_time;

    // Kloning state (O(1)) di bawah lock, lalu lock dilepas sebelum VM jalan.
    let snapshot = take_sandbox_snapshot(ctx, &tx);
    let started = Instant::now();
    let report = dry_run(&snapshot, &tx);
    let elapsed = started.elapsed();
    drop(snapshot);

    if elapsed.as_secs() >= CALL_DEADLINE_SECS {
        return Err(internal_error(format!(
            "Simulasi melampaui deadline {CALL_DEADLINE_SECS}s (state sandbox tidak termutasi)"
        )));
    }

    Ok(render_dry_run(&report, SANDBOX_GAS_LIMIT))
}

/// `aur_estimateGas` -- gas terpakai dengan konteks eksekusi identik STF.
///
/// Parameter: `[raw_tx_hex]` atau `[raw_tx_hex, gas_limit]`.
///
/// Berbeda dari `aur_call`, kegagalan eksekusi dikembalikan **di dalam payload**
/// (`success:false` + `reason`) dengan HTTP 200, bukan sebagai JSON-RPC error,
/// sehingga klien dapat membedakan "kontrak revert" dari "permintaan salah".
///
/// # Inputs
/// - `ctx`: konteks RPC simpul.
/// - `params`: parameter JSON-RPC.
///
/// # Outputs
/// Objek JSON: `gas_used`, `gas_limit`, `success`, `reason`.
///
/// # Errors
/// `-32602` parameter salah, `-32004` kuota habis.
pub fn handle_estimate_gas(
    ctx: &RpcContext,
    params: &[String],
) -> Result<String, JsonRpcError> {
    let raw_hex = params
        .first()
        .ok_or_else(|| invalid_params("Missing raw transaction hex parameter"))?;
    let requested_gas = optional_u64(params, 1, "gas_limit")?;

    if !ctx.call_limiter.try_acquire() {
        return Err(rate_limit_exceeded(
            "Kuota estimasi gas habis (token bucket). Coba lagi sebentar.",
        ));
    }

    let tx = decode_raw_tx(raw_hex)?;
    validate_call_request(ctx, &tx, requested_gas)?;

    match estimate_gas(&tx) {
        Ok(gas) => Ok(format!(
            r#"{{"gas_used":{gas},"gas_limit":{SANDBOX_GAS_LIMIT},"success":true,"reason":null}}"#
        )),
        Err(e) => Ok(format!(
            r#"{{"gas_used":0,"gas_limit":{SANDBOX_GAS_LIMIT},"success":false,"reason":{}}}"#,
            json_quote(&e.to_string())
        )),
    }
}

/// `aur_getContractMetadata` -- metadata kontrak dari registry off-chain simpul.
///
/// Parameter: `[code_hash_hex]`.
///
/// **Asumsi yang harus didokumentasikan:** metadata ABI Aurion bersifat
/// **off-chain**. `Account` on-chain hanya menyimpan `code_hash` dan
/// `storage_root` (AUR-VM-006); menambahkan metadata ke state root akan
/// mengubah `state_root` setiap blok sehingga mengubah consensus -- hal yang
/// dilarang oleh batasan "DO NOT alter BFT consensus or block production".
/// Karena itu registry bersifat in-memory per simpul, best-effort, dan hilang
/// saat restart.
///
/// Bila metadata tidak terdaftar, simpul mengembalikan `-32002` dan SDK memakai
/// metadata lokal yang terikat `code_hash`; pertahanan anti-penipuan tetap
/// dijamin `ContractMetadata::verify_binding` terhadap `aur_getAccount`.
///
/// # Inputs
/// - `ctx`: konteks RPC simpul.
/// - `params`: parameter JSON-RPC berisi `code_hash` hex 32-byte.
///
/// # Outputs
/// Objek JSON metadata kontrak mentah seperti tersimpan di registry.
///
/// # Errors
/// `-32602` hex tidak valid, `-32002` metadata tidak terdaftar.
pub fn handle_get_contract_metadata(
    ctx: &RpcContext,
    params: &[String],
) -> Result<String, JsonRpcError> {
    let code_hash = params
        .first()
        .ok_or_else(|| invalid_params("Missing code_hash parameter"))?;
    let hash = parse_code_hash_hex(code_hash)?;

    ctx.contract_metadata.get(&hash).ok_or_else(|| {
        resource_not_found(format!(
            "Metadata for code_hash {} is not registered on this node. Metadata is off-chain: publish it via aur_sendContractMetadata, or supply local metadata bound to code_hash.",
            hash.to_hex()
        ))
    })
}

/// `aur_getCode` -- apakah alamat adalah kontrak beserta `code_hash`-nya.
///
/// Parameter: `[address_bech32m]` atau `[address_hex]`.
///
/// Melengkapi `aur_getAccount` (yang juga sudah mengembalikan `code_hash`) dengan
/// bentuk minimal khusus untuk binding kontrak pada Contract SDK.
///
/// # Inputs
/// - `ctx`: konteks RPC simpul.
/// - `params`: parameter JSON-RPC berisi alamat.
///
/// # Outputs
/// Objek JSON: `address`, `code_hash`, `storage_root`, `is_contract`,
/// `code_available`.
///
/// # Errors
/// `-32602` alamat tidak valid.
pub fn handle_get_code(ctx: &RpcContext, params: &[String]) -> Result<String, JsonRpcError> {
    let addr_param = params
        .first()
        .ok_or_else(|| invalid_params("Missing address parameter"))?;
    let address = parse_address(addr_param)?;

    let acc = {
        let guard = ctx.accounts.lock().unwrap_or_else(|e| e.into_inner());
        guard.get(&address).cloned().unwrap_or_default()
    };
    let code_hash_json = match acc.code_hash {
        Some(h) => format!("\"{}\"", h.to_hex()),
        None => "null".to_string(),
    };
    let storage_root_json = match acc.storage_root {
        Some(h) => format!("\"{}\"", h.to_hex()),
        None => "null".to_string(),
    };
    let bech32m = encode_address_bech32m(&address, "aur").unwrap_or_else(|_| address.to_hex());

    Ok(format!(
        r#"{{"address":"{bech32m}","code_hash":{code_hash_json},"storage_root":{storage_root_json},"is_contract":{},"code_available":false,"note":"Contract code is not stored on-chain, only code_hash (AUR-VM-006). Fetch runtime via aur_getContractMetadata."}}"#,
        acc.is_contract()
    ))
}

/// `aur_sendContractMetadata` -- daftarkan metadata kontrak ke registry off-chain.
///
/// Parameter: `[code_hash_hex, metadata_json]`.
///
/// **Sengaja bukan bagian dari consensus.** Penulisannya hanya mengisi indeks
/// in-memory simpul; tidak menyentuh state root, mempool, BFT, maupun block
/// production. Metadata yang `code_hash`-nya berbeda akan ditolak, dan
/// `ContractMetadata::from_json` menurunkan ulang setiap selector dari
/// `signature` sehingga selector palsu tidak dapat diselundupkan.
///
/// # Inputs
/// - `ctx`: konteks RPC simpul.
/// - `params`: parameter JSON-RPC berisi `code_hash` dan metadata JSON.
///
/// # Outputs
/// Objek JSON konfirmasi: `code_hash`, `registered`, `runtime_hash`.
///
/// # Errors
/// `-32602` hash tidak valid atau metadata gagal divalidasi.
pub fn handle_send_contract_metadata(
    ctx: &RpcContext,
    params: &[String],
) -> Result<String, JsonRpcError> {
    if params.len() < 2 {
        return Err(invalid_params(
            "Expected params: [code_hash_hex, metadata_json]",
        ));
    }
    let hash = parse_code_hash_hex(&params[0])?;
    let raw = params[1].trim().to_string();

    let metadata = crate::contract::metadata::ContractMetadata::from_json(&raw)
        .map_err(|e| invalid_params(format!("invalid contract metadata: {e}")))?;

    // Pengikatan anti-tamu: code_hash pada metadata harus sama dengan yang diminta.
    let declared = metadata
        .code_hash_bytes()
        .map_err(|e| invalid_params(format!("invalid code_hash in metadata: {e}")))?;
    if declared != hash {
        return Err(invalid_params(format!(
            "metadata code_hash {} does not match requested {}",
            declared.to_hex(),
            hash.to_hex()
        )));
    }
    // Validasi integritas runtime (runtime_hash == blake3(runtime)) bila ada.
    metadata
        .runtime_bytes()
        .map_err(|e| invalid_params(format!("invalid contract runtime: {e}")))?;
    let runtime_hash = metadata.runtime_hash.clone().unwrap_or_default();

    ctx.contract_metadata.register(hash, raw);

    Ok(format!(
        r#"{{"code_hash":"{}","registered":true,"runtime_hash":{}}}"#,
        hash.to_hex(),
        if runtime_hash.is_empty() {
            "null".to_string()
        } else {
            json_quote(&runtime_hash)
        }
    ))
}

/// Hasil validasi intent clear-signing pada transaksi kontrak.
///
/// Parameter `aur_sendRawTransaction` opsional (indeks 2 dan 3):
/// - `params[2]`: Blake3 payload (`payload_hash`) yang disetujui pengguna.
/// - `params[3]`: `code_hash` kontrak yang disetujui pengguna.
///
/// Bila keduanya tidak diberikan, validasi dilewati (klien non-SDK, backward
/// compatible). Bila diberikan, keduanya **wajib** cocok dengan isi transaksi
/// dan dengan state on-chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentBinding {
    /// Blake3 payload yang disetujui prompt clear signing.
    pub payload_hash: Hash256,
    /// `code_hash` kontrak yang disetujui prompt clear signing.
    pub code_hash: Hash256,
}

impl IntentBinding {
    /// Baca binding opsional dari parameter RPC.
    ///
    /// # Inputs
    /// - `params`: parameter JSON-RPC `aur_sendRawTransaction`.
    ///
    /// # Outputs
    /// - `Ok(None)`: tidak ada intent binding (klien non-SDK).
    /// - `Ok(Some(binding))`: kedua hash terbaca.
    ///
    /// # Errors
    /// Hanya satu dari dua hash yang diberikan, atau hex tidak valid.
    pub fn from_params(params: &[String]) -> Result<Option<Self>, JsonRpcError> {
        match (params.get(2), params.get(3)) {
            (None, None) => Ok(None),
            (Some(p), Some(c)) => Ok(Some(Self {
                payload_hash: parse_code_hash_hex(p)?,
                code_hash: parse_code_hash_hex(c)?,
            })),
            _ => Err(invalid_params(
                "Intent binding requires BOTH hashes: payload_hash and code_hash",
            )),
        }
    }

    /// Verifikasi binding terhadap transaksi dan state on-chain.
    ///
    /// Untuk `ContractDeploy`, `code_hash` yang dijanjikan harus sama dengan
    /// `blake3(payload)` -- inilah yang mengikat intent ke kode yang benar-benar
    /// akan dieksekusi. Untuk `ContractCall`, `code_hash` harus sama dengan
    /// `code_hash` kontrak pada state on-chain, sehingga klien yang menampilkan
    /// metadata palsu ditolak **sebelum** transaksi masuk mempool.
    ///
    /// # Inputs
    /// - `tx`: transaksi yang akan disiarkan.
    /// - `on_chain_code_hash`: `code_hash` akun tujuan pada state on-chain.
    ///
    /// # Outputs
    /// - `Ok(())`: intent cocok.
    ///
    /// # Errors
    /// `payload_hash` atau `code_hash` tidak cocok.
    pub fn verify(
        &self,
        tx: &Transaction,
        on_chain_code_hash: Option<Hash256>,
    ) -> Result<(), JsonRpcError> {
        let actual_payload_hash = crate::state::sandbox::payload_code_hash(&tx.payload);
        if actual_payload_hash != self.payload_hash {
            return Err(tx_rejected(
                "Intent mismatch: payload_hash does not match the signed transaction",
                Some(format!(
                    r#"{{"expected":"{}","actual":"{}"}}"#,
                    self.payload_hash.to_hex(),
                    actual_payload_hash.to_hex()
                )),
            ));
        }

        use crate::transaction::types::TxType;
        match tx.tx_type {
            TxType::ContractDeploy => {
                if actual_payload_hash != self.code_hash {
                    return Err(tx_rejected(
                        "Intent mismatch: deploy code_hash must equal blake3(payload)",
                        Some(format!(
                            r#"{{"expected":"{}","actual":"{}"}}"#,
                            self.code_hash.to_hex(),
                            actual_payload_hash.to_hex()
                        )),
                    ));
                }
                Ok(())
            }
            TxType::ContractCall => match on_chain_code_hash {
                Some(on_chain) if on_chain == self.code_hash => Ok(()),
                Some(on_chain) => Err(tx_rejected(
                    "Intent mismatch: on-chain contract code_hash differs",
                    Some(format!(
                        r#"{{"expected":"{}","on_chain":"{}"}}"#,
                        self.code_hash.to_hex(),
                        on_chain.to_hex()
                    )),
                )),
                None => Err(tx_rejected(
                    "Intent mismatch: destination is not an on-chain contract (no code_hash)",
                    None,
                )),
            },
            _ => Ok(()),
        }
    }
}

