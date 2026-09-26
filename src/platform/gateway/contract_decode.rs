#![forbid(unsafe_code)]

//! Layanan Dekoder Interaksi Kontrak Cerdas untuk Explorer Aurion.
//!
//! Menerjemahkan payload `ContractCall`/`ContractDeploy` yang tervalidasi
//! AVM-005 menjadi bentuk yang dapat dibaca manusia: nama metode, argumen
//! ter-decode, dan status eksekusi — untuk transparansi & auditability jaringan.
//!
//! Menghormati Invariant:
//! - AUR-ARCH-001: bagian dari biner tunggal `/bin/aurion`.
//! - AUR-ARCH-011: nol kode `unsafe`.
//! - AUR-ARCH-012: nol floating-point; seluruh nilai kuantitatif integer
//!   (`Quantum` u128) dan string.
//! - AUR-ARCH-004: logika dekoder TIDAK menduplikasi aturan semantik; ia memakai
//!   tipe ABI kanonik dari `contract::metadata`.
//!
//! ## Bentuk payload AVM (lihat `contract::calldata`)
//!
//! ```text
//! payload := PUSH32 argN ... PUSH32 arg1  PUSH4 selector  <runtime script>
//!            \___ argumen (urutan terbalik) ___/  \___ top ___/
//! ```
//!
//! Argumen didorong **terbalik** agar selector berada di puncak stack saat
//! runtime mulai dieksekusi. Karena itu decoder harus:
//!
//! 1. Membaca stream instruksi dari depan (urutan argumen terbalik),
//! 2. Membalik hasilnya agar sesuai urutan parameter metadata,
//! 3. Mengambil 4 byte selector setelah seluruh argumen.
//!
//! ### Menghindari ambiguitas
//!
//! Script runtime bisa saja diawali `PUSH32`, sehingga pemindaian buta
//! ("selama opcode == PUSH32, anggap itu argumen") tidak sound. Decoder
//! memb conundrum ini dengan mencoba **setiap metode pada metadata** sebagai
//! hipotesis arity, lalu mencocokkan selector hasil framing dengan selector
//! metode tersebut. Kecocokan bersifat eksak dan tidak ambigu. Bila metadata
//! tidak tersedia, decoder jatuh ke pemindaian heuristik terbatas dan
//! menandai hasilnya `metadata_missing` sehingga tidak diklaim tervalidasi.

use crate::contract::metadata::{AbiType, ContractMetadata, MethodAbi};
use crate::core::Address;
use crate::gateway::rpc::methods::RpcContext;
use crate::transaction::types::{Transaction, TxType};
use crate::vm::opcode::Opcode;
use serde::Serialize;

/// Batas panjang hex payload yang ditampilkan sebelum dipotong (Prevent UI crash).
pub const MAX_RAW_PAYLOAD_CHARS: usize = 4_096;

/// Batas jumlah argumen yang ditampilkan pada UI.
pub const MAX_DISPLAYED_ARGS: usize = 8;

/// Batas jumlah metode yang dicoba saat mencocokkan framing.
const MAX_METHOD_PROBE: usize = 64;

/// Status dekoder payload kontrak.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodeStatus {
    /// Berhasil ter-decode penuh: metode + seluruh argumen dari metadata.
    Decoded,
    /// Metadata ada tetapi selector tidak cocok dengan frame mana pun.
    UnknownMethod,
    /// Metadata tidak terdaftar di registry; selector dib heuristik.
    MetadataMissing,
    /// Transaksi bukan tipe kontrak sama sekali.
    NotAContract,
}

/// Status eksekusi transaksi kontrak pada explorer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TxStatus {
    /// Masih antre di mempool, belum masuk blok.
    Pending,
    /// Sudah tercakup dalam blok yang tercatat.
    Finalized,
}

/// Satu argumen metode yang sudah ter-decode ke tipe ABI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecodedArgument {
    /// Indeks argumen sesuai urutan parameter metadata.
    pub index: usize,
    /// Nama parameter dari metadata (bila tersedia).
    pub name: String,
    /// Tipe ABI parameter.
    pub abi_type: String,
    /// Nilai ter-format siap tampil (mis. `42`, `aur1...`, `1.000000000 AUR`).
    pub value: String,
    /// Word 32-byte mentah untuk audit lanjutan.
    pub raw_word: String,
}

