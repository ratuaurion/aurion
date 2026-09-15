//! Enkripsi dan Dekripsi File Keystore Standar Aurion.
//! Mematuhi Dokumen 01 (01-WALLET-RULES.md Bagian 1.2).

use crate::crypto::{blake3_derive_key, blake3_hash};
use ed25519_dalek::SigningKey;
use zeroize::Zeroize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeystoreError {
    InvalidPassword,
    InvalidJsonFormat(String),
    MissingField(String),
    DecryptionFailed,
}

impl std::fmt::Display for KeystoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPassword => write!(f, "Kata sandi keystore tidak cocok (Verifikasi MAC gagal)"),
            Self::InvalidJsonFormat(e) => write!(f, "Format JSON keystore rusak: {e}"),
            Self::MissingField(field) => write!(f, "Field wajib '{field}' tidak ditemukan pada file keystore"),
            Self::DecryptionFailed => write!(f, "Dekripsi keystore gagal"),
        }
    }
}

impl std::error::Error for KeystoreError {}

/// Objek Kriptografi Keystore.
#[derive(Debug, Clone)]
pub struct KeystoreCrypto {
    pub cipher: String,
    pub ciphertext: String,
    pub salt: String,
    pub nonce: String,
    pub mac: String,
}

/// Struktur File Keystore Aurion.
#[derive(Debug, Clone)]
pub struct Keystore {
    pub version: u32,
    pub address: String,
    pub crypto: KeystoreCrypto,
}

impl Keystore {
    /// Mengenkripsi SigningKey dengan kata sandi dan menghasilkan objek Keystore.
    pub fn encrypt(signing_key: &SigningKey, password: &str, bech32m_address: &str) -> Self {
        let mut raw_secret = signing_key.to_bytes();
        let salt = generate_pseudo_random_32(password.as_bytes());
        let nonce = generate_pseudo_random_12(&salt);

        // KDF: Blake3 Key Derivation dengan Salt
        let mut kdf_input = Vec::with_capacity(password.len() + salt.len());
        kdf_input.extend_from_slice(password.as_bytes());
        kdf_input.extend_from_slice(&salt);
        let derived_key = blake3_derive_key("AURION-KEYSTORE-KDF-V1", &kdf_input);

        // Derivasi Stream Cipher Key dan MAC Key
        let mut cipher_input = Vec::with_capacity(64 + nonce.len());
        cipher_input.extend_from_slice(derived_key.as_bytes());
        cipher_input.extend_from_slice(&nonce);
        let cipher_stream = blake3_derive_key("AURION-KEYSTORE-CIPHER-V1", &cipher_input);

        // Enkripsi XOR Stream
        let mut ciphertext = [0u8; 32];
        for i in 0..32 {
            ciphertext[i] = raw_secret[i] ^ cipher_stream.as_bytes()[i];
        }

        // Hapus kunci rahasia dari RAM seketika (Zeroize Mandate)
        raw_secret.zeroize();

        // Hitung MAC terotentikasi
        let mut mac_input = Vec::with_capacity(32 + 32);
        mac_input.extend_from_slice(derived_key.as_bytes());
        mac_input.extend_from_slice(&ciphertext);
        let mac = blake3_derive_key("AURION-KEYSTORE-MAC-V1", &mac_input);

        Self {
            version: 1,
            address: bech32m_address.to_string(),
            crypto: KeystoreCrypto {
                cipher: "blake3-stream-v1".to_string(),
                ciphertext: hex::encode(ciphertext),
                salt: hex::encode(salt),
                nonce: hex::encode(nonce),
                mac: hex::encode(mac.as_bytes()),
            },
        }
    }

    /// Mendekripsi Keystore menggunakan kata sandi untuk memperoleh kembali SigningKey.
    pub fn decrypt(&self, password: &str) -> Result<SigningKey, KeystoreError> {
        let salt = hex::decode(&self.crypto.salt)
            .map_err(|e| KeystoreError::InvalidJsonFormat(e.to_string()))?;
        let nonce = hex::decode(&self.crypto.nonce)
            .map_err(|e| KeystoreError::InvalidJsonFormat(e.to_string()))?;
        let ciphertext = hex::decode(&self.crypto.ciphertext)
            .map_err(|e| KeystoreError::InvalidJsonFormat(e.to_string()))?;
        let expected_mac = hex::decode(&self.crypto.mac)
            .map_err(|e| KeystoreError::InvalidJsonFormat(e.to_string()))?;

        if ciphertext.len() != 32 || salt.len() != 32 || nonce.len() != 12 {
            return Err(KeystoreError::DecryptionFailed);
        }

        // KDF
        let mut kdf_input = Vec::with_capacity(password.len() + salt.len());
        kdf_input.extend_from_slice(password.as_bytes());
        kdf_input.extend_from_slice(&salt);
        let derived_key = blake3_derive_key("AURION-KEYSTORE-KDF-V1", &kdf_input);

        // Verifikasi MAC sebelum dekripsi
        let mut mac_input = Vec::with_capacity(32 + 32);
        mac_input.extend_from_slice(derived_key.as_bytes());
        mac_input.extend_from_slice(&ciphertext);
        let calculated_mac = blake3_derive_key("AURION-KEYSTORE-MAC-V1", &mac_input);

        if calculated_mac.as_bytes() != expected_mac.as_slice() {
            return Err(KeystoreError::InvalidPassword);
        }

        // Dekripsi
        let mut cipher_input = Vec::with_capacity(64 + nonce.len());
        cipher_input.extend_from_slice(derived_key.as_bytes());
        cipher_input.extend_from_slice(&nonce);
        let cipher_stream = blake3_derive_key("AURION-KEYSTORE-CIPHER-V1", &cipher_input);

        let mut decrypted_secret = [0u8; 32];
        for i in 0..32 {
            decrypted_secret[i] = ciphertext[i] ^ cipher_stream.as_bytes()[i];
        }

        let signing_key = SigningKey::from_bytes(&decrypted_secret);
        decrypted_secret.zeroize();

        Ok(signing_key)
    }

