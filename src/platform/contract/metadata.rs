//! Metadata & ABI minimal untuk kontrak AVM (refleksi kontrak Contract SDK).
//!
//! Aurion tidak memiliki sistem ABI on-chain; modul ini menetapkan struktur
//! metadata **minimal dan aman** yang mengikat kontrak ke `code_hash` on-chain
//! (AUR-VM-006) dan memakai selector Blake3 4-byte kanonikal dari
//! `scaling::abi::compute_method_selector` (AUR-ARCH-005, satu selector kanonikal).

use serde::{Deserialize, Serialize};

use crate::core::{Address, Hash256, Quantum};
use crate::crypto::{blake3_hash, decode_address_bech32m, encode_address_bech32m};
use crate::transaction::types::MAX_TRANSACTION_PAYLOAD_BYTES;

use super::error::ContractError;

/// Schema identitas metadata Contract SDK.
pub const METADATA_SCHEMA: &str = "aurion/contract-metadata-v1";

/// Batas argumen per metode: instruksi `DUP4`/`SWAP4` hanya menjangkau 4 slot.
pub const MAX_ABI_INPUTS: usize = 4;

/// 1 AUR = 10^9 Quanta (konstanta kanonikal `QUANTA_PER_AUR`).
const QUANTA_PER_AUR: u128 = 1_000_000_000;

/// Format integer murni Quanta -> tampilan AUR (Zero-Float, AUR-ARCH-012).
#[must_use]
pub fn format_quanta(quanta: u128) -> String {
    let whole = quanta / QUANTA_PER_AUR;
    let frac = quanta % QUANTA_PER_AUR;
    if frac == 0 {
        format!("{whole} AUR ({quanta} Q)")
    } else {
        format!("{whole}.{frac:09} AUR ({quanta} Q)")
    }
}

/// Tipe data argumen ABI kontrak (murni integer/bytes, tanpa float).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbiType {
    /// Alamat Bech32m 32-byte.
    Address,
    /// Nilai moneter Quanta (`u128`).
    Quantum,
    /// Bilangan bulat unsigned 64-bit.
    U64,
    /// Bilangan bulat unsigned 32-bit.
    U32,
    /// Boolean (0/1 pada word 256-bit).
    Bool,
    /// Hash kriptografi 32-byte (Blake3).
    Hash256,
}

impl AbiType {
    /// Label tipe untuk prompt clear signing.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Address => "Address",
            Self::Quantum => "Quantum",
            Self::U64 => "U64",
            Self::U32 => "U32",
            Self::Bool => "Bool",
            Self::Hash256 => "Hash256",
        }
    }

    /// Decode word 32-byte kanonikal (big-endian, rata kanan) kembali ke nilai.
    pub fn decode_word(&self, word: &[u8; 32]) -> Result<AbiValue, ContractError> {
        match self {
            Self::Address => Ok(AbiValue::Address(Address(*word))),
            Self::Hash256 => Ok(AbiValue::Hash256(Hash256(*word))),
            Self::Quantum => {
                let mut buf = [0u8; 16];
                buf.copy_from_slice(&word[16..32]);
                Ok(AbiValue::Quantum(Quantum::new(u128::from_be_bytes(buf))))
            }
            Self::U64 => {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&word[24..32]);
                Ok(AbiValue::U64(u64::from_be_bytes(buf)))
            }
            Self::U32 => {
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&word[28..32]);
                Ok(AbiValue::U32(u32::from_be_bytes(buf)))
            }
            Self::Bool => Ok(AbiValue::Bool(word[31] != 0)),
        }
    }
}

/// Nilai argumen ter-tipe untuk pemanggilan metode kontrak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiValue {
    /// Alamat 32-byte.
    Address(Address),
    /// Nilai Quanta.
    Quantum(Quantum),
    /// Bilangan bulat 64-bit.
    U64(u64),
    /// Bilangan bulat 32-bit.
    U32(u32),
    /// Boolean.
    Bool(bool),
    /// Hash 32-byte.
    Hash256(Hash256),
}

impl AbiValue {
    /// Tipe ABI dari nilai ini.
    #[must_use]
    pub fn abi_type(&self) -> AbiType {
        match self {
            Self::Address(_) => AbiType::Address,
            Self::Quantum(_) => AbiType::Quantum,
            Self::U64(_) => AbiType::U64,
            Self::U32(_) => AbiType::U32,
            Self::Bool(_) => AbiType::Bool,
            Self::Hash256(_) => AbiType::Hash256,
        }
    }

