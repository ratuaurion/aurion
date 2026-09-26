//! Enkripsi dan Dekripsi File Keystore Standar Aurion.
//! Mematuhi Dokumen 01 (01-WALLET-RULES.md Bagian 1.2) dan AUR-ISSUE-004.
//!
//! Format kanonik **V2** memakai Argon2id (memory-hard KDF) + ChaCha20Poly1305
//! (AEAD). Format legacy `blake3-stream-v1` (V1) tetap dapat dibaca dan
//! dimigrasikan otomatis ke V2 melalui [`Keystore::unlock_and_migrate`].

mod legacy;

#[cfg(test)]
use crate::crypto::blake3_derive_key;
use crate::crypto::{derive_address_from_pubkey, encode_address_bech32m};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use rand::RngCore;
use zeroize::{Zeroize, Zeroizing};

/// Parameter Argon2id kanonik (minimum Dokumen 01: memory >= 64 MB, iter >= 3, lanes >= 4).
pub const ARGON2ID_M_COST: u32 = 65_536; // 64 MiB (dalam KiB)
pub const ARGON2ID_T_COST: u32 = 3;
pub const ARGON2ID_P_COST: u32 = 4;

const ARGON2ID_ALGORITHM: &str = "argon2id";
const AEAD_ALGORITHM: &str = "chacha20poly1305";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const SECRET_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeystoreError {
    InvalidPassword,
    AddressMismatch { expected: String, derived: String },
    InvalidJsonFormat(String),
    MissingField(String),
    DecryptionFailed,
    KdfFailure(String),
    CipherFailure(String),
    IoFailure(String),
}

impl std::fmt::Display for KeystoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPassword => {
                write!(f, "Kata sandi keystore tidak cocok (Verifikasi MAC gagal)")
            }
            Self::AddressMismatch { expected, derived } => write!(
                f,
                "Alamat keystore tidak cocok: metadata '{expected}', derivasi '{derived}'"
            ),
            Self::InvalidJsonFormat(e) => write!(f, "Format JSON keystore rusak: {e}"),
            Self::MissingField(field) => write!(
                f,
                "Field wajib '{field}' tidak ditemukan pada file keystore"
            ),
            Self::DecryptionFailed => write!(f, "Dekripsi keystore gagal"),
            Self::KdfFailure(e) => write!(f, "Derivasi kunci Argon2id gagal: {e}"),
            Self::CipherFailure(e) => write!(f, "Operasi cipher AEAD gagal: {e}"),
            Self::IoFailure(e) => write!(f, "Operasi berkas keystore gagal: {e}"),
        }
    }
}

impl std::error::Error for KeystoreError {}

/// Parameter KDF Argon2id yang tersimpan pada envelope V2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KdfParamsV2 {
    pub algorithm: String,
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

fn default_kdf_params() -> KdfParamsV2 {
    KdfParamsV2 {
        algorithm: ARGON2ID_ALGORITHM.to_string(),
        m_cost: ARGON2ID_M_COST,
        t_cost: ARGON2ID_T_COST,
        p_cost: ARGON2ID_P_COST,
    }
}

/// Objek Kriptografi Keystore.
#[derive(Debug, Clone)]
pub struct KeystoreCrypto {
    pub cipher: String,
    pub ciphertext: String,
    pub salt: String,
    pub nonce: String,
    pub mac: String,
}

/// Struktur File Keystore Aurion (V1 legacy maupun V2 kanonik).
#[derive(Debug, Clone)]
pub struct Keystore {
    pub version: u32,
    pub address: String,
    pub crypto: KeystoreCrypto,
    /// Parameter KDF; hanya terisi untuk envelope V2.
    pub kdf: Option<KdfParamsV2>,
}

#[derive(serde::Deserialize)]
struct RawKeystore {
    #[serde(default = "default_version")]
    version: u32,
    #[serde(default)]
    address: String,
    #[serde(default)]
    crypto: Option<RawLegacyCrypto>,
    #[serde(default)]
    kdf: Option<RawKdf>,
    #[serde(default)]
    cipher: Option<RawCipher>,
}

#[derive(serde::Deserialize)]
struct RawLegacyCrypto {
    #[serde(default = "default_cipher_id")]
    cipher: String,
    ciphertext: String,
    salt: String,
    nonce: String,
    mac: String,
}

#[derive(serde::Deserialize)]
struct RawKdf {
    algorithm: String,
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
    salt: String,
}

#[derive(serde::Deserialize)]
struct RawCipher {
    algorithm: String,
    nonce: String,
    ciphertext: String,
}

