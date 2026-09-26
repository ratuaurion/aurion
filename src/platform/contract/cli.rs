//! Antarmuka Baris Perintah (CLI) Subcommand Kontrak Aurion (`aurion contract`).
//!
//! Mematuhi Invariant AUR-ARCH-001 (Single Binary), AUR-ARCH-009 (CLI Terpadu),
//! AUR-ARCH-010, dan Dokumen Aturan Aplikasi 15 + 16 (Smart Contract Execution).
//!
//! Modul ini adalah **glue tipis** di atas Contract SDK: seluruh logika
//! kontrak tetap hidup di `super::instance` / `super::provider` / `super::signer`.
//! CLI hanya menangani parsing argumen, penyelesaian keystore, dan presentasi.
//!
//! ## Prinsip keselamatan
//!
//! 1. **Pemisahan tegas simulasi vs broadcast.** `call` selalu mencetak hasil
//!    dry-run (gas, return data, status) **sebelum** meminta konfirmasi
//!    clear-signing. Tidak ada penandatanganan yang terjadi sebelum pengguna
//!    melihat hasil simulasi.
//! 2. **Tidak ada kunci yang dibaca tanpa kebutuhan.** `query` dan `metadata`
//!    tidak pernah memerlukan keystore; `call`/`deploy` hanya membuka keystore
//!    pada jalur yang benar-benar menandatangani.
//! 3. **Tanpa_mode JSON bersih.** Warna ANSI hanya disematkan pada mode teks;
//!    `--output json` selalu menghasilkan JSON valid tanpa escape.
//! 4. **Gagal=jadi pesan**, bukan panik. Kesalahan jaringan, keystore, dan
//!    parsing dikembalikan sebagai `Err(String)` yang ramah pengguna.

#![forbid(unsafe_code)]

use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::Path;

use serde::Serialize;

use super::instance::{CallOptions, ContractInstance, DeployRequest};
use super::metadata::{AbiType, AbiValue, ContractMetadata, MethodAbi};
use super::provider::RpcProvider;
use super::signer::{ApprovalMode, KeystoreSigner};
use crate::contract::Provider;
use crate::crypto::{decode_address_bech32m, encode_address_bech32m};
use crate::core::{Address, Hash256, Quantum};
use crate::platform::cli::output::OutputFormat;
use crate::wallet::keystore::Keystore;
use crate::wallet::password::{resolve_password, ENV_WALLET_PASSWORD};

/// Prefix error seragam untuk seluruh pesan kegagalan subcommand kontrak.
const ERR: &str = "[AURION CONTRACT ERROR]";

/// Seed tetap untuk "view caller" pada `query` (read-only).
///
/// Dipakai HANYA untuk mengisi field `caller`/`origin` pada dry-run view call.
/// Seed ini bukan rahasia dan tidak pernah menandatangani apa pun: `read()`
/// berhenti sebelum tahap signing, dan mode persetujuannya `Reject` sebagai
/// pengaman tambahan bila alur berubah di masa depan.
const VIEW_CALLER_SEED: [u8; 32] = [0xA0; 32];

// ---------------------------------------------------------------------------
// Warna terminal (AUR-CLI-007)
// ---------------------------------------------------------------------------

/// Warna ANSI yang dipakai untuk umpan balik status ke operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    Green,
    Red,
    Yellow,
    Cyan,
    Dim,
    Bold,
}

impl Paint {
    /// Kode escape ANSI untuk warna ini.
    const fn code(self) -> &'static str {
        match self {
            Self::Green => "\u{1b}[32m",
            Self::Red => "\u{1b}[31m",
            Self::Yellow => "\u{1b}[33m",
            Self::Cyan => "\u{1b}[36m",
            Self::Dim => "\u{1b}[2m",
            Self::Bold => "\u{1b}[1m",
        }
    }
}

/// Apakah ANSI diizinkan: hormati `NO_COLOR` (https://no-color.org) dan
/// hanya warnai bila stdout benar-benar sebuah terminal.
#[must_use]
pub fn color_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none() && io::stdout().is_terminal()
}

/// Bungkus `text` dengan warna bila terminal mendukungnya.
#[must_use]
pub fn paint(text: &str, p: Paint) -> String {
    if color_enabled() {
        format!("{}{}\u{1b}[0m", p.code(), text)
    } else {
        text.to_string()
    }
}

fn ok(text: &str) -> String {
    paint(text, Paint::Green)
}
fn bad(text: &str) -> String {
    paint(text, Paint::Red)
}
fn warn(text: &str) -> String {
    paint(text, Paint::Yellow)
}
fn info(text: &str) -> String {
    paint(text, Paint::Cyan)
}
fn dim(text: &str) -> String {
    paint(text, Paint::Dim)
}

/// Cetak banner pemisah agar output dry-run vs broadcast mudah dibedakan.
fn banner(title: &str) {
    let line = "=".repeat(70);
    println!("{}", paint(&line, Paint::Bold));
    println!("{}", paint(&format!("  {title}"), Paint::Bold));
    println!("{}", paint(&line, Paint::Bold));
}

// ---------------------------------------------------------------------------
// Parsing argumen
// ---------------------------------------------------------------------------

/// Nilai flag `--name <value>` (juga menerima `--name=<value>`).
fn flag(args: &[String], name: &str) -> Option<String> {
    let long = format!("--{name}");
    for (i, a) in args.iter().enumerate() {
        if a == &long {
            return args.get(i + 1).cloned();
        }
        if let Some(rest) = a.strip_prefix(&format!("{long}=")) {
            return Some(rest.to_string());
        }
    }
    None
}

/// Semua nilai flag yang diulang, mis. `--args 1 --args 2`.
///
/// Nilai yang dimulai dengan `--` dianggap flag, bukan nilai.
fn flags_all(args: &[String], name: &str) -> Vec<String> {
    let long = format!("--{name}");
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == long {
            if let Some(v) = args.get(i + 1) {
                if !v.starts_with("--") {
                    out.push(v.clone());
                    i += 2;
                    continue;
                }
            }
        } else if let Some(rest) = args[i].strip_prefix(&format!("{long}=")) {
            out.push(rest.to_string());
        }
        i += 1;
    }
    out
}

/// Apakah flag boolean seperti `--yes`/-y` diaktifkan.
fn has_flag(args: &[String], name: &str) -> bool {
    let long = format!("--{name}");
    let short = format!("-{name}");
    args.iter().any(|a| a == &long || *a == short)
}

/// Daftar flag yang **meng Boundaries** nilai, agar argumen posisional tidak
/// tertelan sebagai nilai flag saat pemindaian.
const VALUE_FLAGS: &[&str] = &[
    "args", "arg", "metadata", "keystore", "rpc", "rpc-url", "name", "runtime", "value", "fee",
    "nonce", "chain-id", "out",
];

/// Kumpulkan argumen posensial, melewati flag beserta nilainya dan seluruh
/// flag boolean yang dikenal.
fn positionals(args: &[String], boolean_flags: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let is_bool = boolean_flags
            .iter()
            .any(|f| a == &format!("--{f}") || a == &format!("-{f}"));
        if is_bool {
            i += 1;
            continue;
        }

        if let Some(name) = a.strip_prefix("--") {
            if !name.contains('=') && VALUE_FLAGS.contains(&name) {
                i += 2; // flag + nilainya
            } else {
                i += 1;
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            i += 1;
            continue;
        }
        out.push(a.clone());
        i += 1;
    }
    out
}
// ---------------------------------------------------------------------------
// Parsing argumen ABI (string -> AbiValue)
// ---------------------------------------------------------------------------