    /// Serialisasi ke format JSON string standar.
    pub fn to_json_string(&self) -> String {
        format!(
            r#"{{"version":{},"address":"{}","crypto":{{"cipher":"{}","ciphertext":"{}","salt":"{}","nonce":"{}","mac":"{}"}}}}"#,
            self.version,
            self.address,
            self.crypto.cipher,
            self.crypto.ciphertext,
            self.crypto.salt,
            self.crypto.nonce,
            self.crypto.mac
        )
    }

    /// Deserialisasi dari JSON string.
    pub fn from_json_str(raw: &str) -> Result<Self, KeystoreError> {
        let trimmed = raw.trim();
        let address = extract_field(trimmed, "address")
            .ok_or_else(|| KeystoreError::MissingField("address".to_string()))?;
        let cipher = extract_field(trimmed, "cipher")
            .unwrap_or_else(|| "blake3-stream-v1".to_string());
        let ciphertext = extract_field(trimmed, "ciphertext")
            .ok_or_else(|| KeystoreError::MissingField("ciphertext".to_string()))?;
        let salt = extract_field(trimmed, "salt")
            .ok_or_else(|| KeystoreError::MissingField("salt".to_string()))?;
        let nonce = extract_field(trimmed, "nonce")
            .ok_or_else(|| KeystoreError::MissingField("nonce".to_string()))?;
        let mac = extract_field(trimmed, "mac")
            .ok_or_else(|| KeystoreError::MissingField("mac".to_string()))?;

        Ok(Self {
            version: 1,
            address,
            crypto: KeystoreCrypto {
                cipher,
                ciphertext,
                salt,
                nonce,
                mac,
            },
        })
    }
}

fn extract_field(json: &str, field_name: &str) -> Option<String> {
    let pattern = format!("\"{field_name}\"");
    if let Some(pos) = json.find(&pattern) {
        let after_field = &json[pos + pattern.len()..];
        if let Some(colon_pos) = after_field.find(':') {
            let after_colon = after_field[colon_pos + 1..].trim_start();
            if let Some(stripped) = after_colon.strip_prefix('"') {
                if let Some(end_quote) = stripped.find('"') {
                    return Some(stripped[..end_quote].to_string());
                }
            }
        }
    }
    None
}

fn generate_pseudo_random_32(seed_entropy: &[u8]) -> [u8; 32] {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut input = Vec::with_capacity(seed_entropy.len() + 16);
    input.extend_from_slice(seed_entropy);
    input.extend_from_slice(&now.to_be_bytes());
    *blake3_hash(&input).as_bytes()
}

fn generate_pseudo_random_12(salt: &[u8; 32]) -> [u8; 12] {
    let hash = blake3_hash(salt);
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&hash.as_bytes()[0..12]);
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keystore_encryption_decryption_cycle() {
        let signing_key = SigningKey::from_bytes(&[0x77u8; 32]);
        let address = "aur1testaddresscanonical777777777777777777777777777777777777";
        let password = "SuperSecurePassword123!";

        let keystore = Keystore::encrypt(&signing_key, password, address);
        let json_str = keystore.to_json_string();

        let parsed = Keystore::from_json_str(&json_str).expect("Parsing harus berhasil");
        assert_eq!(parsed.address, address);

        // Dekripsi dengan password benar
        let recovered = parsed.decrypt(password).expect("Dekripsi harus berhasil");
        assert_eq!(recovered.to_bytes(), signing_key.to_bytes());

        // Dekripsi dengan password salah harus gagal (InvalidPassword)
        let err = parsed.decrypt("WrongPassword").unwrap_err();
        assert_eq!(err, KeystoreError::InvalidPassword);
    }
}
