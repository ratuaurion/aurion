//! Ed25519: Tanda tangan digital kanonikal dengan verifikasi ketat RFC 8032 (Anti-Malleability).

use crate::core::Signature;
use ed25519_dalek::{Signature as DalekSignature, Signer, SigningKey, VerifyingKey};
use thiserror::Error;
use zeroize::Zeroize;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Ed25519Error {
    #[error("Invalid public key encoding: {0}")]
    InvalidPublicKey(String),
    #[error("Signature verification failed")]
    VerificationFailed,
}

/// Keypair Ed25519 dengan pembersihan memori otomatis (zeroize).
pub struct Keypair {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl Keypair {
    /// Pembangkitan deterministik dari seed privat 32-byte CSPRNG.
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(seed);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Pembangkitan kunci acak baru menggunakan entropi sistem dan Blake3 KDF.
    pub fn generate() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let pid = std::process::id();
        let mut entropy = Vec::with_capacity(32);
        entropy.extend_from_slice(&now.to_be_bytes());
        entropy.extend_from_slice(&pid.to_be_bytes());
        let hash = crate::crypto::blake3_derive_key("AURION-EPHEMERAL-KEYGEN", &entropy);
        Self::from_seed(hash.as_bytes())
    }

    #[inline]
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    /// Tanda tangani pesan menggunakan Ed25519 kanonikal.
    pub fn sign(&self, message: &[u8]) -> Signature {
        let sig = self.signing_key.sign(message);
        Signature(sig.to_bytes())
    }

    pub fn to_signing_key(&self) -> SigningKey {
        SigningKey::from_bytes(&self.signing_key.to_bytes())
    }

    /// Bangun `Keypair` dari `SigningKey` yang sudah ada.
    ///
    /// Dipakai saat material kunci berasal dari keystore terenkripsi dan perlu
    /// dipakai oleh komponen yang bekerja dengan `Keypair` (mis. faucet).
    /// `Drop` tetap akan zeroize salinan seed privat yang tersimpan di struct.
    #[must_use]
    pub fn from_signing_key(signing_key: &SigningKey) -> Self {
        let signing_key = SigningKey::from_bytes(&signing_key.to_bytes());
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Derivasi alamat kanonikal langsung dari kunci publik.
    pub fn derive_address(&self) -> crate::core::Address {
        crate::crypto::bech32m::derive_address_from_pubkey(&self.public_key_bytes())
    }
}

impl Drop for Keypair {
    fn drop(&mut self) {
        let mut bytes = self.signing_key.to_bytes();
        bytes.zeroize();
    }
}

impl Clone for Keypair {
    fn clone(&self) -> Self {
        Self::from_seed(&self.signing_key.to_bytes())
    }
}

/// Verifikasi tanda tangan Ed25519 dengan aturan ketat RFC 8032 (Anti-Malleability).
pub fn ed25519_verify_strict(
    pubkey: &[u8; 32],
    message: &[u8],
    signature: &Signature,
) -> Result<(), Ed25519Error> {
    let verifying_key = VerifyingKey::from_bytes(pubkey)
        .map_err(|e| Ed25519Error::InvalidPublicKey(e.to_string()))?;

    let dalek_sig = DalekSignature::from_bytes(signature.as_bytes());

    verifying_key
        .verify_strict(message, &dalek_sig)
        .map_err(|_| Ed25519Error::VerificationFailed)
}