    /// Enkripsi ke word 32-byte kanonikal (big-endian, rata kanan).
    #[must_use]
    pub fn encode_word(&self) -> [u8; 32] {
        let mut word = [0u8; 32];
        match self {
            Self::Address(addr) => word.copy_from_slice(addr.as_bytes()),
            Self::Hash256(h) => word.copy_from_slice(h.as_bytes()),
            Self::Quantum(q) => word[16..32].copy_from_slice(&q.as_u128().to_be_bytes()),
            Self::U64(v) => word[24..32].copy_from_slice(&v.to_be_bytes()),
            Self::U32(v) => word[28..32].copy_from_slice(&v.to_be_bytes()),
            Self::Bool(b) => word[31] = u8::from(*b),
        }
        word
    }

    /// Representasi manusiawi untuk prompt clear signing.
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Address(addr) => {
                let bech = encode_address_bech32m(addr, "aur").unwrap_or_else(|_| addr.to_hex());
                format!("{bech} ({})", addr.to_hex())
            }
            Self::Quantum(q) => format_quanta(q.as_u128()),
            Self::U64(v) => format!("{v} (U64)"),
            Self::U32(v) => format!("{v} (U32)"),
            Self::Bool(b) => format!("{b} (Bool)"),
            Self::Hash256(h) => format!("0x{} (Hash256)", h.to_hex()),
        }
    }
}

/// Deklarasi satu parameter metode ABI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbiParam {
    /// Nama parameter (untuk prompt manusiawi).
    pub name: String,
    /// Tipe parameter.
    pub ty: AbiType,
}

fn default_true() -> bool {
    true
}

/// Deklarasi satu metode kontrak: nama, signature kanonikal, selector turunan, dan parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodAbi {
    /// Nama metode (mis. `transfer`).
    pub name: String,
    /// Signature kanonikal pemilih selector (mis. `transfer(u64)`).
    pub signature: String,
    /// Parameter masukan.
    pub inputs: Vec<AbiParam>,
    /// Parameter keluaran (opsional, untuk dokumentasi & dekripsi hasil).
    #[serde(default)]
    pub outputs: Vec<AbiParam>,
    /// Apakah metode menerima nilai Quanta (`value > 0`).
    #[serde(default = "default_true")]
    pub payable: bool,
    /// Selector 4-byte kanonikal Blake3 — **selalu diturunkan dari `signature`**,
    /// tidak pernah dipercaya dari JSON (integritas anti-tamu).
    #[serde(skip)]
    pub selector: [u8; 4],
}

impl MethodAbi {
    /// Membuat deklarasi metode dengan selector diturunkan dari `signature`.
    ///
    /// # Errors
    /// - Nama/signature kosong.
    /// - Jumlah input melebihi jendela stack AVM ([`MAX_ABI_INPUTS`]).
    pub fn new(
        name: impl Into<String>,
        signature: impl Into<String>,
        inputs: Vec<AbiParam>,
        outputs: Vec<AbiParam>,
        payable: bool,
    ) -> Result<Self, ContractError> {
        let name = name.into();
        let signature = signature.into();
        if name.trim().is_empty() {
            return Err(ContractError::Metadata("Nama metode kosong".to_string()));
        }
        if signature.trim().is_empty() {
            return Err(ContractError::Metadata(
                "Signature metode kosong".to_string(),
            ));
        }
        if inputs.len() > MAX_ABI_INPUTS {
            return Err(ContractError::TooManyArguments {
                got: inputs.len(),
                max: MAX_ABI_INPUTS,
            });
        }
        let selector = selector_for(&signature);
        Ok(Self {
            name,
            signature,
            inputs,
            outputs,
            payable,
            selector,
        })
    }

    /// Turunkan ulang selector dari signature (panggil setelah deserialize).
    pub fn derive_selector(&mut self) {
        self.selector = selector_for(&self.signature);
    }

    /// Validasi jumlah & tipe argumen terhadap deklarasi metode.
    pub fn check_args(&self, args: &[AbiValue]) -> Result<(), ContractError> {
        if args.len() > MAX_ABI_INPUTS {
            return Err(ContractError::TooManyArguments {
                got: args.len(),
                max: MAX_ABI_INPUTS,
            });
        }
        if args.len() != self.inputs.len() {
            return Err(ContractError::Abi(format!(
                "Metode '{}' membutuhkan {} argumen, diberikan {}",
                self.signature,
                self.inputs.len(),
                args.len()
            )));
        }
        for (idx, (param, value)) in self.inputs.iter().zip(args.iter()).enumerate() {
            if param.ty != value.abi_type() {
                return Err(ContractError::Abi(format!(
                    "Argumen ke-{} '{}' bertipe {}, diharapkan {}",
                    idx,
                    param.name,
                    value.abi_type().label(),
                    param.ty.label()
                )));
            }
        }
        Ok(())
    }
}