/// Deskripsi lengkap interaksi kontrak untuk ditampilkan Explorer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContractInteraction {
    /// `call`, `deploy`, atau `none` bila transaksi bukan kontrak.
    pub kind: String,
    /// Status eksekusi yang diketahui explorer.
    pub status: TxStatus,
    /// Alamat kontrak tujuan (untuk `call`) atau alamat hasil deploy.
    pub contract_address: String,
    /// Nama kontrak dari metadata bila terdaftar.
    pub contract_name: Option<String>,
    /// `code_hash` kontrak on-chain (hex 64) bila dapat ditentukan.
    pub code_hash: Option<String>,
    /// Signature metode kanonik, mis. `transfer(u64)`.
    pub method: Option<String>,
    /// Nama metode tanpa signature.
    pub method_name: Option<String>,
    /// Selector 4-byte hex dengan prefiks `0x`.
    pub selector: Option<String>,
    /// Argumen ter-decode.
    pub arguments: Vec<DecodedArgument>,
    /// Jumlah byte script runtime setelah selector (informasi audit).
    pub runtime_bytes: usize,
    /// Jumlah byte bytecode konstruktor (khusus deploy).
    pub bytecode_bytes: usize,
    /// Payload mentah (hex, mungkin dipotong).
    pub raw_payload: String,
    /// Panjang payload penuh dalam byte.
    pub raw_payload_bytes: usize,
    /// `true` bila `raw_payload` telah dipotong.
    pub raw_payload_truncated: bool,
    /// Hasil dekoder.
    pub decode_status: DecodeStatus,
    /// Penjelasan bila tidak ter-decode penuh (null bila sukses).
    pub reason: Option<String>,
}

/// Hasil parsing satu framing AVM Call Frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallFrame {
    /// Word argumen dalam urutan TEMUAN (terbalik terhadap urutan metadata).
    pub words: Vec<[u8; 32]>,
    /// Selector 4-byte.
    pub selector: [u8; 4],
    /// Jumlah byte script runtime setelah selector.
    pub runtime_bytes: usize,
}

/// Coba parse payload sebagai AVM Call Frame dengan asumsi jumlah argumen tertentu.
///
/// Fungsi ini murni dan tidak menyentuh state, sehingga aman dipanggil untuk
/// setiap metode pada metadata sebagai hipotesis competing.
///
/// # Inputs
/// - `payload`: byte payload transaksi kontrak.
/// - `arg_count`: jumlah argumen yang diasumsikan.
///
/// # Outputs
/// - `Ok(Some(frame))`: framing cocok; `words` dalam urutan terbalik.
/// - `Ok(None)`: payload tidak berbentuk frame dengan arity tersebut.
/// - `Err(String)`: payload terlalu pendek untuk arity itu.
///
/// # Errors
/// Payload lebih pendek daripada yang dibutuhkan arity tersebut.
pub fn parse_call_frame(payload: &[u8], arg_count: usize) -> Result<Option<CallFrame>, String> {
    let mut cursor = 0usize;
    let mut words: Vec<[u8; 32]> = Vec::with_capacity(arg_count);

    for _ in 0..arg_count {
        if cursor >= payload.len() {
            return Err("payload berakhir sebelum argumen lengkap".to_string());
        }
        if payload[cursor] != Opcode::Push32 as u8 {
            // Bukan PUSH32: framing dengan arity ini tidak cocok. Sisa payload
            // adalah script runtime, bukan argumen.
            return Ok(None);
        }
        let start = cursor + 1;
        let end = start + 32;
        if end > payload.len() {
            return Err("payload terpotong di tengah argumen PUSH32".to_string());
        }
        let mut word = [0u8; 32];
        word.copy_from_slice(&payload[start..end]);
        words.push(word);
        cursor = end;
    }

    if cursor >= payload.len() || payload[cursor] != Opcode::Push4 as u8 {
        return Ok(None);
    }
    let sel_start = cursor + 1;
    let sel_end = sel_start + 4;
    if sel_end > payload.len() {
        return Err("payload terpotong di tengah selector PUSH4".to_string());
    }
    let mut selector = [0u8; 4];
    selector.copy_from_slice(&payload[sel_start..sel_end]);

    Ok(Some(CallFrame {
        words,
        selector,
        runtime_bytes: payload.len() - sel_end,
    }))
}

/// Render satu word 32-byte ke nilai ABI yang dapat ditampilkan.
fn decode_word(ty: AbiType, word: &[u8; 32]) -> String {
    ty.decode_word(word)
        .map(|v| v.render())
        .unwrap_or_else(|_| "<tidak dapat didecode>".to_string())
}