/// Ubah satu argumen teks menjadi [`AbiValue`] sesuai tipe metadata.
///
/// Aturan parsing per tipe (AUR-ARCH-012: tanpa floating-point):
/// - `address`: Bech32m `aur`/`aurt` atau hex 64 karakter.
/// - `quantum`: bilangan bulat Quanta (`1000000000`), atau desimal AUR
///   (`1.5` = 1,5 AUR) untuk kenyamanan operator.
/// - `u64`/`u32`: bilangan bulat desimal.
/// - `bool`: `true`/`false`/`1`/`0`/`yes`/`no`.
/// - `hash256`: hex 64 karakter.
///
/// # Inputs
/// - `ty`: tipe ABI tujuan.
/// - `raw`: teks argumen dari baris perintah.
///
/// # Outputs
/// - `Ok(value)`: argumen terkonversi.
/// - `Err(String)`: pesan galat yang menjelaskan format yang diharapkan.
///
/// # Errors
/// Teks tidak cocok dengan tipe ABI, atau alamat/hex tidak valid.
pub fn parse_abi_arg(ty: AbiType, raw: &str) -> Result<AbiValue, String> {
    let trimmed = raw.trim();
    match ty {
        AbiType::Address => {
            if trimmed.starts_with("aur") {
                return decode_address_bech32m(trimmed, "aur")
                    .or_else(|_| decode_address_bech32m(trimmed, "aurt"))
                    .map(AbiValue::Address)
                    .map_err(|e| format!("Alamat Bech32m tidak valid '{trimmed}': {e}"));
            }
            if trimmed.len() != 64 {
                return Err(format!(
                    "Alamat harus Bech32m (aur1...) atau hex 64 karakter, diterima '{trimmed}'"
                ));
            }
            let mut arr = [0u8; 32];
            hex::decode_to_slice(trimmed, &mut arr)
                .map_err(|e| format!("Alamat hex tidak valid '{trimmed}': {e}"))?;
            Ok(AbiValue::Address(Address::from_bytes(arr)))
        }
        AbiType::Quantum => {
            // Desimal bertanda titik dibaca sebagai AUR (konvensi operator),
            // sedangkan bilangan bulat murni dibaca sebagai Quanta.
            let quanta = if trimmed.contains('.') {
                Quantum::from_aur_str(trimmed)
                    .map_err(|e| format!("Nilai AUR '{trimmed}' tidak valid: {e}"))?
                    .as_u128()
            } else {
                trimmed
                    .parse::<u128>()
                    .map_err(|_| {
                        format!("Nilai Quantum '{trimmed}' tidak valid (gunakan bilangan bulat Quanta atau desimal AUR)")
                    })?
            };
            Ok(AbiValue::Quantum(Quantum::new(quanta)))
        }
        AbiType::U64 => trimmed
            .parse::<u64>()
            .map(AbiValue::U64)
            .map_err(|_| format!("Argumen u64 '{trimmed}' tidak valid")),
        AbiType::U32 => trimmed
            .parse::<u32>()
            .map(AbiValue::U32)
            .map_err(|_| format!("Argumen u32 '{trimmed}' tidak valid")),
        AbiType::Bool => match trimmed.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "y" => Ok(AbiValue::Bool(true)),
            "false" | "0" | "no" | "n" => Ok(AbiValue::Bool(false)),
            _ => Err(format!("Argumen bool '{trimmed}' tidak valid (true/false)")),
        },
        AbiType::Hash256 => {
            if trimmed.len() != 64 {
                return Err(format!("Hash256 harus 64 hex, diterima '{}'", trimmed.len()));
            }
            let mut arr = [0u8; 32];
            hex::decode_to_slice(trimmed, &mut arr)
                .map_err(|e| format!("Hash256 '{trimmed}' tidak valid: {e}"))?;
            Ok(AbiValue::Hash256(Hash256::from_bytes(arr)))
        }
    }
}


/// Kumpulkan argumen metode dari beberapa sumber: `--args '[..]'` (JSON array),
/// `--args <v>` yang diulang, dan/atau argumen posisional.
///
/// Mengembalikan `Err` bila jumlah argumen tidak konsisten dengan arity
/// metadata agar pengguna mendapat umpan balik segera, bukan revert dari VM.
///
/// # Inputs
/// - `args`: argumen mentah baris perintah.
/// - `positionals`: argumen posisional setelah alamat + nama metode.
/// - `method`: deklarasi metode untuk memvalidasi jumlah & tipe.
///
/// # Outputs
/// - `Ok(values)`: daftar argumen sesuai tipe metadata.
///
/// # Errors
/// JSON tidak valid, jumlah argumen tidak cocok, atau tipe argumen salah.
fn collect_method_args(
    args: &[String],
    positionals: &[String],
    method: &MethodAbi,
) -> Result<Vec<AbiValue>, String> {
    let mut raw: Vec<String> = Vec::new();

    // Sumber 1: --args dengan JSON array.
    if let Some(json) = flag(args, "args").or_else(|| flag(args, "arg")) {
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json)
            .map_err(|e| format!("--args bukan JSON array yang valid: {e}"))?;
        for v in parsed {
            let s = match v {
                serde_json::Value::String(s) => s,
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                other => {
                    return Err(format!("Elemen --args tidak didukung: {other}"));
                }
            };
            raw.push(s);
        }
    }

    // Sumber 2: --args/<--arg> berulang.
    raw.extend(flags_all(args, "args"));
    raw.extend(flags_all(args, "arg"));

    // Sumber 3: posisional.
    raw.extend(positionals.iter().cloned());

    if raw.len() != method.inputs.len() {
        return Err(format!(
            "Metode '{}' memerlukan {} argumen, diberikan {}. Signature: {}",
            method.name,
            method.inputs.len(),
            raw.len(),
            method.signature
        ));
    }

    method
        .inputs
        .iter()
        .zip(raw.iter())
        .map(|(param, text)| parse_abi_arg(param.ty, text))
        .collect()
}

// ---------------------------------------------------------------------------
// Helper bersama
// ---------------------------------------------------------------------------

/// Baca bytecode dari file biner atau dari string hex inline.
///
/// Bila argumen menunjuk berkas yang ada, isinya dibaca sebagai bytecode mentah
/// (dengan deteksi otomatis berkas bertekstur hex). Bila bukan berkas,
/// argumen diperlakukan sebagai hex. Ini menjaga kompatibilitas dengan bentuk
/// lama `aurion contract deploy <hex>`.
///
/// # Inputs
/// - `spec`: path berkas atau string hex.
///
/// # Outputs
/// - `Ok(bytes)`: bytecode AVM.
/// - `Err(String)`: berkas tidak terbaca atau hex tidak valid.
///
/// # Errors
/// Berkas tidak dapat dibaca, atau string hex rusak.
pub fn load_bytecode(spec: &str) -> Result<Vec<u8>, String> {
    let path = Path::new(spec);
    if path.is_file() {
        let raw = fs::read(path).map_err(|e| format!("Gagal membaca '{spec}': {e}"))?;
        let looks_hex = !raw.is_empty()
            && raw
                .iter()
                .all(|b| b.is_ascii_hexdigit() || b.is_ascii_whitespace());
        if looks_hex {
            let cleaned: String = raw
                .iter()
                .map(|&b| b as char)
                .filter(|c| !c.is_whitespace())
                .collect();
            return hex::decode(&cleaned).map_err(|e| format!("Hex pada '{spec}' tidak valid: {e}"));
        }
        return Ok(raw);
    }
    let cleaned = spec.trim().trim_start_matches("0x");
    hex::decode(cleaned).map_err(|e| {
        format!("'{spec}' bukan berkas yang valid dan juga bukan hex bytecode: {e}")
    })
}