/// Selector 4-byte kanonikal Aurion: 4 byte pertama Blake3 atas signature.
#[must_use]
pub fn selector_for(signature: &str) -> [u8; 4] {
    crate::l2::abi::compute_method_selector(signature)
}

/// Metadata kontrak minimal: identitas + binding `code_hash` + runtime + ABI metode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractMetadata {
    /// Skema metadata (`aurion/contract-metadata-v1`).
    pub schema: String,
    /// Nama kontrak untuk ditampilkan (mis. `AurionCounter`).
    pub name: String,
    /// Chain ID tempat kontrak berlaku.
    pub chain_id: u32,
    /// Alamat kontrak (Bech32m HRP `aur`).
    pub address: String,
    /// `code_hash` on-chain kontrak (hex 32-byte) hasil `blake3(konstruktor)`.
    pub code_hash: String,
    /// Bytecode runtime panggilan (hex) — dieksekusi STF untuk `ContractCall`.
    #[serde(default)]
    pub runtime: String,
    /// Hash integritas runtime opsional; bila terisi wajib sama dengan `blake3(runtime)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_hash: Option<String>,
    /// Daftar metode ABI.
    #[serde(default)]
    pub methods: Vec<MethodAbi>,
}

impl ContractMetadata {
    /// Membuat metadata baru dengan selector seluruh metode diturunkan ulang.
    ///
    /// # Errors
    /// - Alamat bukan Bech32m valid.
    /// - `runtime` kosong atau melebihi batas payload 24 KB.
    pub fn new(
        name: impl Into<String>,
        chain_id: u32,
        address_bech32m: &str,
        code_hash: &Hash256,
        runtime: &[u8],
        methods: Vec<MethodAbi>,
    ) -> Result<Self, ContractError> {
        decode_address_bech32m(address_bech32m, "aur").map_err(|e| {
            ContractError::InvalidAddress(format!("Alamat kontrak metadata tidak valid: {e}"))
        })?;
        if runtime.is_empty() {
            return Err(ContractError::Metadata(
                "Runtime bytecode kontrak kosong".to_string(),
            ));
        }
        if runtime.len() > MAX_TRANSACTION_PAYLOAD_BYTES {
            return Err(ContractError::Metadata(format!(
                "Runtime {} byte melebihi batas payload {} byte",
                runtime.len(),
                MAX_TRANSACTION_PAYLOAD_BYTES
            )));
        }
        let mut meta = Self {
            schema: METADATA_SCHEMA.to_string(),
            name: name.into(),
            chain_id,
            address: address_bech32m.to_string(),
            code_hash: code_hash.to_hex(),
            runtime: hex::encode(runtime),
            runtime_hash: Some(blake3_hash(runtime).to_hex()),
            methods,
        };
        meta.derive_selectors();
        Ok(meta)
    }

    /// Turunkan ulang seluruh selector dari signature (kerentanan JSON ditutup).
    pub fn derive_selectors(&mut self) {
        for method in &mut self.methods {
            method.derive_selector();
        }
    }

    /// Deserialisasi dari JSON + turunkan ulang seluruh selector.
    ///
    /// # Errors
    /// JSON tidak valid atau skema tidak didukung.
    pub fn from_json(raw: &str) -> Result<Self, ContractError> {
        let mut meta: ContractMetadata = serde_json::from_str(raw.trim())
            .map_err(|e| ContractError::Metadata(format!("JSON metadata tidak valid: {e}")))?;
        if meta.schema != METADATA_SCHEMA {
            return Err(ContractError::Metadata(format!(
                "Skema metadata '{}' tidak didukung (diharapkan '{METADATA_SCHEMA}')",
                meta.schema
            )));
        }
        meta.derive_selectors();
        Ok(meta)
    }

