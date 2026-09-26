//! Intent clear-signing kontrak: representasi manusiawi yang **diikat** ke
//! transaksi kanonikal sebelum ditandatangani (menggantikan blind signing).

use crate::core::{Address, Hash256, Quantum};
use crate::crypto::blake3_hash;
use crate::transaction::types::{Transaction, TxType};

use super::error::ContractError;
use super::metadata::format_quanta;
use super::provider::DryRunReport;

/// Jenis aksi kontrak pada intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentAction {
    /// Deploy kontrak baru (payload = konstruktor).
    Deploy,
    /// Panggilan metode kontrak (payload = call frame + runtime).
    Call,
}

/// Intent clear-signing: seluruh field yang signifikan secara finansial dan
/// semantik, dalam bentuk manusiawi, **terikat eksplisit** ke `Transaction`
/// melalui hash payload Blake3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractIntent {
    /// Jenis aksi.
    pub action: IntentAction,
    /// Chain ID transaksi.
    pub chain_id: u32,
    /// Alamat pengirim.
    pub sender: Address,
    /// Format Bech32m pengirim.
    pub sender_bech32m: String,
    /// Penerima pada header transaksi (`Address::ZERO` untuk deploy).
    pub tx_recipient: Address,
    /// Format Bech32m penerima transaksi.
    pub tx_recipient_bech32m: String,
    /// Alamat kontrak target (deploy: alamat prediksi dari nonce).
    pub contract_bech32m: String,
    /// Nama kontrak dari metadata.
    pub contract_name: String,
    /// Nama metode (untuk aksi `Call`).
    pub method_name: Option<String>,
    /// Argumen ter-render untuk ditampilkan.
    pub rendered_args: Vec<String>,
    /// Nilai Quanta dikirim bersama transaksi.
    pub amount: Quantum,
    /// Fee transaksi.
    pub fee: Quantum,
    /// Nonce transaksi.
    pub nonce: u64,
    /// Batas waktu valid transaksi (Unix detik).
    pub valid_until: u64,
    /// `blake3(tx.payload)` — mengikat intent ke byte yang tepat.
    pub payload_hash: Hash256,
    /// `code_hash` metadata kontrak (identitas kode yang dieksekusi).
    pub code_hash: Hash256,
    /// Hasil simulasi lokal yang mendahului signing.
    pub dry_run: DryRunReport,
}

impl ContractIntent {
    /// Tipe transaksi yang diharapkan untuk aksi ini.
    fn expected_tx_type(&self) -> TxType {
        match self.action {
            IntentAction::Deploy => TxType::ContractDeploy,
            IntentAction::Call => TxType::ContractCall,
        }
    }

    /// Guardrail anti-tamper: seluruh field intent **wajib identik** dengan
    /// transaksi yang akan ditandatangani. Satu ketidakcocokan pun membatalkan.
    ///
    /// # Errors
    /// Field berbeda (termasuk payload yang dimodifikasi setelah simulasi).
    pub fn verify_against(&self, tx: &Transaction) -> Result<(), ContractError> {
        if tx.version != 1 {
            return Err(ContractError::IntentMismatch(format!(
                "version {} != 1",
                tx.version
            )));
        }
        if tx.chain_id != self.chain_id {
            return Err(ContractError::IntentMismatch(format!(
                "chain_id {} != {}",
                tx.chain_id, self.chain_id
            )));
        }
        let expected_type = self.expected_tx_type();
        if tx.tx_type != expected_type {
            return Err(ContractError::IntentMismatch(format!(
                "tx_type {tx_type:?} != {expected_type:?}",
                tx_type = tx.tx_type
            )));
        }
        if tx.flags != 0 {
            return Err(ContractError::IntentMismatch(format!(
                "flags {} != 0",
                tx.flags
            )));
        }
        if tx.sender != self.sender {
            return Err(ContractError::IntentMismatch(format!(
                "sender {} != {}",
                tx.sender.to_hex(),
                self.sender.to_hex()
            )));
        }
        if tx.recipient != self.tx_recipient {
            return Err(ContractError::IntentMismatch(format!(
                "recipient {} != {}",
                tx.recipient.to_hex(),
                self.tx_recipient.to_hex()
            )));
        }
        if tx.nonce != self.nonce {
            return Err(ContractError::IntentMismatch(format!(
                "nonce {} != {}",
                tx.nonce, self.nonce
            )));
        }
        if tx.amount != self.amount {
            return Err(ContractError::IntentMismatch(format!(
                "amount {} != {}",
                tx.amount.as_u128(),
                self.amount.as_u128()
            )));
        }
        if tx.fee != self.fee {
            return Err(ContractError::IntentMismatch(format!(
                "fee {} != {}",
                tx.fee.as_u128(),
                self.fee.as_u128()
            )));
        }
        if tx.valid_until != self.valid_until {
            return Err(ContractError::IntentMismatch(format!(
                "valid_until {} != {}",
                tx.valid_until, self.valid_until
            )));
        }
        let payload_hash = blake3_hash(&tx.payload);
        if payload_hash != self.payload_hash {
            return Err(ContractError::IntentMismatch(format!(
                "hash payload {} != {}",
                payload_hash.to_hex(),
                self.payload_hash.to_hex()
            )));
        }
        Ok(())
    }

    /// Susun prompt clear signing multi-baris (bahasa manusia, bukan hash buta).
    #[must_use]
    pub fn format_clear_signing_prompt(&self) -> String {
        let action = match self.action {
            IntentAction::Deploy => "DEPLOY KONTRAK",
            IntentAction::Call => "PANGGILAN METODE",
        };
        let mut out = String::new();
        out.push_str("==================================================================\n");
        out.push_str("             AURION CLEAR SIGNING - KONTRAK AVM\n");
        out.push_str("==================================================================\n");
        out.push_str(&format!(
            "  Aksi             : {action}\n"
        ));
        out.push_str(&format!(
            "  Kontrak          : {} ({})\n",
            self.contract_name, self.contract_bech32m
        ));
        if let Some(method) = &self.method_name {
            out.push_str(&format!("  Metode           : {method}\n"));
        }
        if self.rendered_args.is_empty() {
            out.push_str("  Argumen          : (tanpa argumen)\n");
        } else {
            for (idx, arg) in self.rendered_args.iter().enumerate() {
                out.push_str(&format!("  Argumen [{idx}]        : {arg}\n"));
            }
        }
        out.push_str(&format!(
            "  Dari (Pengirim)  : {}\n",
            self.sender_bech32m
        ));
        if !self.amount.is_zero() {
            out.push_str(&format!(
                "  Nilai (Value)    : {}\n",
                format_quanta(self.amount.as_u128())
            ));
        }
        out.push_str(&format!(
            "  Fee              : {} -> 100% Validator Blok\n",
            format_quanta(self.fee.as_u128())
        ));
        out.push_str(&format!("  Nonce            : {}\n", self.nonce));
        out.push_str(&format!("  Chain ID         : {}\n", self.chain_id));
        out.push_str(&format!("  Berlaku s/d      : {}\n", self.valid_until));
        out.push_str(&format!(
            "  Code Hash        : 0x{}\n",
            self.code_hash.to_hex()
        ));
        out.push_str(&format!(
            "  Payload Blake3   : 0x{}\n",
            self.payload_hash.to_hex()
        ));
        out.push_str(&format!(
            "  Dry-Run (STF)    : {}\n",
            self.dry_run.summary()
        ));
        out.push_str("==================================================================\n");
        out
    }
}