/// Susun daftar argumen ter-decode dari frame + deklarasi metode.
///
/// `frame.words` berurutan terbalik (hasil push), sedangkan `method.inputs`
/// berurutan deklarasi; keduanya dibalik agar sejajar.
fn build_arguments(frame: &CallFrame, method: &MethodAbi) -> Vec<DecodedArgument> {
    let ordered: Vec<[u8; 32]> = frame.words.iter().rev().copied().collect();
    let mut out = Vec::new();
    for (index, word) in ordered.iter().enumerate().take(MAX_DISPLAYED_ARGS) {
        let (name, abi_type, value) = match method.inputs.get(index) {
            Some(p) => (
                p.name.clone(),
                p.ty.label().to_string(),
                decode_word(p.ty, word),
            ),
            None => (
                format!("arg{index}"),
                "unknown".to_string(),
                format!("0x{}", hex::encode(word)),
            ),
        };
        out.push(DecodedArgument {
            index,
            name,
            abi_type,
            value,
            raw_word: format!("0x{}", hex::encode(word)),
        });
    }
    out
}

/// Potong hex payload agar aman ditampilkan (mencegah UI crash pada calldata besar).
///
/// Batas diterapkan pada **jumlah karakter hasil hex**, dan penanda ellipsis
/// memakai ASCII (`...`) agar `String::len()` (byte) tetap sama dengan
/// jumlah karakter — penting agar UI tidak salah menghitung ukuran buffer.
fn truncate_hex_payload(payload: &[u8]) -> (String, bool) {
    let full = hex::encode(payload);
    if full.len() <= MAX_RAW_PAYLOAD_CHARS {
        return (full, false);
    }
    // Jaga kelipatan pasangan hex: potong pada batas byte lengkap.
    let keep = (MAX_RAW_PAYLOAD_CHARS - 3) / 2 * 2;
    (format!("{}...", &full[..keep]), true)
}

/// Pemindaian heuristik frame tanpa metadata (best-effort, tidak diklaim tervalidasi).
fn heuristic_frame(payload: &[u8]) -> (Option<[u8; 4]>, usize, usize) {
    for arity in 0..=crate::contract::MAX_ABI_INPUTS {
        if let Ok(Some(frame)) = parse_call_frame(payload, arity) {
            return (Some(frame.selector), frame.runtime_bytes, arity);
        }
    }
    (None, payload.len(), 0)
}

/// Alamat kontrak on-chain untuk sebuah transaksi kontrak.
fn contract_binding(tx: &Transaction) -> Address {
    match tx.tx_type {
        // Untuk deploy, alamat hasil deploy diturunkan deterministik dari
        // (sender, nonce) — sama persis dengan STF.
        TxType::ContractDeploy => crate::state::stf::derive_contract_address(&tx.sender, tx.nonce),
        _ => tx.recipient,
    }
}

/// Ambil metadata kontrak dari registry off-chain simpul.
fn lookup_metadata(ctx: &RpcContext, address: Address) -> Option<ContractMetadata> {
    let code_hash = {
        let accounts = ctx.accounts.lock().unwrap_or_else(|e| e.into_inner());
        accounts.get(&address).and_then(|a| a.code_hash)
    }?;
    let raw = ctx.contract_metadata.get(&code_hash)?;
    ContractMetadata::from_json(&raw).ok()
}

