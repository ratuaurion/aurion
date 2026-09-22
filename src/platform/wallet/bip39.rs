//! Enkoder dan Dekoder Mnemonik BIP-39 (24 Kata / 256-bit Entropi).
//! Mematuhi Dokumen 01 (01-WALLET-RULES.md Bagian 1.1).

use crate::wallet::sha512::pbkdf2_hmac_sha512;
use crate::wallet::wordlist::BIP39_WORDLIST;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MnemonicError {
    InvalidWordCount(usize),
    UnknownWord(String),
    ChecksumMismatch,
}

impl std::fmt::Display for MnemonicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidWordCount(n) => write!(f, "Jumlah kata harus tepat 24 kata, ditemukan: {n}"),
            Self::UnknownWord(w) => write!(f, "Kata tidak dikenal dalam kamus BIP-39: '{w}'"),
            Self::ChecksumMismatch => write!(f, "Checksum mnemonik tidak cocok"),
        }
    }
}

impl std::error::Error for MnemonicError {}

/// Mengonversi 32-byte (256-bit) entropi kriptografis menjadi 24 kata BIP-39.
#[allow(clippy::chunks_exact_to_as_chunks)]
pub fn entropy_to_mnemonic_24(entropy: &[u8; 32]) -> String {
    let checksum_byte = Sha256::digest(entropy)[0];

    // 256 bit entropi + 8 bit checksum = 264 bit total
    let mut bits = Vec::with_capacity(264);
    for byte in entropy {
        for i in (0..8).rev() {
            bits.push((byte >> i) & 1);
        }
    }
    for i in (0..8).rev() {
        bits.push((checksum_byte >> i) & 1);
    }

    let mut words = Vec::with_capacity(24);
    for chunk in bits.chunks_exact(11) {
        let mut idx: usize = 0;
        for &bit in chunk {
            idx = (idx << 1) | (bit as usize);
        }
        words.push(BIP39_WORDLIST[idx]);
    }

    words.join(" ")
}

/// Mengonversi 24 kata BIP-39 kembali menjadi 32-byte entropi dan memverifikasi checksum.
pub fn mnemonic_to_entropy_24(mnemonic: &str) -> Result<[u8; 32], MnemonicError> {
    let words: Vec<&str> = mnemonic.split_whitespace().collect();
    if words.len() != 24 {
        return Err(MnemonicError::InvalidWordCount(words.len()));
    }

    let mut bits = Vec::with_capacity(264);
    for word in words {
        let idx = BIP39_WORDLIST
            .iter()
            .position(|&w| w == word)
            .ok_or_else(|| MnemonicError::UnknownWord(word.to_string()))?;

        for i in (0..11).rev() {
            bits.push(((idx >> i) & 1) as u8);
        }
    }

    let mut entropy = [0u8; 32];
    for (i, byte) in entropy.iter_mut().enumerate() {
        let mut b = 0u8;
        for j in 0..8 {
            b = (b << 1) | bits[i * 8 + j];
        }
        *byte = b;
    }

    let mut checksum_byte = 0u8;
    for j in 0..8 {
        checksum_byte = (checksum_byte << 1) | bits[256 + j];
    }

    let expected_checksum = Sha256::digest(entropy)[0];

    if checksum_byte != expected_checksum {
        entropy.zeroize();
        return Err(MnemonicError::ChecksumMismatch);
    }

    Ok(entropy)
}

/// Menghasilkan 512-bit (64-byte) master seed dari frase mnemonik dan passphrase (BIP-39).
pub fn mnemonic_to_seed(mnemonic: &str, passphrase: &str) -> [u8; 64] {
    let salt = format!("mnemonic{passphrase}");
    pbkdf2_hmac_sha512(mnemonic.as_bytes(), salt.as_bytes(), 2048)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_to_mnemonic_and_back_deterministic() {
        let entropy = [0x42u8; 32];
        let mnemonic = entropy_to_mnemonic_24(&entropy);
        let words: Vec<&str> = mnemonic.split_whitespace().collect();
        assert_eq!(words.len(), 24);

        let recovered = mnemonic_to_entropy_24(&mnemonic).expect("Pemulihan harus valid");
        assert_eq!(entropy, recovered);
    }

    #[test]
    fn test_invalid_checksum_rejected() {
        let entropy = [0x11u8; 32];
        let mnemonic = entropy_to_mnemonic_24(&entropy);
        let mut words: Vec<&str> = mnemonic.split_whitespace().collect();
        // Ubah kata terakhir
        words[23] = "zoo";
        let tampered = words.join(" ");

        let res = mnemonic_to_entropy_24(&tampered);
        assert!(res.is_err());
    }

    #[test]
    fn test_bip39_official_vectors_256bit() {
        let vectors = [
            (
                [0x00; 32],
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art",
            ),
            (
                [0x7f; 32],
                "legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth title",
            ),
            (
                [0xff; 32],
                "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote",
            ),
        ];

        for (entropy, expected_mnemonic) in vectors {
            let mnemonic = entropy_to_mnemonic_24(&entropy);
            assert_eq!(mnemonic, expected_mnemonic);
            assert_eq!(mnemonic_to_entropy_24(&mnemonic), Ok(entropy));
        }
    }
}