/// Baca metadata kontrak dari berkas JSON.
///
/// # Inputs
/// - `path`: path berkas JSON metadata.
///
/// # Outputs
/// - `Ok(metadata)`: metadata tervalidasi (skema + selector diturunkan ulang).
/// - `Err(String)`: berkas bermasalah atau skema tidak cocok.
///
/// # Errors
/// Berkas tidak terbaca, JSON rusak, atau skema metadata tidak didukung.
pub fn load_metadata_file(path: &str) -> Result<ContractMetadata, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("Gagal membaca metadata '{path}': {e}"))?;
    ContractMetadata::from_json(&raw).map_err(|e| format!("Metadata '{path}' tidak valid: {e}"))
}

/// Parse alamat Bech32m (`aur`/`aurt`) atau hex 32-byte.
///
/// # Inputs
/// - `raw`: alamat dalam Bech32m atau hex.
///
/// # Outputs
/// - `Ok(address)`: alamat 32-byte.
/// - `Err(String)`: format tidak dikenali.
///
/// # Errors
/// Bukan Bech32m yang valid, bukan hex 32-byte, atau hex rusak.
pub fn parse_address(raw: &str) -> Result<Address, String> {
    let trimmed = raw.trim();
    if let Ok(addr) = decode_address_bech32m(trimmed, "aur") {
        return Ok(addr);
    }
    if let Ok(addr) = decode_address_bech32m(trimmed, "aurt") {
        return Ok(addr);
    }
    let hexed = trimmed.trim_start_matches("0x");
    if hexed.len() == 64 {
        let mut arr = [0u8; 32];
        hex::decode_to_slice(hexed, &mut arr)
            .map_err(|e| format!("Alamat hex '{trimmed}' tidak valid: {e}"))?;
        return Ok(Address::from_bytes(arr));
    }
    Err(format!(
        "Alamat '{trimmed}' tidak valid: gunakan Bech32m (aur1.../aurt1...) atau hex 64 karakter"
    ))
}

/// Buka keystore wallet dan kembalikan [`KeystoreSigner`].
///
/// Mengikuti pola `wallet send` (3-tier password: stdin -> env -> prompt).
///
/// # Inputs
/// - `args`: argumen baris perintah (`--keystore`, `--password-stdin`).
/// - `mode`: kebijakan persetujuan yang diteruskan ke signer.
///
/// # Outputs
/// - `Ok(signer)`: signer siap menandatangani.
/// - `Err(String)`: berkas/format/password bermasalah.
///
/// # Errors
/// Keystore tidak terbaca/rusak, password salah, atau tidak dapat diperoleh.
fn open_signer(args: &[String], mode: ApprovalMode) -> Result<KeystoreSigner, String> {
    let path = flag(args, "keystore").unwrap_or_else(|| "default.keystore.json".to_string());
    let raw = fs::read_to_string(&path).map_err(|e| format!("Gagal membuka keystore '{path}': {e}"))?;
    // Validasi format keystore lebih awal agar pesan galat lebih informatif
    // dibanding kegagalan enkripsi yang samar.
    Keystore::from_json_str(&raw)
        .map_err(|e| format!("Format keystore '{path}' rusak: {e}"))?;
    let password = resolve_password(
        has_flag(args, "password-stdin") || args.iter().any(|a| a == "--passphrase-stdin"),
        Some(ENV_WALLET_PASSWORD),
        "Masukkan password wallet",
        false,
    )
    .map_err(|e| format!("Gagal memperoleh password keystore: {e}"))?;
    KeystoreSigner::from_keystore_json(&raw, &password, mode)
        .map_err(|e| format!("Gagal membuka keystore '{path}': {e}"))
}

/// Konfirmasi clear signing; hormati `--yes`/`-y` untuk CI non-interaktif.
///
/// # Inputs
/// - `args`: argumen baris perintah.
/// - `summary`: ringkasan yang ditampilkan sebelum konfirmasi.
///
/// # Outputs
/// - `Ok(true)`: pengguna menyetujui.
/// - `Ok(false)`: pengguna menolak (bukan galat).
/// - `Err(String)`: lingkungan non-TTY tanpa `--yes`.
///
/// # Errors
/// stdin bukan terminal sehingga konfirmasi interaktif tidak mungkin.
fn confirm(args: &[String], summary: &str) -> Result<bool, String> {
    if has_flag(args, "yes") || has_flag(args, "auto-approve") {
        println!("{}", dim("  (--yes: persetujuan otomatis)"));
        return Ok(true);
    }
    if !io::stdin().is_terminal() {
        return Err(
            "Lingkungan non-TTY: gunakan --yes/-y untuk persetujuan otomatis (CI/scripting)"
                .to_string(),
        );
    }
    print!("  {summary} Lanjutkan? [y/N]: ");
    io::stdout()
        .flush()
        .map_err(|e| format!("Gagal menampilkan konfirmasi: {e}"))?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Gagal membaca konfirmasi: {e}"))?;
    Ok(matches!(input.trim(), "y" | "Y"))
}

// ---------------------------------------------------------------------------
// Struktur keluaran JSON (AUR-CLI-007)
// ---------------------------------------------------------------------------

/// Hasil `aurion contract deploy`.
#[derive(Debug, Clone, Serialize)]
pub struct DeployOutput {
    /// Status operasi.
    pub status: String,
    /// Alamat kontrak hasil deploy (Bech32m).
    pub contract_address: String,
    /// TxID transaksi deploy.
    pub tx_id: String,
    /// `blake3(payload)` konstruktor.
    pub code_hash: String,
    /// Jumlah byte bytecode konstruktor.
    pub bytecode_bytes: usize,
    /// Gas hasil simulasi STF.
    pub gas_used: u64,
    /// Fee dibayarkan (Quanta).
    pub fee_quanta: String,
    /// Nonce yang dipakai.
    pub nonce: u64,
    /// Berapa mode eksekusi: `offline-verify` atau `broadcast`.
    pub mode: String,
}

/// Hasil `aurion contract call` / `query`.
#[derive(Debug, Clone, Serialize)]
pub struct CallOutput {
    /// Status operasi: `success` atau `reverted`.
    pub status: String,
    /// Alamat kontrak yang dipanggil.
    pub contract_address: String,
    /// Nama metode yang dipanggil.
    pub method: String,
    /// Gas hasil dry-run.
    pub gas_used: u64,
    /// Data kembalian (hex, tanpa prefiks `0x`).
    pub return_data: String,
    /// Data kembalian setelah didekode ke tipe ABI.
    pub decoded: Option<String>,
    /// Alasan kegagalan bila revert.
    pub reason: Option<String>,
    /// TxID; `None` untuk mode query (tidak pernah disiarkan).
    pub tx_id: Option<String>,
    /// Fee (Quanta); `None` bila tidak menyiarkan.
    pub fee_quanta: Option<String>,
}