/// Bangun deskripsi interaksi kontrak untuk satu transaksi.
///
/// # Inputs
/// - `ctx`: konteks RPC (state akun + registry metadata off-chain).
/// - `tx`: transaksi yang akan dideskripsikan.
/// - `status`: status eksekusi yang diketahui explorer.
///
/// # Outputs
/// Deskripsi lengkap; `kind` bernilai `"none"` bila transaksi bukan kontrak.
///
/// # Errors
/// Tidak ada; kegagalan decoding dilaporkan lewat `decode_status` + `reason`
/// agar UI dapat menampilkannya tanpa menggagalkan halaman.
#[must_use]
pub fn describe_contract_interaction(
    ctx: &RpcContext,
    tx: &Transaction,
    status: TxStatus,
) -> ContractInteraction {
    let (raw_payload, truncated) = truncate_hex_payload(&tx.payload);
    let payload_len = tx.payload.len();

    if !matches!(tx.tx_type, TxType::ContractCall | TxType::ContractDeploy) {
        return ContractInteraction {
            kind: "none".to_string(),
            status,
            contract_address: String::new(),
            contract_name: None,
            code_hash: None,
            method: None,
            method_name: None,
            selector: None,
            arguments: Vec::new(),
            runtime_bytes: 0,
            bytecode_bytes: 0,
            raw_payload,
            raw_payload_bytes: payload_len,
            raw_payload_truncated: truncated,
            decode_status: DecodeStatus::NotAContract,
            reason: None,
        };
    }

    let address = contract_binding(tx);
    let address_bech =
        crate::crypto::encode_address_bech32m(&address, "aur").unwrap_or_else(|_| address.to_hex());
    let metadata = lookup_metadata(ctx, address);
    let contract_name = metadata.as_ref().map(|m| m.name.clone());
    let code_hash = {
        let accounts = ctx.accounts.lock().unwrap_or_else(|e| e.into_inner());
        accounts
            .get(&address)
            .and_then(|a| a.code_hash)
            .map(|h| h.to_hex())
    };

    // ---- Deploy: tampilkan hasil deploy, tanpa selector/argumen ----
    if tx.tx_type == TxType::ContractDeploy {
        let code = crate::crypto::blake3_hash(&tx.payload);
        let verified = crate::vm::verifier::BytecodeVerifier::verify(&tx.payload).is_ok();
        return ContractInteraction {
            kind: "deploy".to_string(),
            status,
            contract_address: address_bech,
            contract_name,
            code_hash: Some(code.to_hex()),
            method: None,
            method_name: None,
            selector: None,
            arguments: Vec::new(),
            runtime_bytes: 0,
            bytecode_bytes: payload_len,
            raw_payload,
            raw_payload_bytes: payload_len,
            raw_payload_truncated: truncated,
            decode_status: if verified {
                DecodeStatus::Decoded
            } else {
                DecodeStatus::UnknownMethod
            },
            reason: if verified {
                None
            } else {
                Some("Bytecode konstruktor gagal verifikasi statis (AUR-VM-005)".to_string())
            },
        };
    }

    // ---- Call: cari framing yang cocok dengan salah satu metode metadata ----
    if let Some(meta) = &metadata {
        for method in meta.methods.iter().take(MAX_METHOD_PROBE) {
            if let Ok(Some(frame)) = parse_call_frame(&tx.payload, method.inputs.len()) {
                if frame.selector == method.selector {
                    let arguments = build_arguments(&frame, method);
                    return ContractInteraction {
                        kind: "call".to_string(),
                        status,
                        contract_address: address_bech,
                        contract_name,
                        code_hash,
                        method: Some(method.signature.clone()),
                        method_name: Some(method.name.clone()),
                        selector: Some(format!("0x{}", hex::encode(frame.selector))),
                        arguments,
                        runtime_bytes: frame.runtime_bytes,
                        bytecode_bytes: 0,
                        raw_payload,
                        raw_payload_bytes: payload_len,
                        raw_payload_truncated: truncated,
                        decode_status: DecodeStatus::Decoded,
                        reason: None,
                    };
                }
            }
        }

        // Metadata ada, tetapi selector tidak cocok dengan frame mana pun.
        let (selector, runtime_bytes, arity) = heuristic_frame(&tx.payload);
        return ContractInteraction {
            kind: "call".to_string(),
            status,
            contract_address: address_bech,
            contract_name,
            code_hash,
            method: None,
            method_name: None,
            selector: selector.map(|s| format!("0x{}", hex::encode(s))),
            arguments: Vec::new(),
            runtime_bytes,
            bytecode_bytes: 0,
            raw_payload,
            raw_payload_bytes: payload_len,
            raw_payload_truncated: truncated,
            decode_status: DecodeStatus::UnknownMethod,
            reason: Some(format!(
                "Method tidak dikenal pada metadata (perkiraan arity={arity})"
            )),
        };
    }

    // Tanpa metadata: tampilkan sebagai Unknown Method, bukan hasil tebakan.
    let (selector, runtime_bytes, _) = heuristic_frame(&tx.payload);
    ContractInteraction {
        kind: "call".to_string(),
        status,
        contract_address: address_bech,
        contract_name: None,
        code_hash,
        method: None,
        method_name: None,
        selector: selector.map(|s| format!("0x{}", hex::encode(s))),
        arguments: Vec::new(),
        runtime_bytes,
        bytecode_bytes: 0,
        raw_payload,
        raw_payload_bytes: payload_len,
        raw_payload_truncated: truncated,
        decode_status: DecodeStatus::MetadataMissing,
        reason: Some(
            "Metadata kontrak belum terdaftar pada registry off-chain simpul ini; \
             ditampilkan sebagai Unknown Method."
                .to_string(),
        ),
    }
}
