//! Mesin Eksekusi Rollup Layer-2 (L2 VM).
//! Menjalankan State Transition Function (STF) L2 dengan Zero-Float Arithmetic.

use thiserror::Error;
use crate::core::Quantum;
use crate::l2::state::{L2Account, L2StateStore};
use crate::l2::types::{L2Receipt, L2Transaction};

/// Kesalahan Eksekusi Transaksi L2
#[derive(Debug, Error, PartialEq, Eq)]
pub enum L2ExecutionError {
    #[error("Pengirim akun L2 tidak ditemukan")]
    SenderNotFound,

    #[error("Saldo L2 pengirim tidak mencukupi")]
    InsufficientBalance,

    #[error("Nonce L2 tidak valid (diharapkan {expected}, diterima {actual})")]
    InvalidNonce { expected: u64, actual: u64 },

    #[error("Terjadi overflow pada aritmetika Quantum")]
    ArithmeticOverflow,
}

/// Mesin Eksekusi Rollup L2
#[derive(Debug, Default, Clone)]
pub struct L2ExecutionEngine;

impl L2ExecutionEngine {
    /// Membuat instance baru L2ExecutionEngine
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Mengeksekusi transaksi L2 tunggal dan memutasi state L2 secara atomik
    pub fn execute_transaction(
        &self,
        state: &mut L2StateStore,
        tx: &L2Transaction,
    ) -> Result<L2Receipt, L2ExecutionError> {
        // 1. Ambil akun pengirim
        let sender_acc = state
            .get_account(&tx.sender)
            .cloned()
            .ok_or(L2ExecutionError::SenderNotFound)?;

        // 2. Validasi Nonce
        if sender_acc.nonce != tx.nonce {
            return Err(L2ExecutionError::InvalidNonce {
                expected: sender_acc.nonce,
                actual: tx.nonce,
            });
        }

        // 3. Validasi Saldo (Amount + Fee)
        let total_required = tx
            .amount
            .as_u128()
            .checked_add(tx.fee.as_u128())
            .ok_or(L2ExecutionError::ArithmeticOverflow)?;

        if sender_acc.balance.as_u128() < total_required {
            return Err(L2ExecutionError::InsufficientBalance);
        }

        // 4. Potong Saldo Pengirim & Naikkan Nonce
        let new_sender_balance = sender_acc
            .balance
            .as_u128()
            .checked_sub(total_required)
            .ok_or(L2ExecutionError::ArithmeticOverflow)?;

        let updated_sender = L2Account {
            address: tx.sender,
            balance: Quantum::new(new_sender_balance),
            nonce: sender_acc
                .nonce
                .checked_add(1)
                .ok_or(L2ExecutionError::ArithmeticOverflow)?,
        };
        state.set_account(updated_sender);

        // 5. Tambah Saldo Penerima
        let recipient_acc = state.get_account(&tx.recipient).cloned().unwrap_or(L2Account {
            address: tx.recipient,
            balance: Quantum::ZERO,
            nonce: 0,
        });

        let new_recipient_balance = recipient_acc
            .balance
            .as_u128()
            .checked_add(tx.amount.as_u128())
            .ok_or(L2ExecutionError::ArithmeticOverflow)?;

        let updated_recipient = L2Account {
            address: tx.recipient,
            balance: Quantum::new(new_recipient_balance),
            nonce: recipient_acc.nonce,
        };
        state.set_account(updated_recipient);

        // 6. Terbitkan Receipt
        Ok(L2Receipt {
            tx_hash: tx.compute_hash(),
            success: true,
            gas_used: 21_000,
            fee_paid: tx.fee,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Address, Signature};

    #[test]
    fn test_execute_transaction_success() {
        let mut state = L2StateStore::new();
        let engine = L2ExecutionEngine::new();

        let sender = Address::from_bytes([1u8; 32]);
        let recipient = Address::from_bytes([2u8; 32]);

        state.set_account(L2Account {
            address: sender,
            balance: Quantum::new(1_000_000_000), // 10 AUR
            nonce: 0,
        });

        let tx = L2Transaction {
            sender,
            recipient,
            amount: Quantum::new(400_000_000), // 4 AUR
            fee: Quantum::new(10_000),        // 0.0001 AUR
            nonce: 0,
            signature: Signature::from_bytes([0u8; 64]),
            payload: vec![],
        };

        let receipt = engine.execute_transaction(&mut state, &tx).unwrap();
        assert!(receipt.success);

        let sender_after = state.get_account(&sender).unwrap();
        assert_eq!(sender_after.balance.as_u128(), 599_990_000);
        assert_eq!(sender_after.nonce, 1);

        let recipient_after = state.get_account(&recipient).unwrap();
        assert_eq!(recipient_after.balance.as_u128(), 400_000_000);
    }

    #[test]
    fn test_insufficient_balance_rejected() {
        let mut state = L2StateStore::new();
        let engine = L2ExecutionEngine::new();

        let sender = Address::from_bytes([1u8; 32]);
        let recipient = Address::from_bytes([2u8; 32]);

        state.set_account(L2Account {
            address: sender,
            balance: Quantum::new(100_000), // Hanya 0.001 AUR
            nonce: 0,
        });

        let tx = L2Transaction {
            sender,
            recipient,
            amount: Quantum::new(400_000_000),
            fee: Quantum::new(10_000),
            nonce: 0,
            signature: Signature::from_bytes([0u8; 64]),
            payload: vec![],
        };

        let err = engine.execute_transaction(&mut state, &tx).unwrap_err();
        assert_eq!(err, L2ExecutionError::InsufficientBalance);
    }
}
