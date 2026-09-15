//! Modul Penandatanganan Transparan (Clear Signing Mandate) Dompet Aurion.
//! Mematuhi Dokumen 01 (01-WALLET-RULES.md Bagian 3) dan Invariant AUR-ARCH-012 (Zero Float).

use crate::codec::CanonicalEncode;
use crate::core::{Address, Quantum, Signature};
use crate::crypto::decode_address_bech32m;
use crate::transaction::types::{Transaction, TxType, MAX_TRANSACTION_PAYLOAD_BYTES};
use ed25519_dalek::{Signer, SigningKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigningError {
    ZeroAmount,
    AmountExceedsSupply(u128),
    FeeBelowMinimum(u128),
    ArithmeticOverflow,
    SelfTransferProhibited,
    PayloadTooLarge(usize),
    InvalidAddress(String),
}

impl std::fmt::Display for SigningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroAmount => write!(f, "Nilai transfer harus lebih besar dari 0 Quantum"),
            Self::AmountExceedsSupply(a) => write!(f, "Nilai transfer {a} Q melebihi pasokan maksimum (66M AUR)"),
            Self::FeeBelowMinimum(fee) => write!(f, "Biaya transaksi {fee} Q di bawah batas minimum (10.000 Q)"),
            Self::ArithmeticOverflow => write!(f, "Total debet (amount + fee) mengalami overflow aritmatika"),
            Self::SelfTransferProhibited => write!(f, "Pengiriman dana ke alamat diri sendiri dilarang"),
            Self::PayloadTooLarge(len) => write!(f, "Ukuran payload memo ({len} B) melebihi batas kanonikal"),
            Self::InvalidAddress(e) => write!(f, "Alamat tidak valid: {e}"),
        }
    }
}

impl std::error::Error for SigningError {}

/// Rincian manusiawi untuk disajikan kepada pengguna sebelum konfirmasi (Clear Signing).
#[derive(Debug, Clone)]
pub struct ClearSigningDetails {
    pub sender: Address,
    pub sender_bech32m: String,
    pub recipient: Address,
    pub recipient_bech32m: String,
    pub amount: Quantum,
    pub fee: Quantum,
    pub nonce: u64,
    pub memo: String,
}

impl ClearSigningDetails {
    pub fn new(
        sender_bech32m: &str,
        recipient_bech32m: &str,
        amount_quanta: u128,
        fee_quanta: u128,
        nonce: u64,
        memo: &str,
    ) -> Result<Self, SigningError> {
        let sender = decode_address_bech32m(sender_bech32m, "aur")
            .map_err(|e| SigningError::InvalidAddress(format!("Sender Bech32m error: {e}")))?;
        let recipient = decode_address_bech32m(recipient_bech32m, "aur")
            .map_err(|e| SigningError::InvalidAddress(format!("Recipient Bech32m error: {e}")))?;

        // 1. amount > 0
        if amount_quanta == 0 {
            return Err(SigningError::ZeroAmount);
        }

        // 2. amount <= MAX_SUPPLY_QUANTA (66_000_000 * 10^8)
        let max_supply = 66_000_000u128 * 100_000_000u128;
        if amount_quanta > max_supply {
            return Err(SigningError::AmountExceedsSupply(amount_quanta));
        }

        // 3. fee >= MIN_TX_FEE_QUANTA (10_000 Q)
        if fee_quanta < 10_000 {
            return Err(SigningError::FeeBelowMinimum(fee_quanta));
        }

        // 4. amount.checked_add(fee)
        if amount_quanta.checked_add(fee_quanta).is_none() {
            return Err(SigningError::ArithmeticOverflow);
        }

        // 5. sender != recipient
        if sender == recipient {
            return Err(SigningError::SelfTransferProhibited);
        }

        // 6. payload length
        if memo.len() > MAX_TRANSACTION_PAYLOAD_BYTES {
            return Err(SigningError::PayloadTooLarge(memo.len()));
        }

        Ok(Self {
            sender,
            sender_bech32m: sender_bech32m.to_string(),
            recipient,
            recipient_bech32m: recipient_bech32m.to_string(),
            amount: Quantum::new(amount_quanta),
            fee: Quantum::new(fee_quanta),
            nonce,
            memo: memo.to_string(),
        })
    }

    /// Menghitung rincian pembakaran biaya 20% protokol dan 80% imbalan penambang (Zero Float).
    pub fn fee_split(&self) -> (Quantum, Quantum) {
        let fee_raw = self.fee.as_u128();
        let burn_raw = fee_raw * 20 / 100;
        let miner_raw = fee_raw - burn_raw;
        (Quantum::new(burn_raw), Quantum::new(miner_raw))
    }

