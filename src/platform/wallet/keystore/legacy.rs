//! Implementasi legacy `blake3-stream-v1` (AUR-ISSUE-004).
//!
//! Modul ini **hanya** dipertahankan untuk membaca keystore lama dan
//! memigrasikannya ke format kanonik V2 (Argon2id + ChaCha20Poly1305).
//! Enkripsi baru TIDAK PERNAH memakai jalur ini.

use crate::crypto::blake3_derive_key;

#[cfg(test)]
use crate::crypto::blake3_hash;

/// Identitas cipher legacy pada field `crypto.cipher`.
pub(crate) const CIPHER_ID: &str = "blake3-stream-v1";

const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const SECRET_LEN: usize = 32;

/// Ciphertext legacy yang sudah didekode dari representasi hex.
pub(crate) struct LegacyCiphertext {
    pub ciphertext: [u8; SECRET_LEN],
    pub salt: [u8; SALT_LEN],
    pub nonce: [u8; NONCE_LEN],
    pub mac: [u8; SECRET_LEN],
}

/// Menurunkan kunci KDF legacy dari password dan salt.
fn derive_key(password: &str, salt: &[u8]) -> crate::core::Hash256 {
    let mut kdf_input = Vec::with_capacity(password.len() + salt.len());
    kdf_input.extend_from_slice(password.as_bytes());
    kdf_input.extend_from_slice(salt);
    blake3_derive_key("AURION-KEYSTORE-KDF-V1", &kdf_input)
}

fn cipher_stream(derived_key: &[u8; 32], nonce: &[u8]) -> crate::core::Hash256 {
    let mut cipher_input = Vec::with_capacity(64 + nonce.len());
    cipher_input.extend_from_slice(derived_key);
    cipher_input.extend_from_slice(nonce);
    blake3_derive_key("AURION-KEYSTORE-CIPHER-V1", &cipher_input)
}

fn mac(derived_key: &[u8; 32], ciphertext: &[u8; SECRET_LEN]) -> crate::core::Hash256 {
    let mut mac_input = Vec::with_capacity(64);
    mac_input.extend_from_slice(derived_key);
    mac_input.extend_from_slice(ciphertext);
    blake3_derive_key("AURION-KEYSTORE-MAC-V1", &mac_input)
}

/// Mendekripsi ciphertext legacy. Mengembalikan `None` bila verifikasi MAC gagal.
pub(crate) fn decrypt(password: &str, ct: &LegacyCiphertext) -> Option<[u8; SECRET_LEN]> {
    let derived_key = derive_key(password, &ct.salt);
    let calculated = mac(derived_key.as_bytes(), &ct.ciphertext);
    if calculated.as_bytes() != &ct.mac {
        return None;
    }

    let stream = cipher_stream(derived_key.as_bytes(), &ct.nonce);
    let mut secret = [0u8; SECRET_LEN];
    for (i, byte) in secret.iter_mut().enumerate() {
        *byte = ct.ciphertext[i] ^ stream.as_bytes()[i];
    }
    Some(secret)
}

/// Mengenkripsi secret dengan jalur legacy. Hanya dipakai untuk test migrasi.
#[cfg(test)]
pub(crate) fn encrypt(secret: &[u8; SECRET_LEN], password: &str) -> LegacyCiphertext {
    let salt = generate_pseudo_random_32(password.as_bytes());
    let nonce = generate_pseudo_random_12(&salt);
    let derived_key = derive_key(password, &salt);
    let stream = cipher_stream(derived_key.as_bytes(), &nonce);

    let mut ciphertext = [0u8; SECRET_LEN];
    for (i, byte) in ciphertext.iter_mut().enumerate() {
        *byte = secret[i] ^ stream.as_bytes()[i];
    }

    let mut mac_arr = [0u8; SECRET_LEN];
    mac_arr.copy_from_slice(mac(derived_key.as_bytes(), &ciphertext).as_bytes());

    LegacyCiphertext {
        ciphertext,
        salt,
        nonce,
        mac: mac_arr,
    }
}

#[cfg(test)]
fn generate_pseudo_random_32(seed_entropy: &[u8]) -> [u8; SALT_LEN] {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut input = Vec::with_capacity(seed_entropy.len() + 16);
    input.extend_from_slice(seed_entropy);
    input.extend_from_slice(&now.to_be_bytes());
    *blake3_hash(&input).as_bytes()
}

#[cfg(test)]
fn generate_pseudo_random_12(salt: &[u8; SALT_LEN]) -> [u8; NONCE_LEN] {
    let hash = blake3_hash(salt);
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&hash.as_bytes()[0..NONCE_LEN]);
    nonce
}