/// Hasil `aurion contract metadata`.
#[derive(Debug, Clone, Serialize)]
pub struct MetadataOutput {
    /// Sumber metadata: `local-file` atau `node-registry`.
    pub source: String,
    /// Nama kontrak.
    pub name: String,
    /// Chain ID metadata.
    pub chain_id: u32,
    /// Alamat kontrak (Bech32m).
    pub address: String,
    /// `code_hash` kontrak.
    pub code_hash: String,
    /// Jumlah metode dalam ABI.
    pub method_count: usize,
    /// Ringkasan metode beserta selector.
    pub methods: Vec<MethodOutput>,
}

/// Ringkasan satu metode ABI.
#[derive(Debug, Clone, Serialize)]
pub struct MethodOutput {
    /// Nama metode.
    pub name: String,
    /// Signature kanonikal.
    pub signature: String,
    /// Selector 4-byte (hex).
    pub selector: String,
    /// Tipe parameter masukan.
    pub inputs: Vec<String>,
    /// Apakah menerima nilai Quanta.
    pub payable: bool,
}

/// Hasil `aurion contract verify` (offline, tanpa jaringan).
#[derive(Debug, Clone, Serialize)]
pub struct VerifyOutput {
    /// Status verifikasi statis.
    pub status: String,
    /// Jumlah byte bytecode.
    pub bytecode_bytes: usize,
    /// `blake3(bytecode)`.
    pub code_hash: String,
    /// Jumlah target `JUMPDEST` yang sah.
    pub valid_jumpdests: usize,
    /// Estimasi gas deploy.
    pub estimated_gas: u64,
}

// ---------------------------------------------------------------------------
// Subcommand: deploy
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// Subcommand: deploy / verify
// ---------------------------------------------------------------------------

/// `aurion contract deploy <file|hex> [options]`
///
/// Tanpa `--keystore`, perintah berjalan **offline**: hanya memverifikasi
/// bytecode secara statis dan melaporkan `code_hash` (kompatibel dengan
/// perilaku lama, tanpa menyentuh jaringan). Dengan `--keystore`, alur penuh
/// Contract SDK dijalankan: dry-run STF -> clear signing -> broadcast.
fn cmd_deploy(rest: &[String], format: OutputFormat) -> Result<(), String> {
    let positional = positionals(rest, &["yes", "y", "auto-approve", "verify-only"]);
    let Some(spec) = positional.first().cloned() else {
        return Err("Bytecode wajib diisi: aurion contract deploy <file.avm|hex> \
                    [--runtime <f>] [--name <n>] [--keystore <path>]"
            .to_string());
    };

    let constructor = load_bytecode(&spec)?;
    let bytecode_bytes = constructor.len();
    // Runtime default = konstruktor agar kontrak tanpa logika runtime terpisah
    // tetap dapat dideploy (konsisten dengan perilaku CLI sebelumnya).
    let runtime = match flag(rest, "runtime") {
        Some(rt) => load_bytecode(&rt)?,
        None => constructor.clone(),
    };
    let name = flag(rest, "name").unwrap_or_else(|| "AurionContract".to_string());
    let verify_only = has_flag(rest, "verify-only");
    let wants_keystore = flag(rest, "keystore").is_some();

    // Verifikasi statis selalu lebih awal (AUR-VM-005).
    let verified = crate::vm::verifier::BytecodeVerifier::verify(&constructor)
        .map_err(|e| format!("Verifikasi bytecode konstruktor gagal: {e}"))?;
    let code_hash = crate::crypto::blake3_hash(&constructor);

    if verify_only || !wants_keystore {
        return print_verify(&constructor, &code_hash, verified.valid_jump_dests.len(), format);
    }

    let provider = RpcProvider::new(rpc_url(rest));
    let methods = match flag(rest, "metadata") {
        Some(path) => load_metadata_file(&path)?.methods,
        None => Vec::new(),
    };

    let mut request = DeployRequest::new(name, constructor, runtime).with_methods(methods);
    if let Some(v) = flag(rest, "value") {
        request.initial_balance = Quantum::new(
            v.parse::<u128>()
                .map_err(|_| format!("--value '{v}' bukan bilangan bulat Quanta yang valid"))?,
        );
    }
    if let Some(f) = flag(rest, "fee") {
        request.fee = Some(Quantum::new(
            f.parse::<u128>()
                .map_err(|_| format!("--fee '{f}' bukan bilangan bulat Quanta yang valid"))?,
        ));
    }
    if let Some(n) = flag(rest, "nonce") {
        request.nonce = Some(
            n.parse::<u64>()
                .map_err(|_| format!("--nonce '{n}' bukan integer yang valid"))?,
        );
    }

    let auto = has_flag(rest, "yes") || has_flag(rest, "auto-approve");
    let signer = open_signer(
        rest,
        if auto {
            ApprovalMode::AutoApprove
        } else {
            ApprovalMode::Interactive
        },
    )?;

    banner("AURION CONTRACT DEPLOY - SIMULASI (DRY-RUN)");
    println!("  Bytecode  : {bytecode_bytes} bytes");
    println!("  Code Hash : {}", code_hash.to_hex());
    let (outcome, _instance) = ContractInstance::<RpcProvider, KeystoreSigner>::deploy(
        provider, signer, request,
    )
    .map_err(|e| format!("Deploy gagal: {e}"))?;

    if !auto {
        println!("{}", outcome.prompt);
        if !confirm(rest, "Setujui deploy kontrak ini?")? {
            println!(
                "{}",
                warn("Deploy dibatalkan oleh pengguna. Tidak ada transaksi disiarkan.")
            );
            return Ok(());
        }
    }

    banner("AURION CONTRACT DEPLOY - HASIL BROADCAST");
    let out = DeployOutput {
        status: "DEPLOYED".to_string(),
        contract_address: outcome.contract_bech32m.clone(),
        tx_id: outcome.tx_id.to_hex(),
        code_hash: outcome.code_hash.to_hex(),
        bytecode_bytes,
        gas_used: outcome.gas_used,
        fee_quanta: outcome.fee.as_u128().to_string(),
        nonce: outcome.nonce,
        mode: "broadcast".to_string(),
    };
    format.print(&out, || {
        println!("  Contract Address : {}", ok(&out.contract_address));
        println!("  Transaction Hash : {}", ok(&out.tx_id));
        println!("  Code Hash        : {}", out.code_hash);
        println!("  Nonce            : {}", out.nonce);
        println!("  Fee              : {} Quanta", out.fee_quanta);
        println!("  Gas (simulasi)   : {}", out.gas_used);
        println!();
        println!(
            "{}",
            dim("  Simpan metadata JSON agar CLI dapat memanggil kontrak ini:")
        );
        println!("{}", dim("    aurion contract metadata <addr> --metadata <file.json>"));
    });
    Ok(())
}