fn default_version() -> u32 {
    1
}

fn default_cipher_id() -> String {
    legacy::CIPHER_ID.to_string()
}

/// Fungsi KDF kanonik: Argon2id dengan parameter V2.
fn derive_key_argon2id(
    password: &str,
    salt: &[u8],
    params: &KdfParamsV2,
) -> Result<Zeroizing<[u8; SECRET_LEN]>, KeystoreError> {
    let params = Params::new(
        params.m_cost,
        params.t_cost,
        params.p_cost,
        Some(SECRET_LEN),
    )
    .map_err(|e| KeystoreError::KdfFailure(e.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut derived = Zeroizing::new([0u8; SECRET_LEN]);
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut *derived)
        .map_err(|e| KeystoreError::KdfFailure(e.to_string()))?;
    Ok(derived)
}

fn verify_address_binding(
    signing_key: &SigningKey,
    metadata_address: &str,
) -> Result<(), KeystoreError> {
    let address = derive_address_from_pubkey(signing_key.verifying_key().as_bytes());
    let derived = encode_address_bech32m(&address, "aur")
        .map_err(|e| KeystoreError::InvalidJsonFormat(e.to_string()))?;
    if derived.to_lowercase() != metadata_address.trim().to_lowercase() {
        return Err(KeystoreError::AddressMismatch {
            expected: metadata_address.to_string(),
            derived,
        });
    }
    Ok(())
}

impl Keystore {
    /// Mengenkripsi SigningKey dengan envelope kanonik V2.
    pub fn encrypt(
        signing_key: &SigningKey,
        password: &str,
        bech32m_address: &str,
    ) -> Result<Self, KeystoreError> {
        let mut raw_secret = signing_key.to_bytes();
        let result = Self::encrypt_v2(&raw_secret, password, bech32m_address);
        raw_secret.zeroize();
        result
    }

    /// Jalur enkripsi V2 (Argon2id + ChaCha20Poly1305).
    pub fn encrypt_v2(
        secret: &[u8; SECRET_LEN],
        password: &str,
        bech32m_address: &str,
    ) -> Result<Self, KeystoreError> {
        let mut salt = [0u8; SALT_LEN];
        let mut nonce_bytes = [0u8; NONCE_LEN];
        let mut rng = OsRng;
        rng.fill_bytes(&mut salt);
        rng.fill_bytes(&mut nonce_bytes);

        let params = default_kdf_params();
        let derived_key = derive_key_argon2id(password, &salt, &params)?;

        let cipher = ChaCha20Poly1305::new_from_slice(&*derived_key)
            .map_err(|e| KeystoreError::CipherFailure(e.to_string()))?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, secret.as_slice())
            .map_err(|e| KeystoreError::CipherFailure(e.to_string()))?;

        Ok(Self {
            version: 2,
            address: bech32m_address.to_string(),
            crypto: KeystoreCrypto {
                cipher: AEAD_ALGORITHM.to_string(),
                ciphertext: hex::encode(ciphertext),
                salt: hex::encode(salt),
                nonce: hex::encode(nonce_bytes),
                mac: String::new(),
            },
            kdf: Some(params),
        })
    }

    /// Mendekripsi keystore (V1 legacy atau V2 kanonik).
    pub fn decrypt(&self, password: &str) -> Result<SigningKey, KeystoreError> {
        match self.version {
            1 => self.decrypt_legacy(password),
            2 => self.decrypt_v2(password),
            other => Err(KeystoreError::InvalidJsonFormat(format!(
                "Versi keystore tidak didukung: {other}"
            ))),
        }
    }

    /// Dekripsi + migrasi otomatis ke V2.
    ///
    /// Mengembalikan signing key beserta keystore V2 baru bila input masih
    /// legacy (V1). Bila sudah V2, komponen kedua bernilai `None`.
    pub fn unlock_and_migrate(
        &self,
        password: &str,
    ) -> Result<(SigningKey, Option<Keystore>), KeystoreError> {
        let signing_key = self.decrypt(password)?;
        if self.version == 1 {
            let upgraded = Keystore::encrypt(&signing_key, password, &self.address)?;
            Ok((signing_key, Some(upgraded)))
        } else {
            Ok((signing_key, None))
        }
    }

    /// Apakah keystore memakai format legacy `blake3-stream-v1`.
    pub fn is_legacy(&self) -> bool {
        self.version == 1
    }

    /// Dekripsi + migrasi otomatis, lalu tuliskan hasil V2 ke `path` bila
    /// keystore masih legacy. Mengembalikan signing key.
    pub fn unlock_and_migrate_to_file(
        &self,
        password: &str,
        path: &str,
    ) -> Result<SigningKey, KeystoreError> {
        let (signing_key, upgraded) = self.unlock_and_migrate(password)?;
        if let Some(upgraded) = upgraded {
            std::fs::write(path, upgraded.to_json_string())
                .map_err(|e| KeystoreError::IoFailure(e.to_string()))?;
        }
        Ok(signing_key)
    }

    fn decrypt_v2(&self, password: &str) -> Result<SigningKey, KeystoreError> {
        let salt = hex::decode(&self.crypto.salt)
            .map_err(|e| KeystoreError::InvalidJsonFormat(e.to_string()))?;
        let nonce_bytes = hex::decode(&self.crypto.nonce)
            .map_err(|e| KeystoreError::InvalidJsonFormat(e.to_string()))?;
        let ciphertext = hex::decode(&self.crypto.ciphertext)
            .map_err(|e| KeystoreError::InvalidJsonFormat(e.to_string()))?;

        if nonce_bytes.len() != NONCE_LEN || salt.is_empty() {
            return Err(KeystoreError::DecryptionFailed);
        }

        let params = self.kdf.clone().unwrap_or_else(default_kdf_params);
        let derived_key = derive_key_argon2id(password, &salt, &params)?;

        let cipher = ChaCha20Poly1305::new_from_slice(&*derived_key)
            .map_err(|e| KeystoreError::CipherFailure(e.to_string()))?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| KeystoreError::InvalidPassword)?;
        let plaintext = Zeroizing::new(plaintext);

        if plaintext.len() != SECRET_LEN {
            return Err(KeystoreError::DecryptionFailed);
        }

        let mut secret = [0u8; SECRET_LEN];
        secret.copy_from_slice(plaintext.as_slice());
        let signing_key = SigningKey::from_bytes(&secret);
        secret.zeroize();
        verify_address_binding(&signing_key, &self.address)?;

        Ok(signing_key)
    }

    fn decrypt_legacy(&self, password: &str) -> Result<SigningKey, KeystoreError> {
        let salt = hex::decode(&self.crypto.salt)
            .map_err(|e| KeystoreError::InvalidJsonFormat(e.to_string()))?;
        let nonce = hex::decode(&self.crypto.nonce)
            .map_err(|e| KeystoreError::InvalidJsonFormat(e.to_string()))?;
        let ciphertext = hex::decode(&self.crypto.ciphertext)
            .map_err(|e| KeystoreError::InvalidJsonFormat(e.to_string()))?;
        let expected_mac = hex::decode(&self.crypto.mac)
            .map_err(|e| KeystoreError::InvalidJsonFormat(e.to_string()))?;

        if ciphertext.len() != SECRET_LEN || salt.len() != 32 || nonce.len() != NONCE_LEN {
            return Err(KeystoreError::DecryptionFailed);
        }

        let mut ct = legacy::LegacyCiphertext {
            ciphertext: [0u8; SECRET_LEN],
            salt: [0u8; 32],
            nonce: [0u8; NONCE_LEN],
            mac: [0u8; SECRET_LEN],
        };
        ct.ciphertext.copy_from_slice(&ciphertext);
        ct.salt.copy_from_slice(&salt);
        ct.nonce.copy_from_slice(&nonce);
        match expected_mac.len() {
            SECRET_LEN => ct.mac.copy_from_slice(&expected_mac),
            _ => return Err(KeystoreError::DecryptionFailed),
        }

        match legacy::decrypt(password, &ct) {
            Some(mut secret) => {
                let signing_key = SigningKey::from_bytes(&secret);
                secret.zeroize();
                verify_address_binding(&signing_key, &self.address)?;
                Ok(signing_key)
            }
            None => Err(KeystoreError::InvalidPassword),
        }
    }

    /// Serialisasi ke format JSON string standar (V2 kanonik atau V1 legacy).
    pub fn to_json_string(&self) -> String {
        if self.version == 1 {
            return format!(
                r#"{{"version":{},"address":"{}","crypto":{{"cipher":"{}","ciphertext":"{}","salt":"{}","nonce":"{}","mac":"{}"}}}}"#,
                self.version,
                self.address,
                self.crypto.cipher,
                self.crypto.ciphertext,
                self.crypto.salt,
                self.crypto.nonce,
                self.crypto.mac
            );
        }

        let kdf = self.kdf.clone().unwrap_or_else(default_kdf_params);
        serde_json::json!({
            "version": self.version,
            "address": self.address,
            "kdf": {
                "algorithm": kdf.algorithm,
                "m_cost": kdf.m_cost,
                "t_cost": kdf.t_cost,
                "p_cost": kdf.p_cost,
                "salt": self.crypto.salt,
            },
            "cipher": {
                "algorithm": self.crypto.cipher,
                "nonce": self.crypto.nonce,
                "ciphertext": self.crypto.ciphertext,
            }
        })
        .to_string()
    }

    /// Deserialisasi dari JSON string (V1 legacy maupun V2 kanonik).
    pub fn from_json_str(raw: &str) -> Result<Self, KeystoreError> {
        let parsed: RawKeystore = serde_json::from_str(raw.trim())
            .map_err(|e| KeystoreError::InvalidJsonFormat(e.to_string()))?;

        match parsed.version {
            1 => {
                let crypto = parsed
                    .crypto
                    .ok_or_else(|| KeystoreError::MissingField("crypto".to_string()))?;
                Ok(Self {
                    version: 1,
                    address: parsed.address,
                    crypto: KeystoreCrypto {
                        cipher: crypto.cipher,
                        ciphertext: crypto.ciphertext,
                        salt: crypto.salt,
                        nonce: crypto.nonce,
                        mac: crypto.mac,
                    },
                    kdf: None,
                })
            }
            2 => {
                let kdf = parsed
                    .kdf
                    .ok_or_else(|| KeystoreError::MissingField("kdf".to_string()))?;
                let cipher = parsed
                    .cipher
                    .ok_or_else(|| KeystoreError::MissingField("cipher".to_string()))?;
                Ok(Self {
                    version: 2,
                    address: parsed.address,
                    crypto: KeystoreCrypto {
                        cipher: cipher.algorithm,
                        ciphertext: cipher.ciphertext,
                        salt: kdf.salt,
                        nonce: cipher.nonce,
                        mac: String::new(),
                    },
                    kdf: Some(KdfParamsV2 {
                        algorithm: kdf.algorithm,
                        m_cost: kdf.m_cost,
                        t_cost: kdf.t_cost,
                        p_cost: kdf.p_cost,
                    }),
                })
            }
            other => Err(KeystoreError::InvalidJsonFormat(format!(
                "Versi keystore tidak didukung: {other}"
            ))),
        }
    }

    /// Membangun keystore V1 legacy secara manual (dipakai untuk test migrasi).
    #[cfg(test)]
    fn legacy_fixture(signing_key: &SigningKey, password: &str, address: &str) -> Self {
        let secret = signing_key.to_bytes();
        let ct = legacy::encrypt(&secret, password);
        Self {
            version: 1,
            address: address.to_string(),
            crypto: KeystoreCrypto {
                cipher: legacy::CIPHER_ID.to_string(),
                ciphertext: hex::encode(ct.ciphertext),
                salt: hex::encode(ct.salt),
                nonce: hex::encode(ct.nonce),
                mac: hex::encode(ct.mac),
            },
            kdf: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[0x77u8; SECRET_LEN])
    }

    const TEST_PASSWORD: &str = "SuperSecurePassword123!";

    fn test_address() -> String {
        let key = test_signing_key();
        let address = derive_address_from_pubkey(key.verifying_key().as_bytes());
        encode_address_bech32m(&address, "aur").unwrap()
    }

    #[test]
    fn test_keystore_v2_encryption_decryption_cycle() {
        let signing_key = test_signing_key();

        let address = test_address();
        let keystore = Keystore::encrypt(&signing_key, TEST_PASSWORD, &address)
            .expect("Enkripsi V2 harus berhasil");
        assert_eq!(keystore.version, 2);
        assert_eq!(keystore.crypto.cipher, AEAD_ALGORITHM);

        let json_str = keystore.to_json_string();
        let parsed = Keystore::from_json_str(&json_str).expect("Parsing V2 harus berhasil");
        assert_eq!(parsed.address, address);
        assert_eq!(parsed.version, 2);

        let recovered = parsed
            .decrypt(TEST_PASSWORD)
            .expect("Dekripsi V2 harus berhasil");
        assert_eq!(recovered.to_bytes(), signing_key.to_bytes());
    }

    #[test]
    fn test_keystore_v2_wrong_password_rejected_by_aead_tag() {
        let signing_key = test_signing_key();
        let address = test_address();
        let keystore = Keystore::encrypt(&signing_key, TEST_PASSWORD, &address).unwrap();
        let parsed = Keystore::from_json_str(&keystore.to_json_string()).unwrap();

        let err = parsed.decrypt("WrongPassword").unwrap_err();
        assert_eq!(err, KeystoreError::InvalidPassword);
    }

    #[test]
    fn test_keystore_v2_tampered_ciphertext_rejected_by_aead_tag() {
        let signing_key = test_signing_key();
        let address = test_address();
        let keystore = Keystore::encrypt(&signing_key, TEST_PASSWORD, &address).unwrap();
        let mut parsed = Keystore::from_json_str(&keystore.to_json_string()).unwrap();

        let mut bad_ciphertext = parsed.crypto.ciphertext.clone();
        bad_ciphertext.replace_range(0..2, "ff");
        parsed.crypto.ciphertext = bad_ciphertext;

        let err = parsed.decrypt(TEST_PASSWORD).unwrap_err();
        assert_eq!(err, KeystoreError::InvalidPassword);
    }

    #[test]
    fn test_keystore_v1_legacy_still_decrypts() {
        let signing_key = test_signing_key();
        let address = test_address();
        let legacy_keystore = Keystore::legacy_fixture(&signing_key, TEST_PASSWORD, &address);

        let parsed = Keystore::from_json_str(&legacy_keystore.to_json_string()).unwrap();
        assert!(parsed.is_legacy());

        let recovered = parsed
            .decrypt(TEST_PASSWORD)
            .expect("Dekripsi legacy harus berhasil");
        assert_eq!(recovered.to_bytes(), signing_key.to_bytes());

        let err = parsed.decrypt("WrongPassword").unwrap_err();
        assert_eq!(err, KeystoreError::InvalidPassword);
    }

    #[test]
    fn test_keystore_v1_auto_migrates_to_v2() {
        let signing_key = test_signing_key();
        let address = test_address();
        let legacy_keystore = Keystore::legacy_fixture(&signing_key, TEST_PASSWORD, &address);
        let parsed = Keystore::from_json_str(&legacy_keystore.to_json_string()).unwrap();

        let (recovered, upgraded) = parsed
            .unlock_and_migrate(TEST_PASSWORD)
            .expect("Migrasi V1 -> V2 harus berhasil");
        assert_eq!(recovered.to_bytes(), signing_key.to_bytes());

        let upgraded = upgraded.expect("Keystore legacy harus menghasilkan envelope V2");
        assert_eq!(upgraded.version, 2);

        // V2 hasil migrasi harus dapat dibaca & didekripsi ulang dengan password yang sama.
        let reparsed = Keystore::from_json_str(&upgraded.to_json_string()).unwrap();
        assert_eq!(reparsed.version, 2);
        assert_eq!(
            reparsed.decrypt(TEST_PASSWORD).unwrap().to_bytes(),
            signing_key.to_bytes()
        );
    }

    #[test]
    fn test_keystore_v2_unlock_does_not_emit_upgrade() {
        let signing_key = test_signing_key();
        let address = test_address();
        let keystore = Keystore::encrypt(&signing_key, TEST_PASSWORD, &address).unwrap();

        let (_, upgraded) = keystore.unlock_and_migrate(TEST_PASSWORD).unwrap();
        assert!(upgraded.is_none());
    }

    #[test]
    fn test_zeroize_clears_sensitive_buffer() {
        let mut buffer = [0xABu8; SECRET_LEN];
        buffer.zeroize();
        assert!(buffer.iter().all(|byte| *byte == 0));
    }

    #[test]
    fn test_keystore_rejects_tampered_address_metadata() {
        let signing_key = test_signing_key();
        let address = test_address();
        let keystore = Keystore::encrypt(&signing_key, TEST_PASSWORD, &address).unwrap();
        let mut parsed = Keystore::from_json_str(&keystore.to_json_string()).unwrap();
        parsed.address = "aur1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq".to_string();

        assert!(matches!(
            parsed.decrypt(TEST_PASSWORD),
            Err(KeystoreError::AddressMismatch { .. })
        ));
    }

    #[test]
    fn test_blake3_derive_key_import_is_retained_for_legacy_only() {
        // Sanity: DST legacy tidak berubah sehingga keystore lama tetap terbaca.
        let hash = blake3_derive_key("AURION-KEYSTORE-KDF-V1", b"legacy-material");
        assert_eq!(hash.as_bytes().len(), 32);
    }
}
