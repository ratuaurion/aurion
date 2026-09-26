//! Trait [`Signer`]: clear signing terstruktur untuk transaksi kontrak.
//!
//! Kontrak utama keamanan: signer **tidak pernah** menandatangani hash buta —
//! intent harus terbukti identik dengan transaksi (`verify_against`) dan prompt
//! manusiawi wajib disetujui sebelum tanda tangan Ed25519 dibuat.

use std::io::Write;

use ed25519_dalek::Signer as _;

use crate::core::Address;
use crate::crypto::derive_address_from_pubkey;
use crate::transaction::types::Transaction;
use crate::transaction::validator::validate_transaction_stateless;

use super::error::ContractError;
use super::intent::ContractIntent;

/// Kebijakan persetujuan prompt clear signing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalMode {
    /// Cetak prompt dan baca konfirmasi `y` dari stdin.
    Interactive,
    /// Setujui otomatis (otomatisasi/CI) — prompt tetap dicetak untuk audit.
    AutoApprove,
    /// Selalu tolak (mode aman untuk pengujian penolakan).
    Reject,
}

/// Penandatangan transaksi kontrak (diposisikan Wallet sebagai pemilik kunci).
pub trait Signer {
    /// Alamat pengirim yang dimiliki signer.
    fn address(&self) -> Address;

    /// Kunci publik Ed25519 (32 byte) yang cocok dengan [`Signer::address`].
    fn public_key(&self) -> [u8; 32];

    /// Minta persetujuan pengguna atas prompt clear signing.
    fn approve(&self, prompt: &str) -> bool;

    /// Isi tanda tangan Ed25519 pada transaksi (dipanggil setelah persetujuan).
    ///
    /// # Errors
    /// Kegagalan internal signer.
    fn sign_raw(&self, tx: &Transaction) -> Result<Transaction, ContractError>;

    /// Alur clear signing lengkap: verifikasi intent -> persetujuan -> tanda
    /// tangan -> validasi nir-status.
    ///
    /// # Errors
    /// - Intent tidak cocok dengan transaksi (anti-tamper).
    /// - Signer bukan pemilik `tx.sender`.
    /// - Pengguna menolak.
    /// - Validasi nir-status gagal.
    fn sign(
        &self,
        intent: &ContractIntent,
        tx: &Transaction,
    ) -> Result<Transaction, ContractError> {
        if tx.sender != self.address() {
            return Err(ContractError::SignerMismatch);
        }
        intent.verify_against(tx)?;
        let prompt = intent.format_clear_signing_prompt();
        if !self.approve(&prompt) {
            return Err(ContractError::UserRejected);
        }
        let signed = self.sign_raw(tx)?;
        validate_transaction_stateless(&signed, &self.public_key())
            .map_err(|e| ContractError::StatelessValidation(e.to_string()))?;
        Ok(signed)
    }
}

/// Signer berbasis keystore/kunci mentah milik Wallet Aurion.
#[derive(Clone)]
pub struct KeystoreSigner {
    key: ed25519_dalek::SigningKey,
    address: Address,
    mode: ApprovalMode,
}

impl KeystoreSigner {
    /// Dari seed 32-byte (kunci deterministik untuk pengujian/otomatisasi).
    #[must_use]
    pub fn from_seed(seed: [u8; 32], mode: ApprovalMode) -> Self {
        let key = ed25519_dalek::SigningKey::from_bytes(&seed);
        let address = derive_address_from_pubkey(key.verifying_key().as_bytes());
        Self { key, address, mode }
    }

    /// Dari [`ed25519_dalek::SigningKey`] yang sudah ada.
    #[must_use]
    pub fn from_signing_key(key: ed25519_dalek::SigningKey, mode: ApprovalMode) -> Self {
        let address = derive_address_from_pubkey(key.verifying_key().as_bytes());
        Self { key, address, mode }
    }

    /// Dari file keystore terenkripsi (password dibaca sesuai parameter).
    ///
    /// # Errors
    /// File tidak dapat dibaca / password salah / format rusak.
    pub fn from_keystore_file(
        path: &str,
        password: &str,
        mode: ApprovalMode,
    ) -> Result<Self, ContractError> {
        let raw = std::fs::read_to_string(path).map_err(|e| {
            ContractError::Provider(format!("Gagal membuka keystore '{path}': {e}"))
        })?;
        Self::from_keystore_json(&raw, password, mode)
    }

    /// Dari isi JSON keystore.
    ///
    /// # Errors
    /// Format rusak / password salah.
    pub fn from_keystore_json(
        raw: &str,
        password: &str,
        mode: ApprovalMode,
    ) -> Result<Self, ContractError> {
        let keystore = crate::wallet::keystore::Keystore::from_json_str(raw)
            .map_err(|e| ContractError::Provider(format!("Format keystore rusak: {e}")))?;
        let (key, _upgraded) = keystore
            .unlock_and_migrate(password)
            .map_err(|e| ContractError::Provider(format!("Gagal membuka keystore: {e}")))?;
        Ok(Self::from_signing_key(key, mode))
    }

    /// Kebijakan persetujuan yang dipakai signer ini.
    #[must_use]
    pub fn approval_mode(&self) -> ApprovalMode {
        self.mode
    }

    /// Ekspor kunci privat ke bentuk `Keypair` Aurion.
    ///
    /// Hanya untuk komponen yang harus menandatangani (faucet).
    /// `Keypair` membasahi seed privat saat di-drop, dan `Debug` keduanya
    /// sama-sama tidak pernah membocorkan kunci privat (AUR-ARCH-015).
    #[must_use]
    pub fn to_keypair(&self) -> crate::crypto::Keypair {
        crate::crypto::Keypair::from_signing_key(&self.key)
    }
}

/// `Debug` manual: **tidak pernah** membocorkan seed kunci privat (AUR-ARCH-015).
impl std::fmt::Debug for KeystoreSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeystoreSigner")
            .field("address", &self.address)
            .field("mode", &self.mode)
            .field("private_key", &"<redacted>")
            .finish()
    }
}

impl Signer for KeystoreSigner {
    fn address(&self) -> Address {
        self.address
    }

    fn public_key(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }

    fn approve(&self, prompt: &str) -> bool {
        print!("{prompt}");
        let _ = std::io::stdout().flush();
        match self.mode {
            ApprovalMode::Reject => false,
            ApprovalMode::AutoApprove => true,
            ApprovalMode::Interactive => {
                print!("Setujui & tanda tangani? [y/N]: ");
                let _ = std::io::stdout().flush();
                let mut line = String::new();
                if std::io::stdin().read_line(&mut line).is_err() {
                    return false;
                }
                matches!(line.trim(), "y" | "Y")
            }
        }
    }

    fn sign_raw(&self, tx: &Transaction) -> Result<Transaction, ContractError> {
        let mut signed = tx.clone();
        let preimage = signed.signing_preimage();
        let signature = self.key.sign(&preimage);
        signed.signature = crate::core::Signature(signature.to_bytes());
        Ok(signed)
    }
}
