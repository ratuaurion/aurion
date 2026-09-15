//! Validasi stateless transaksi protokol Aurion.

use crate::core::Quantum;
use crate::crypto::ed25519_verify_strict;
use crate::transaction::types::{Transaction, MAX_TRANSACTION_PAYLOAD_BYTES};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransactionValidationError {
    #[error("Transaction payload exceeds maximum length of {max} bytes: got {got}")]
    PayloadTooLarge { max: usize, got: usize },
    #[error("Invalid transaction fee: fee must be greater than zero")]
    ZeroFee,
    #[error("Transaction signature verification failed")]
    InvalidSignature,
    #[error("Invalid transaction version: expected {expected}, got {got}")]
    UnsupportedVersion { expected: u16, got: u16 },
}

/// Validasi stateless transaksi Aurion sebelum masuk ke mempool.
pub fn validate_transaction_stateless(
    tx: &Transaction,
    sender_pubkey: &[u8; 32],
) -> Result<(), TransactionValidationError> {
    if tx.version != 1 {
        return Err(TransactionValidationError::UnsupportedVersion {
            expected: 1,
            got: tx.version,
        });
    }

    if tx.payload.len() > MAX_TRANSACTION_PAYLOAD_BYTES {
        return Err(TransactionValidationError::PayloadTooLarge {
            max: MAX_TRANSACTION_PAYLOAD_BYTES,
            got: tx.payload.len(),
        });
    }

    if tx.fee == Quantum::ZERO {
        return Err(TransactionValidationError::ZeroFee);
    }

    let preimage = tx.signing_preimage();
    ed25519_verify_strict(sender_pubkey, &preimage, &tx.signature)
        .map_err(|_| TransactionValidationError::InvalidSignature)
}