    /// Serialisasi ke JSON.
    ///
    /// # Errors
    /// Gagal serialisasi (tidak terjadi pada tipe ini).
    pub fn to_json(&self) -> Result<String, ContractError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| ContractError::Metadata(format!("Gagal serialisasi metadata: {e}")))
    }

    /// Cari metode berdasarkan nama.
    #[must_use]
    pub fn find_method(&self, name: &str) -> Option<&MethodAbi> {
        self.methods.iter().find(|m| m.name == name)
    }

    /// Decode `code_hash` metadata.
    ///
    /// # Errors
    /// Hex tidak valid / bukan 32 byte.
    pub fn code_hash_bytes(&self) -> Result<Hash256, ContractError> {
        parse_hash32(&self.code_hash, "code_hash")
    }

    /// Decode bytecode runtime (dengan verifikasi integritas `runtime_hash`).
    ///
    /// # Errors
    /// Hex tidak valid / kosong / hash tidak cocok.
    pub fn runtime_bytes(&self) -> Result<Vec<u8>, ContractError> {
        if self.runtime.is_empty() {
            return Err(ContractError::Metadata(
                "Metadata tidak memiliki runtime bytecode (panggilan tidak mungkin)".to_string(),
            ));
        }
        let bytes = hex::decode(&self.runtime)
            .map_err(|e| ContractError::Metadata(format!("Runtime hex tidak valid: {e}")))?;
        if bytes.is_empty() {
            return Err(ContractError::Metadata(
                "Runtime bytecode kosong".to_string(),
            ));
        }
        if let Some(expected) = &self.runtime_hash {
            let actual = blake3_hash(&bytes).to_hex();
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(ContractError::CodeBinding(format!(
                    "runtime_hash metadata ({expected}) tidak cocok dengan blake3(runtime) ({actual})"
                )));
            }
        }
        Ok(bytes)
    }

    /// Binding metadata ke `code_hash` akun kontrak **on-chain** (pertahanan utama
    /// melawan metadata palsu: metadata yang tidak cocok dengan kontrak ditolak
    /// sebelum signing).
    ///
    /// # Errors
    /// - Akun bukan kontrak / `code_hash` hilang.
    /// - `code_hash` berbeda dengan metadata.
    pub fn verify_binding(
        &self,
        on_chain_code_hash: Option<&Hash256>,
    ) -> Result<(), ContractError> {
        let expected = self.code_hash_bytes()?;
        match on_chain_code_hash {
            None => Err(ContractError::CodeBinding(
                "Akun tujuan bukan kontrak (code_hash on-chain tidak ada)".to_string(),
            )),
            Some(actual) if *actual == expected => Ok(()),
            Some(actual) => Err(ContractError::CodeBinding(format!(
                "code_hash on-chain ({}) tidak cocok dengan metadata ({})",
                actual.to_hex(),
                expected.to_hex()
            ))),
        }
    }
}

/// Decode hex 32-byte menjadi [`Hash256`].
///
/// # Errors
/// Hex tidak valid atau panjang != 32 byte.
pub fn parse_hash32(hex_str: &str, field: &str) -> Result<Hash256, ContractError> {
    let trimmed = hex_str.trim().trim_start_matches("0x");
    let bytes = hex::decode(trimmed)
        .map_err(|e| ContractError::Metadata(format!("{field} hex tidak valid: {e}")))?;
    if bytes.len() != 32 {
        return Err(ContractError::Metadata(format!(
            "{field} harus 32 byte, diterima {} byte",
            bytes.len()
        )));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(Hash256(arr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_quanta_zero_float() {
        assert_eq!(format_quanta(1_000_000_000), "1 AUR (1000000000 Q)");
        assert_eq!(format_quanta(10_000), "0.000010000 AUR (10000 Q)");
    }

    #[test]
    fn test_abi_word_roundtrip() {
        let values = vec![
            AbiValue::U64(0xDEAD_BEEF),
            AbiValue::U32(7),
            AbiValue::Quantum(Quantum::new(42)),
            AbiValue::Bool(true),
            AbiValue::Address(Address::from_bytes([0xAA; 32])),
            AbiValue::Hash256(Hash256::from_bytes([0xBB; 32])),
        ];
        for value in values {
            let word = value.encode_word();
            let decoded = value.abi_type().decode_word(&word).expect("decode");
            assert_eq!(decoded, value);
        }
    }

    #[test]
    fn test_selector_is_derived_not_trusted_from_json() {
        let method = MethodAbi::new(
            "transfer",
            "transfer(u64)",
            vec![AbiParam {
                name: "amount".to_string(),
                ty: AbiType::U64,
            }],
            vec![],
            false,
        )
        .expect("method");
        assert_eq!(method.selector, selector_for("transfer(u64)"));

        // Selector tidak pernah diserialisasi: JSON lama yang memalsukan selector
        // tetap diturunkan ulang dari signature saat dimuat.
        let bech32m =
            encode_address_bech32m(&Address::from_bytes([0x33; 32]), "aur").expect("bech32m");
        let meta = ContractMetadata::new(
            "Demo",
            1001,
            &bech32m,
            &blake3_hash(b"ctor"),
            &[0x00],
            vec![method],
        )
        .expect("metadata");
        let json = meta.to_json().expect("json");
        assert!(!json.contains("selector"));
        let loaded = ContractMetadata::from_json(&json).expect("load");
        assert_eq!(loaded.methods[0].selector, selector_for("transfer(u64)"));
    }
}