    /// Format teks prompt transparan manusiawi untuk CLI atau Hardware Display.
    pub fn format_clear_signing_prompt(&self) -> String {
        let (burn, miner) = self.fee_split();
        let amount_aur_whole = self.amount.as_u128() / 100_000_000;
        let amount_aur_frac = self.amount.as_u128() % 100_000_000;
        let fee_aur_whole = self.fee.as_u128() / 100_000_000;
        let fee_aur_frac = self.fee.as_u128() % 100_000_000;

        format!(
            r#"================================================================================
                   AURION CLEAR SIGNING VERIFICATION PROMPT
================================================================================
  Sender (From):    {}
  Recipient (To):   {}
  Amount:           {} Quantum ({}.{:08} AUR)
  Network Fee:      {} Quantum ({}.{:08} AUR)
    ├─ Permanent Burn (20%):   {} Quantum
    └─ Miner Reward   (80%):   {} Quantum
  Account Nonce:    {}
  Memo / Payload:   "{}"
--------------------------------------------------------------------------------
  [MANDAT DOKUMEN 01]: Seluruh field di atas diverifikasi langsung dari
  preimage kanonikal sebelum menandatangani dengan kunci privat Ed25519.
================================================================================
"#,
            self.sender_bech32m,
            self.recipient_bech32m,
            self.amount.as_u128(),
            amount_aur_whole,
            amount_aur_frac,
            self.fee.as_u128(),
            fee_aur_whole,
            fee_aur_frac,
            burn.as_u128(),
            miner.as_u128(),
            self.nonce,
            self.memo
        )
    }

    /// Menandatangani transaksi kanonikal dan mengembalikan objek Transaction serta string hex.
    pub fn sign(
        &self,
        signing_key: &SigningKey,
        chain_id: u32,
        valid_until: u64,
    ) -> (Transaction, String) {
        let mut tx = Transaction {
            version: 1,
            chain_id,
            tx_type: TxType::Transfer,
            flags: 0,
            sender: self.sender,
            recipient: self.recipient,
            nonce: self.nonce,
            amount: self.amount,
            fee: self.fee,
            valid_until,
            payload: self.memo.as_bytes().to_vec(),
            signature: Signature::ZERO,
        };

        let preimage = tx.signing_preimage();
        let sig = signing_key.sign(&preimage);
        tx.signature = Signature(sig.to_bytes());

        let mut encoded = Vec::new();
        tx.encode_canonical(&mut encoded);
        let raw_hex = hex::encode(&encoded);

        (tx, raw_hex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{derive_address_from_pubkey, ed25519_verify_strict, encode_address_bech32m};

    #[test]
    fn test_clear_signing_validation_and_fee_split() {
        let key_sender = SigningKey::from_bytes(&[0x11u8; 32]);
        let addr_sender = derive_address_from_pubkey(key_sender.verifying_key().as_bytes());
        let sender_bech = encode_address_bech32m(&addr_sender, "aur").unwrap();

        let key_recip = SigningKey::from_bytes(&[0x22u8; 32]);
        let addr_recip = derive_address_from_pubkey(key_recip.verifying_key().as_bytes());
        let recip_bech = encode_address_bech32m(&addr_recip, "aur").unwrap();

        let details = ClearSigningDetails::new(
            &sender_bech,
            &recip_bech,
            500_000_000, // 5 AUR
            100_000,     // 0.001 AUR
            1,
            "Clear signing test",
        )
        .expect("Validasi harus berhasil");

        let (burn, miner) = details.fee_split();
        assert_eq!(burn.as_u128(), 20_000);  // 20% dari 100.000
        assert_eq!(miner.as_u128(), 80_000); // 80% dari 100.000

        let (tx, raw_hex) = details.sign(&key_sender, 1, 1000);
        assert!(!raw_hex.is_empty());
        assert_ne!(tx.signature, Signature::ZERO);

        // Verifikasi signature valid
        let preimage = tx.signing_preimage();
        let verify_res = ed25519_verify_strict(
            key_sender.verifying_key().as_bytes(),
            &preimage,
            &tx.signature,
        );
        assert!(verify_res.is_ok());
    }

    #[test]
    fn test_clear_signing_rejection_on_self_transfer() {
        let key = SigningKey::from_bytes(&[0x33u8; 32]);
        let addr = derive_address_from_pubkey(key.verifying_key().as_bytes());
        let bech = encode_address_bech32m(&addr, "aur").unwrap();

        let res = ClearSigningDetails::new(&bech, &bech, 100_000_000, 10_000, 0, "");
        assert_eq!(res.unwrap_err(), SigningError::SelfTransferProhibited);
    }
}