/// Cetak hasil verifikasi statis (dipakai `verify` dan `deploy` offline).
fn print_verify(
    bytecode: &[u8],
    code_hash: &Hash256,
    jumpdests: usize,
    format: OutputFormat,
) -> Result<(), String> {
    let estimated_gas = 50_000 + (bytecode.len() as u64 * 200);
    let out = VerifyOutput {
        status: "VERIFIED_CANONICAL".to_string(),
        bytecode_bytes: bytecode.len(),
        code_hash: code_hash.to_hex(),
        valid_jumpdests: jumpdests,
        estimated_gas,
    };
    format.print(&out, || {
        banner("AURION AVM BYTECODE VERIFICATION");
        println!("  Status          : {}", ok(&out.status));
        println!("  Bytecode Size   : {} bytes", out.bytecode_bytes);
        println!("  Code Hash       : {}", out.code_hash);
        println!("  Valid JumpDests : {}", out.valid_jumpdests);
        println!("  Estimated Gas   : {}", out.estimated_gas);
        println!();
        println!("{}", dim("  Mode offline: tidak ada transaksi disiarkan."));
        println!(
            "{}",
            dim("  Tambahkan --keystore <path> untuk deploy sungguhan.")
        );
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Subcommand: call / query
// ---------------------------------------------------------------------------

/// Opsi yang dikumpulkan bersama untuk `call` dan `query`.
struct Invocation {
    address: Address,
    address_bech32m: String,
    method: MethodAbi,
    args: Vec<AbiValue>,
    metadata: ContractMetadata,
    provider: RpcProvider,
}

/// Siapkan seluruh konteks pemanggilan: alamat, metadata, metode, dan argumen.
///
/// Validasi `code_hash` terhadap akun on-chain dilakukan di sini sehingga
/// metadata palsu tertolak **sebelum** Dry-Run dilakukan.
///
/// # Inputs
/// - `args`: argumen baris perintah.
/// - `rest`: argumen posisional setelah nama subcommand (addr, method, args..).
///
/// # Outputs
/// - `Ok(inv)`: konteks pemanggilan siap dipakai.
/// - `Err(String)`: alamat/metode/argumen/metadata bermasalah.
///
/// # Errors
/// Alamat tidak valid, akun bukan kontrak, metadata tidak ditemukan atau tidak
/// cocok dengan `code_hash` on-chain, atau argumen tidak cocok signature.
fn prepare_invocation(args: &[String]) -> Result<Invocation, String> {
    // Posisional dipisah dari flag agar alamat/metode/argumen mudah dibaca.
    let positionals_in = positionals(args, &["yes", "y", "auto-approve"]);
    let rest = &positionals_in[..];
    let addr_str = rest
        .first()
        .ok_or_else(|| "Alamat kontrak wajib diisi: aurion contract <call|query> <address> <method> [args...]".to_string())?;
    let method_name = rest
        .get(1)
        .ok_or_else(|| format!("Nama metode wajib diisi untuk kontrak {addr_str}"))?;
    let arg_positionals = rest[2..].to_vec();

    let address = parse_address(addr_str)?;
    let address_bech32m = encode_address_bech32m(&address, "aur")
        .map_err(|e| format!("Gagal meng-encode alamat kontrak: {e}"))?;

    let provider = RpcProvider::new(rpc_url(args));

    // 1. Baca akun on-chain: memastikan target memang kontrak dan mengambil code_hash.
    let account = provider
        .get_account(&address)
        .map_err(|e| format!("Gagal membaca akun kontrak dari simpul: {e}"))?;
    let code_hash = account.code_hash.ok_or_else(|| {
        format!("{address_bech32m} bukan kontrak on-chain (tidak ada code_hash)")
    })?;

    // 2. Resolusi metadata (file lokal, lalu registry simpul).
    let metadata = resolve_metadata(args, &provider, &code_hash)?;

    // 3. Anti-penipuan: metadata wajib terikat ke code_hash on-chain.
    metadata
        .verify_binding(Some(&code_hash))
        .map_err(|e| format!("Binding metadata gagal: {e}"))?;

    // 4. Resolusi metode & argumen.
    let method = metadata
        .find_method(method_name)
        .ok_or_else(|| {
            let available: Vec<String> = metadata.methods.iter().map(|m| m.name.clone()).collect();
            format!(
                "Metode '{method_name}' tidak ada di metadata. Tersedia: {}",
                if available.is_empty() {
                    "(tidak ada)".to_string()
                } else {
                    available.join(", ")
                }
            )
        })?
        .clone();

    let values = collect_method_args(args, &arg_positionals, &method)?;

    Ok(Invocation {
        address,
        address_bech32m,
        method,
        args: values,
        metadata,
        provider,
    })
}

/// Tentukan nilai kembalian yang dapat didekode ke tipe ABI keluaran.
fn decode_return(method: &MethodAbi, return_data: &[u8]) -> Option<String> {
    let out_type = method.outputs.first().map(|p| p.ty)?;
    if return_data.len() != 32 {
        return None;
    }
    let mut word = [0u8; 32];
    word.copy_from_slice(return_data);
    out_type.decode_word(&word).ok().map(|v| v.render())
}

/// Cetak hasil simulasi dalam bentuk manusiawi.
fn print_simulation(
    method: &MethodAbi,
    address: &Address,
    address_bech32m: &str,
    simulation: &crate::state::sandbox::DryRunReport,
    decoded: &Option<String>,
) {
    banner("LANGKAH 1/2 - HASIL SIMULASI (DRY-RUN, TIDAK DISIARKAN)");
    if simulation.success {
        println!("  Status        : {}", ok("SIMULATION SUCCESS"));
    } else {
        println!("  Status        : {}", bad("REVERTED"));
        println!(
            "  Reason        : {}",
            simulation
                .reason
                .as_deref()
                .unwrap_or("(tidak diberikan)")
        );
    }
    println!("  Contract     : {}", dim(address_bech32m));
    println!("  Account Hex  : {}", dim(&address.to_hex()));
    println!("  Method        : {}", method.signature);
    println!("  Gas Used      : {}", simulation.gas_used);
    println!("  Return Data   : 0x{}", hex::encode(&simulation.return_data));
    if let Some(d) = decoded {
        println!("  Decoded       : {d}");
    }
    println!();
    println!("{}", dim("  Belum ada transaksi yang disiarkan."));
}

/// `aurion contract call <address> <method> [args...] [options]`
///
/// Pipeline penuh: dry-run -> tampilkan hasil simulasi -> clear signing ->
/// broadcast. Tidak ada penandatanganan sebelum pengguna melihat simulasi.
fn cmd_call(args: &[String], format: OutputFormat) -> Result<(), String> {
    let inv = prepare_invocation(args)?;
    let opts = call_options(args)?;
    let auto = has_flag(args, "yes") || has_flag(args, "auto-approve");

    // Salin字段 yang dibutuhkan setelah `ContractInstance` mengambil kepemilikan.
    let method_name = inv.method.name.clone();
    let contract_bech32m = inv.address_bech32m.clone();
    let method_abi = inv.method.clone();

    // Keystore hanya dibuka setelah mode persetujuan ditentukan.
    let signer = open_signer(
        args,
        if auto {
            ApprovalMode::AutoApprove
        } else {
            ApprovalMode::Interactive
        },
    )?;

    let instance = ContractInstance::<RpcProvider, KeystoreSigner>::new(
        &contract_bech32m,
        inv.metadata,
        inv.provider,
        signer,
    )
    .map_err(|e| format!("Gagal menyiapkan instance kontrak: {e}"))?;

    // Tahap 1: simulasi read-only lewat aur_call.
    let simulation = instance
        .read(&method_name, &inv.args, &opts)
        .map_err(|e| format!("Simulasi gagal: {e}"))?;
    let decoded = decode_return(&method_abi, &simulation.return_data);

    let sim_out = CallOutput {
        status: if simulation.success {
            "simulation_success".to_string()
        } else {
            "reverted".to_string()
        },
        contract_address: inv.address_bech32m.clone(),
        method: inv.method.signature.clone(),
        gas_used: simulation.gas_used,
        return_data: hex::encode(&simulation.return_data),
        decoded: decoded.clone(),
        reason: simulation.reason.clone(),
        tx_id: None,
        fee_quanta: opts.fee.map(|f| f.as_u128().to_string()),
    };
    format.print(&sim_out, || {
        print_simulation(
            &method_abi,
            &inv.address,
            &contract_bech32m,
            &simulation,
            &decoded,
        )
    });

    if !simulation.success {
        return Err("Simulasi kontrak gagal: tidak ada transaksi disiarkan.".to_string());
    }

    // Tahap 2: konfirmasi clear signing lalu broadcast.
    if !auto {
        let summary = format!(
            "Setujui pemanggilan '{}' dengan fee {} Quanta?",
            inv.method.signature,
            opts.fee
                .map(|f| f.as_u128().to_string())
                .unwrap_or_else(|| "otomatis".to_string())
        );
        if !confirm(args, &summary)? {
            println!(
                "{}",
                warn("Pemanggilan dibatalkan. Tidak ada transaksi disiarkan.")
            );
            return Ok(());
        }
    }

    let outcome = instance
        .call(&inv.method.name, &inv.args, &opts)
        .map_err(|e| format!("Broadcast gagal: {e}"))?;

    banner("LANGKAH 2/2 - HASIL BROADCAST");
    let out = CallOutput {
        status: "broadcast".to_string(),
        contract_address: inv.address_bech32m.clone(),
        method: inv.method.signature.clone(),
        gas_used: outcome.gas_used,
        return_data: hex::encode(&outcome.return_data),
        decoded,
        reason: None,
        tx_id: Some(outcome.tx_id.to_hex()),
        fee_quanta: Some(outcome.fee.as_u128().to_string()),
    };
    format.print(&out, || {
        println!(
            "  Transaction Hash : {}",
            ok(out.tx_id.as_deref().unwrap_or_default())
        );
        println!("  Nonce            : {}", outcome.nonce);
        println!(
            "  Fee              : {} Quanta",
            out.fee_quanta.as_deref().unwrap_or_default()
        );
        println!("  Gas (simulasi)   : {}", out.gas_used);
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Subcommand: query (read-only)
// ---------------------------------------------------------------------------

/// `aurion contract query <address> <method> [args...] [options]`
///
/// Sepenuhnya read-only: menjalankan `aur_call` tanpa menandatangani dan tanpa
/// menyiarkan apa pun. Tidak ada keystore yang dibuka.
fn cmd_query(args: &[String], format: OutputFormat) -> Result<(), String> {
    let inv = prepare_invocation(args)?;
    let opts = call_options(args)?;

    // View caller deterministik dengan mode Reject: `read()` berhenti sebelum
    // signing, sehingga tidak ada materi kunci yang dipakai.
    let view_signer = if flag(args, "keystore").is_some() {
        open_signer(args, ApprovalMode::Reject)?
    } else {
        KeystoreSigner::from_seed(VIEW_CALLER_SEED, ApprovalMode::Reject)
    };

    let instance = ContractInstance::<RpcProvider, KeystoreSigner>::new(
        &inv.address_bech32m,
        inv.metadata,
        inv.provider,
        view_signer,
    )
    .map_err(|e| format!("Gagal menyiapkan instance kontrak: {e}"))?;

    let simulation = instance
        .read(&inv.method.name, &inv.args, &opts)
        .map_err(|e| format!("Simulasi query gagal: {e}"))?;
    let decoded = decode_return(&inv.method, &simulation.return_data);

    let out = CallOutput {
        status: if simulation.success {
            "success".to_string()
        } else {
            "reverted".to_string()
        },
        contract_address: inv.address_bech32m.clone(),
        method: inv.method.signature.clone(),
        gas_used: simulation.gas_used,
        return_data: hex::encode(&simulation.return_data),
        decoded: decoded.clone(),
        reason: simulation.reason.clone(),
        tx_id: None,
        fee_quanta: None,
    };
    format.print(&out, || {
        banner("AURION CONTRACT QUERY (READ-ONLY)");
        print_simulation(
            &inv.method,
            &inv.address,
            &inv.address_bech32m,
            &simulation,
            &decoded,
        );
        println!(
            "{}",
            dim("  Read-only: tidak ada tanda tangan atau broadcast yang terjadi.")
        );
    });

    if !simulation.success {
        return Err("Query kontrak revert.".to_string());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Subcommand: metadata / publish-metadata
// ---------------------------------------------------------------------------

/// `aurion contract metadata <address|code_hash> [--metadata <file>]`
///
/// Menampilkan ABI kontrak beserta selector turunan dan `code_hash`.
fn cmd_metadata(args: &[String], format: OutputFormat) -> Result<(), String> {
    let target = args.first().ok_or_else(|| {
        "Alamat kontrak atau code_hash wajib diisi: aurion contract metadata <address|code_hash> [--metadata <file>]".to_string()
    })?;
    let provider = RpcProvider::new(rpc_url(args));

    let (source, metadata) = if looks_like_code_hash(target) {
        let code_hash = parse_code_hash(target)?;
        let meta = resolve_metadata(args, &provider, &code_hash)?;
        ("node-registry".to_string(), meta)
    } else {
        let address = parse_address(target)?;
        let bech32m = encode_address_bech32m(&address, "aur")
            .map_err(|e| format!("Gagal meng-encode alamat: {e}"))?;
        let account = provider
            .get_account(&address)
            .map_err(|e| format!("Gagal membaca akun dari simpul: {e}"))?;
        let code_hash = account
            .code_hash
            .ok_or_else(|| format!("{bech32m} bukan kontrak on-chain (tidak ada code_hash)"))?;
        let meta = resolve_metadata(args, &provider, &code_hash)?;
        // Anti-penipuan: metadata harus cocok dengan code_hash on-chain.
        meta.verify_binding(Some(&code_hash))
            .map_err(|e| format!("Binding metadata gagal: {e}"))?;
        let src = if flag(args, "metadata").is_some() {
            "local-file"
        } else {
            "node-registry"
        };
        (src.to_string(), meta)
    };

    let methods: Vec<MethodOutput> = metadata
        .methods
        .iter()
        .map(|m| MethodOutput {
            name: m.name.clone(),
            signature: m.signature.clone(),
            selector: hex::encode(m.selector),
            inputs: m.inputs.iter().map(|p| p.ty.label().to_string()).collect(),
            payable: m.payable,
        })
        .collect();

    let out = MetadataOutput {
        source: source.clone(),
        name: metadata.name.clone(),
        chain_id: metadata.chain_id,
        address: metadata.address.clone(),
        code_hash: metadata.code_hash.clone(),
        method_count: methods.len(),
        methods: methods.clone(),
    };

    format.print(&out, || {
        banner("AURION CONTRACT METADATA");
        println!("  Name        : {}", out.name);
        println!("  Address     : {}", out.address);
        println!("  Chain ID    : {}", out.chain_id);
        println!("  Code Hash   : {}", out.code_hash);
        println!("  Runtime     : {} bytes (hex)", metadata.runtime.len() / 2);
        println!("  Source      : {}", info(&out.source));
        println!("  Methods     : {}", out.method_count);
        for m in &out.methods {
            let extra = if m.inputs.is_empty() {
                String::new()
            } else {
                format!("  args=[{}]", m.inputs.join(", "))
            };
            println!(
                "    - {}  {}  selector=0x{}{}",
                m.name,
                dim(&format!("({})", m.signature)),
                m.selector,
                extra
            );
        }
    });
    Ok(())
}

/// `aurion contract publish-metadata <code_hash> --metadata <file.json>`
///
/// Mendaftarkan metadata ke registry off-chain simpul
/// (`aur_sendContractMetadata`). Registry in-memory: hilang saat restart.
fn cmd_publish_metadata(args: &[String], format: OutputFormat) -> Result<(), String> {
    let target = args.first().ok_or_else(|| {
        "code_hash wajib diisi: aurion contract publish-metadata <code_hash> --metadata <file.json>"
            .to_string()
    })?;
    let code_hash = parse_code_hash(target)?;
    let path = flag(args, "metadata")
        .ok_or_else(|| "Flag --metadata <file.json> wajib diisi".to_string())?;
    let metadata = load_metadata_file(&path)?;

    metadata
        .verify_binding(Some(&code_hash))
        .map_err(|e| format!("Metadata tidak cocok dengan code_hash: {e}"))?;

    let provider = RpcProvider::new(rpc_url(args));
    provider
        .publish_metadata(&code_hash, &metadata)
        .map_err(|e| format!("Gagal mendaftarkan metadata ke simpul: {e}"))?;

    let out = serde_json::json!({
        "status": "published",
        "code_hash": code_hash.to_hex(),
        "address": metadata.address,
        "method_count": metadata.methods.len(),
        "source": "node-registry",
    });
    format.print(&out, || {
        banner("AURION CONTRACT METADATA PUBLISHED");
        println!("  Status     : {}", ok("published"));
        println!("  Code Hash  : {}", code_hash.to_hex());
        println!("  Address    : {}", metadata.address);
        println!("  Methods    : {}", metadata.methods.len());
        println!();
        println!("{}", warn("  Registry in-memory: hilang saat restart simpul."));
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Inspect (dipertahankan untuk kompatibilitas mundur)
// ---------------------------------------------------------------------------

/// `aurion contract inspect <address> [--db-path <path>] [options]`
///
/// Membaca state kontrak dari penyimpanan lokal redb. Disarankan memakai
/// `aurion contract metadata` yang membaca state on-chain via RPC.
fn cmd_inspect(args: &[String], format: OutputFormat) -> Result<(), String> {
    let target = args
        .first()
        .ok_or_else(|| "Alamat wajib diisi: aurion contract inspect <address>".to_string())?;
    let address = parse_address(target)?;

    let db_path = flag(args, "db-path").unwrap_or_else(|| "data/aurion.redb".to_string());
    let account = crate::storage::RedbStorageEngine::open_or_create(&db_path)
        .ok()
        .and_then(|store| crate::storage::StateStore::get_account(&store, &address).ok().flatten());

    let (balance, nonce, code_hash, storage_root, is_contract) = match account {
        Some(acc) => (
            acc.balance,
            acc.nonce,
            acc.code_hash.map(|h| h.to_hex()),
            acc.storage_root.map(|h| h.to_hex()),
            acc.is_contract(),
        ),
        None => (Quantum::ZERO, 0, None, None, false),
    };

    let out = serde_json::json!({
        "address": target,
        "is_contract": is_contract,
        "code_hash": code_hash,
        "storage_root": storage_root,
        "balance_aur": super::metadata::format_quanta(balance.as_u128()),
        "nonce": nonce,
        "source": "local-redb",
    });
    format.print(&out, || {
        println!("  Address      : {target}");
        println!("  Is Contract  : {is_contract}");
        println!("  Code Hash    : {}", code_hash.as_deref().unwrap_or("None"));
        println!(
            "  Storage Root : {}",
            storage_root.as_deref().unwrap_or("None")
        );
        println!(
            "  Balance      : {} AUR",
            super::metadata::format_quanta(balance.as_u128())
        );
        println!("  Nonce        : {nonce}");
        println!();
        println!("{}", dim("  Sumber: penyimpanan lokal redb (bukan on-chain)."));
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Bantuan
// ---------------------------------------------------------------------------

/// Cetak teks bantuan `aurion contract`.
#[allow(clippy::too_many_lines)]
pub fn print_contract_help() {
    println!("================================================================================");
    println!("               AURION SMART CONTRACT (AVM) COMMAND REFERENCE                   ");
    println!("================================================================================");
    println!("Usage: aurion contract <subcommand> [options]");
    println!();
    println!("Subcommands:");
    println!("  deploy            Deploy kontrak baru via Contract SDK (offline bila tanpa --keystore)");
    println!("  call              Panggil metode: simulasi -> clear signing -> broadcast");
    println!("  query             Panggil metode read-only (tanpa tanda tangan / broadcast)");
    println!("  metadata          Tampilkan ABI, selector, dan code_hash kontrak");
    println!("  publish-metadata  Daftarkan metadata ke registry off-chain simpul");
    println!("  inspect           Baca state kontrak dari penyimpanan lokal redb");
    println!("  verify            Verifikasi bytecode AVM secara statis (offline)");
    println!("  help              Tampilkan bantuan ini");
    println!();
    println!("Common Options:");
    println!("  --rpc <url>            URL JSON-RPC simpul (default: http://127.0.0.1:8545)");
    println!("  --keystore <path>      Berkas keystore Argon2 (wajib untuk call/deploy)");
    println!("  --password-stdin       Baca password dari stdin");
    println!("  --metadata <file>      Metadata kontrak dalam JSON");
    println!("  --yes, -y              Persetujuan otomatis (CI/scripting, tanpa prompt)");
    println!("  --auto-approve         Alias dari --yes");
    println!("  --output, -o [text|json]  Format keluaran (AUR-CLI-007)");
    println!();
    println!("Deploy:");
    println!("  aurion contract deploy <file.avm|hex> [--runtime <file>] [--name <n>]");
    println!("                          [--value <quanta>] [--fee <quanta>] [--nonce <n>]");
    println!("                          [--metadata <file.json>] [--keystore <path>] [--yes]");
    println!("      Tanpa --keystore -> hanya verifikasi statis offline (tidak broadcast)");
    println!();
    println!("Call (mengubah state):");
    println!("  aurion contract call <address> <method> [args...] [options]");
    println!("      --args '[1,2]'        Argumen sebagai JSON array");
    println!("      --args <v> (berulang)  Argumen terpisah spasi");
    println!("      --value <quanta>       Nilai Quanta untuk metode payable");
    println!("      --fee, --nonce         Override fee / nonce on-chain");
    println!();
    println!("Query (read-only, tanpa keystore):");
    println!("  aurion contract query <address> <method> [args...] [--args '[...]']");
    println!();
    println!("Metadata:");
    println!("  aurion contract metadata <address|code_hash> [--metadata <file.json>]");
    println!("  aurion contract publish-metadata <code_hash> --metadata <file.json>");
    println!();
    println!("Verify (offline):");
    println!("  aurion contract verify <file.avm|hex>");
    println!();
    println!("Tipe argumen yang didukung (dari metadata ABI):");
    println!("  address   Bech32m (aur1.../aurt1...) atau hex 64 karakter");
    println!("  quantum   bilangan bulat Quanta, atau desimal AUR (mis. 1.5)");
    println!("  u64/u32   bilangan bulat desimal");
    println!("  bool      true/false/1/0/yes/no");
    println!("  hash256   hex 64 karakter");
    println!();
    println!("Contoh:");
    println!("  aurion contract verify ./build/echo.avm");
    println!("  aurion contract metadata aur1... --metadata echo.json");
    println!("  aurion contract query   aur1... balanceOf 7");
    println!("  aurion contract call    aur1... transfer 1000000000 --keystore my.keystore.json");
    println!("================================================================================");
}
/// Parse `code_hash` hex 32-byte, opsional berprefiks `0x`.
///
/// # Inputs
/// - `raw`: string hex 32-byte.
///
/// # Outputs
/// - `Ok(hash)`: digest 32-byte.
/// - `Err(String)`: panjang atau format hex salah.
///
/// # Errors
/// Panjang bukan 64 karakter hex atau hex rusak.
fn parse_code_hash(raw: &str) -> Result<Hash256, String> {
    let cleaned = raw.trim().trim_start_matches("0x");
    if cleaned.len() != 64 {
        return Err(format!(
            "code_hash harus 64 hex karakter, diterima {}",
            cleaned.len()
        ));
    }
    let mut arr = [0u8; 32];
    hex::decode_to_slice(cleaned, &mut arr)
        .map_err(|e| format!("code_hash '{raw}' tidak valid: {e}"))?;
    Ok(Hash256::from_bytes(arr))
}

/// Apakah argumen tersebut `code_hash` (hex 64) alih-alih alamat kontrak.
fn looks_like_code_hash(raw: &str) -> bool {
    let t = raw.trim().trim_start_matches("0x");
    t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit())
}

/// Susun opsi panggilan dari flag baris perintah.
///
/// # Inputs
/// - `args`: argumen baris perintah.
///
/// # Outputs
/// - `Ok(opts)`: opsi terisi dari flag yang ada (nilai lain memakai default).
/// - `Err(String)`: nilai flag numerik tidak valid.
///
/// # Errors
/// `--value`, `--fee`, atau `--nonce` bukan bilangan yang valid.
fn call_options(args: &[String]) -> Result<CallOptions, String> {
    let mut opts = CallOptions::default();
    if let Some(v) = flag(args, "value") {
        opts.value = Quantum::new(
            v.parse::<u128>()
                .map_err(|_| format!("--value '{v}' bukan bilangan bulat Quanta yang valid"))?,
        );
    }
    if let Some(f) = flag(args, "fee") {
        opts.fee = Some(Quantum::new(
            f.parse::<u128>()
                .map_err(|_| format!("--fee '{f}' bukan bilangan bulat Quanta yang valid"))?,
        ));
    }
    if let Some(n) = flag(args, "nonce") {
        opts.nonce = Some(
            n.parse::<u64>()
                .map_err(|_| format!("--nonce '{n}' bukan integer yang valid"))?,
        );
    }
    Ok(opts)
}

/// Resolusi metadata kontrak dengan urutan prioritas yang dapat diprediksi.
///
/// 1. `--metadata <file>` (lokal, otoritatif untuk pengujian offline).
/// 2. Registry off-chain simpul via `aur_getContractMetadata`.
///
/// Bila keduanya gagal, pesan galat menjelaskan cara memperbaikinya — termasuk
/// bahwa metadata memang off-chain dan dapat di-seed lewat
/// `aur_sendContractMetadata`.
///
/// # Inputs
/// - `args`: argumen baris perintah.
/// - `provider`: provider RPC untuk pencarian registry.
/// - `code_hash`: `code_hash` kontrak target.
///
/// # Outputs
/// - `Ok(metadata)`: metadata kontrak tervalidasi.
/// - `Err(String)`: tidak ditemukan di kedua sumber.
///
/// # Errors
/// Metadata lokal rusak, atau registry tidak memiliki entri tersebut.
fn resolve_metadata(
    args: &[String],
    provider: &RpcProvider,
    code_hash: &Hash256,
) -> Result<ContractMetadata, String> {
    if let Some(path) = flag(args, "metadata") {
        return load_metadata_file(&path);
    }
    match provider.fetch_metadata(code_hash) {
        Ok(Some(meta)) => Ok(meta),
        Ok(None) => Err(format!(
            "Metadata kontrak (code_hash {}) tidak ditemukan.\n  \
             Metadata bersifat OFF-CHAIN. Pilih salah satu:\n    \
             1) --metadata <file.json>  (metadata lokal, terikat code_hash)\n    \
             2) daftarkan ke simpul lebih dulu:\n       \
             aurion contract publish-metadata <code_hash> --metadata <file.json>",
            code_hash.to_hex()
        )),
        Err(e) => Err(format!("Gagal mengambil metadata dari simpul: {e}")),
    }
}

/// URL RPC node; default devnet lokal.
fn rpc_url(args: &[String]) -> String {
    flag(args, "rpc")
        .or_else(|| flag(args, "rpc-url"))
        .unwrap_or_else(|| "http://127.0.0.1:8545".to_string())
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

/// Titik masuk `aurion contract`.
///
/// Mengembalikan `Err(String)` berisi pesan ramah pengguna untuk seluruh
/// kegagalan; tidak pernah panik pada kesalahan jaringan/parsing.
///
/// # Inputs
/// - `args`: argumen setelah `contract` (tanpa nama subcommand).
/// - `format`: format keluaran yang diminta.
///
/// # Outputs
/// - `Ok(())`: perintah selesai.
/// - `Err(String)`: pesan galat siap ditampilkan ke pengguna.
pub fn handle_contract_subcommand(args: &[String], format: OutputFormat) -> Result<(), String> {
    let sub = args.first().map(String::as_str).unwrap_or("help");
    let rest = &args[args.len().min(1)..];

    let result = match sub {
        "deploy" => cmd_deploy(rest, format),
        "call" => cmd_call(rest, format),
        "query" => cmd_query(rest, format),
        "metadata" | "abi" => cmd_metadata(rest, format),
        "publish-metadata" => cmd_publish_metadata(rest, format),
        "inspect" => cmd_inspect(rest, format),
        "verify" => {
            let positional = positionals(rest, &["verify-only"]);
            let Some(spec) = positional.first().cloned() else {
                return Err(
                    "Bytecode wajib diisi: aurion contract verify <file.avm|hex>".to_string(),
                );
            };
            let bytecode = load_bytecode(&spec)?;
            let verified = crate::vm::verifier::BytecodeVerifier::verify(&bytecode)
                .map_err(|e| format!("Verifikasi bytecode gagal: {e}"))?;
            let code_hash = crate::crypto::blake3_hash(&bytecode);
            print_verify(&bytecode, &code_hash, verified.valid_jump_dests.len(), format)
        }
        "help" | "--help" | "-h" => {
            print_contract_help();
            Ok(())
        }
        other => {
            return Err(format!(
                "Subcommand kontrak '{other}' tidak dikenal. Jalankan 'aurion contract help'."
            ));
        }
    };

    // Cetak galat ke stderr dengan prefix seragam, lalu teruskan ke pemanggil
    // agar `main` tetap keluar dengan kode status non-zero.
    if let Err(e) = &result {
        eprintln!("{ERR} {e}");
    }
    result
}

