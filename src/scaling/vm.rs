//! Mesin Eksekusi Rollup Layer-2 (L2 VM) & Runtime Rollup.
//! Menjalankan State Transition Function (STF) L2, Gas Metering, Fee Splitting, dan Rollback Atomik.
//! Mematuhi Invariant L2-ARCH-001 (Monolithic Execution), L2-ARCH-003 (Zero-Float Quantum),
//! L2-EXEC-001 (Throughput Tinggi), dan AUR-ARCH-011 (#![forbid(unsafe_code)]).

use thiserror::Error;
use crate::core::{Hash256, Quantum};
use crate::l2::state::{L2Account, L2StateStore};
use crate::l2::types::{compute_txs_root, L2Block, L2Receipt, L2Transaction};

/// Biaya gas dasar untuk transfer L2 (10.000 gas, hemat 52% dari transfer L1)
pub const L2_BASE_TRANSFER_GAS: u64 = 10_000;

/// Biaya gas tambahan per byte payload calldata transaksi L2
pub const L2_GAS_PER_PAYLOAD_BYTE: u64 = 4;

/// Harga gas minimum dalam satuan Quanta (1 Quanta per unit gas)
pub const L2_MIN_GAS_PRICE_QUANTA: u128 = 1;

/// Persentase pembagian fee untuk Sequencer L2 (80%)
pub const L2_FEE_SPLIT_SEQUENCER_PERCENT: u128 = 80;

/// Persentase pembagian fee untuk cadangan settlement Layer-1 DA (20%)
pub const L2_FEE_SPLIT_L1_SETTLEMENT_PERCENT: u128 = 20;

/// Kesalahan Eksekusi Transaksi & Blok L2
#[derive(Debug, Error, PartialEq, Eq)]
pub enum L2ExecutionError {
    #[error("Pengirim akun L2 tidak ditemukan")]
    SenderNotFound,

    #[error("Saldo L2 pengirim tidak mencukupi untuk transfer dan fee")]
    InsufficientBalance,

    #[error("Nonce L2 tidak valid (diharapkan {expected}, diterima {actual})")]
    InvalidNonce { expected: u64, actual: u64 },

    #[error("Fee transaksi tidak mencukupi kebutuhan gas (dibutuhkan minimal {required_fee} quanta, disediakan {provided_fee} quanta)")]
    InsufficientFeeForGas { required_fee: u128, provided_fee: u128 },

    #[error("Terjadi overflow atau underflow pada aritmetika Quantum")]
    ArithmeticOverflow,

    #[error("Header blok L2 tidak valid: {0}")]
    InvalidBlockHeader(&'static str),

    #[error("Ketidaksesuaian State Root blok (diharapkan {expected}, aktual {actual})")]
    StateRootMismatch { expected: Hash256, actual: Hash256 },

    #[error("Ketidaksesuaian Transactions Root blok L2")]
    TxsRootMismatch,
}

/// Rincian Pembagian Biaya Transaksi L2 (Fee Split)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct L2FeeSplit {
    pub sequencer_fee: Quantum,
    pub l1_settlement_fee: Quantum,
}

/// Menghitung pembagian fee transaksi 80/20 antara Sequencer dan L1 DA Settlement Reserve
pub fn calculate_fee_split(total_fee: Quantum) -> Result<L2FeeSplit, L2ExecutionError> {
    let total = total_fee.as_u128();
    let sequencer_quanta = total
        .checked_mul(L2_FEE_SPLIT_SEQUENCER_PERCENT)
        .ok_or(L2ExecutionError::ArithmeticOverflow)?
        / 100;
    let l1_quanta = total.saturating_sub(sequencer_quanta);

    Ok(L2FeeSplit {
        sequencer_fee: Quantum::new(sequencer_quanta),
        l1_settlement_fee: Quantum::new(l1_quanta),
    })
}

/// Menghitung total gas yang dibutuhkan oleh transaksi berdasarkan panjang payload
#[must_use]
pub fn calculate_required_gas(payload_len: usize) -> u64 {
    let payload_gas = (payload_len as u64).saturating_mul(L2_GAS_PER_PAYLOAD_BYTE);
    L2_BASE_TRANSFER_GAS.saturating_add(payload_gas)
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
        // 1. Validasi Gas Metering & Minimum Fee
        let required_gas = calculate_required_gas(tx.payload.len());
        let min_required_fee = (required_gas as u128).saturating_mul(L2_MIN_GAS_PRICE_QUANTA);

        if tx.fee.as_u128() < min_required_fee {
            return Err(L2ExecutionError::InsufficientFeeForGas {
                required_fee: min_required_fee,
                provided_fee: tx.fee.as_u128(),
            });
        }

        // 2. Ambil akun pengirim
        let sender_acc = state
            .get_account(&tx.sender)
            .cloned()
            .ok_or(L2ExecutionError::SenderNotFound)?;

        // 3. Validasi Nonce
        if sender_acc.nonce != tx.nonce {
            return Err(L2ExecutionError::InvalidNonce {
                expected: sender_acc.nonce,
                actual: tx.nonce,
            });
        }

        // 4. Validasi Saldo (Amount + Fee)
        let total_required = tx
            .amount
            .as_u128()
            .checked_add(tx.fee.as_u128())
            .ok_or(L2ExecutionError::ArithmeticOverflow)?;

        if sender_acc.balance.as_u128() < total_required {
            return Err(L2ExecutionError::InsufficientBalance);
        }

        // 5. Potong Saldo Pengirim & Naikkan Nonce
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
            storage_root: sender_acc.storage_root,
        };
        state.set_account(updated_sender);

        // 6. Tambah Saldo Penerima
        let recipient_acc = state.get_account(&tx.recipient).cloned().unwrap_or(L2Account::new(
            tx.recipient,
            Quantum::ZERO,
            0,
        ));

        let new_recipient_balance = recipient_acc
            .balance
            .as_u128()
            .checked_add(tx.amount.as_u128())
            .ok_or(L2ExecutionError::ArithmeticOverflow)?;

        let updated_recipient = L2Account {
            address: tx.recipient,
            balance: Quantum::new(new_recipient_balance),
            nonce: recipient_acc.nonce,
            storage_root: recipient_acc.storage_root,
        };
        state.set_account(updated_recipient);

        // 7. Terbitkan Struk Eksekusi
        Ok(L2Receipt {
            tx_hash: tx.compute_hash(),
            success: true,
            gas_used: required_gas,
            fee_paid: tx.fee,
        })
    }

    /// Mengeksekusi seluruh transaksi dalam sebuah blok L2 dengan validasi State Transition Function (STF)
    /// Jika terjadi kegagalan transaksi atau ketidaksesuaian root, seluruh perubahan di-rollback secara atomik.
    pub fn execute_block(
        &self,
        state: &mut L2StateStore,
        block: &L2Block,
    ) -> Result<Vec<L2Receipt>, L2ExecutionError> {
        // 1. Validasi Transactions Root
        let expected_txs_root = compute_txs_root(&block.transactions);
        if expected_txs_root != block.header.txs_root {
            return Err(L2ExecutionError::TxsRootMismatch);
        }

        // 2. Ambil snapshot untuk proteksi atomik
        let snapshot = state.checkpoint();
        let mut receipts = Vec::with_capacity(block.transactions.len());

        // 3. Eksekusi seluruh transaksi
        for tx in &block.transactions {
            match self.execute_transaction(state, tx) {
                Ok(receipt) => receipts.push(receipt),
                Err(err) => {
                    // Batalkan seluruh perubahan blok jika salah satu transaksi gagal
                    state.rollback(snapshot);
                    return Err(err);
                }
            }
        }

        // 4. Validasi State Root pasca-eksekusi
        let actual_state_root = state.compute_state_root();
        if actual_state_root != block.header.state_root {
            // Batalkan perubahan state jika hasil eksekusi tidak sesuai dengan komitmen header blok
            state.rollback(snapshot);
            return Err(L2ExecutionError::StateRootMismatch {
                expected: block.header.state_root,
                actual: actual_state_root,
            });
        }

        Ok(receipts)
    }

    /// Mengeksekusi sekumpulan transaksi secara atomik (Batch Execution)
    /// Menjamin semantik all-or-nothing: jika satu transaksi gagal, state kembali utuh ke kondisi semula.
    pub fn execute_batch_atomic(
        &self,
        state: &mut L2StateStore,
        txs: &[L2Transaction],
    ) -> Result<Vec<L2Receipt>, L2ExecutionError> {
        let snapshot = state.checkpoint();
        let mut receipts = Vec::with_capacity(txs.len());

        for tx in txs {
            match self.execute_transaction(state, tx) {
                Ok(receipt) => receipts.push(receipt),
                Err(err) => {
                    state.rollback(snapshot);
                    return Err(err);
                }
            }
        }

        Ok(receipts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Address, Signature};

    #[test]
    fn test_fee_split_calculation() {
        let fee = Quantum::new(100_000); // 0.001 AUR
        let split = calculate_fee_split(fee).expect("Kalkulasi fee split gagal");

        assert_eq!(split.sequencer_fee.as_u128(), 80_000);
        assert_eq!(split.l1_settlement_fee.as_u128(), 20_000);
        assert_eq!(
            split.sequencer_fee.as_u128() + split.l1_settlement_fee.as_u128(),
            fee.as_u128()
        );
    }

    #[test]
    fn test_execute_transaction_success() {
        let mut state = L2StateStore::new();
        let engine = L2ExecutionEngine::new();

        let sender = Address::from_bytes([1u8; 32]);
        let recipient = Address::from_bytes([2u8; 32]);

        state.set_account(L2Account::new(
            sender,
            Quantum::new(1_000_000_000), // 10 AUR
            0,
        ));

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
        assert_eq!(receipt.gas_used, L2_BASE_TRANSFER_GAS);

        let sender_after = state.get_account(&sender).unwrap();
        assert_eq!(sender_after.balance.as_u128(), 599_990_000);
        assert_eq!(sender_after.nonce, 1);

        let recipient_after = state.get_account(&recipient).unwrap();
        assert_eq!(recipient_after.balance.as_u128(), 400_000_000);
    }

    #[test]
    fn test_insufficient_fee_for_gas_rejected() {
        let mut state = L2StateStore::new();
        let engine = L2ExecutionEngine::new();

        let sender = Address::from_bytes([1u8; 32]);
        let recipient = Address::from_bytes([2u8; 32]);

        state.set_account(L2Account::new(sender, Quantum::new(1_000_000_000), 0));

        // Transaksi dengan fee 500 (dibawah batas 10.000)
        let tx = L2Transaction {
            sender,
            recipient,
            amount: Quantum::new(100_000),
            fee: Quantum::new(500),
            nonce: 0,
            signature: Signature::ZERO,
            payload: vec![],
        };

        let err = engine.execute_transaction(&mut state, &tx).unwrap_err();
        assert_eq!(
            err,
            L2ExecutionError::InsufficientFeeForGas {
                required_fee: 10_000,
                provided_fee: 500,
            }
        );
    }

    #[test]
    fn test_insufficient_balance_rejected() {
        let mut state = L2StateStore::new();
        let engine = L2ExecutionEngine::new();

        let sender = Address::from_bytes([1u8; 32]);
        let recipient = Address::from_bytes([2u8; 32]);

        state.set_account(L2Account::new(
            sender,
            Quantum::new(100_000), // Hanya 0.001 AUR
            0,
        ));

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

    #[test]
    fn test_execute_block_success_and_state_transition() {
        let mut state = L2StateStore::new();
        let engine = L2ExecutionEngine::new();

        let sender = Address::from_bytes([1u8; 32]);
        let recipient = Address::from_bytes([2u8; 32]);
        state.set_account(L2Account::new(sender, Quantum::new(1_000_000_000), 0));

        let tx = L2Transaction {
            sender,
            recipient,
            amount: Quantum::new(250_000_000),
            fee: Quantum::new(10_000),
            nonce: 0,
            signature: Signature::ZERO,
            payload: vec![],
        };

        // Simulasikan state sesudah tx dieksekusi untuk mendapatkan root yang diharapkan
        let mut sim_state = state.clone();
        engine.execute_transaction(&mut sim_state, &tx).unwrap();
        let expected_state_root = sim_state.compute_state_root();

        let block = L2Block::new(
            1,
            Hash256::ZERO,
            expected_state_root,
            1700000000,
            vec![tx],
        );

        let receipts = engine.execute_block(&mut state, &block).expect("Eksekusi blok gagal");
        assert_eq!(receipts.len(), 1);
        assert_eq!(state.compute_state_root(), expected_state_root);
    }

    #[test]
    fn test_execute_block_state_root_mismatch_rollback() {
        let mut state = L2StateStore::new();
        let engine = L2ExecutionEngine::new();

        let sender = Address::from_bytes([1u8; 32]);
        let recipient = Address::from_bytes([2u8; 32]);
        state.set_account(L2Account::new(sender, Quantum::new(1_000_000_000), 0));

        let tx = L2Transaction {
            sender,
            recipient,
            amount: Quantum::new(250_000_000),
            fee: Quantum::new(10_000),
            nonce: 0,
            signature: Signature::ZERO,
            payload: vec![],
        };

        // Header blok memuat state root palsu
        let fake_state_root = Hash256::from_bytes([0xEE; 32]);
        let block = L2Block::new(
            1,
            Hash256::ZERO,
            fake_state_root,
            1700000000,
            vec![tx],
        );

        let err = engine.execute_block(&mut state, &block).unwrap_err();
        assert!(matches!(err, L2ExecutionError::StateRootMismatch { .. }));

        // Buktikan saldo pengirim tidak berubah karena di-rollback
        assert_eq!(state.get_account(&sender).unwrap().balance.as_u128(), 1_000_000_000);
        assert_eq!(state.get_account(&sender).unwrap().nonce, 0);
    }

    #[test]
    fn test_atomic_batch_rollback_on_failure() {
        let mut state = L2StateStore::new();
        let engine = L2ExecutionEngine::new();

        let sender = Address::from_bytes([1u8; 32]);
        let recipient = Address::from_bytes([2u8; 32]);
        state.set_account(L2Account::new(sender, Quantum::new(1_000_000_000), 0));

        // Tx 1: valid
        let tx1 = L2Transaction {
            sender,
            recipient,
            amount: Quantum::new(100_000_000),
            fee: Quantum::new(10_000),
            nonce: 0,
            signature: Signature::ZERO,
            payload: vec![],
        };

        // Tx 2: invalid (saldo tidak cukup untuk 2 miliar)
        let tx2 = L2Transaction {
            sender,
            recipient,
            amount: Quantum::new(2_000_000_000),
            fee: Quantum::new(10_000),
            nonce: 1,
            signature: Signature::ZERO,
            payload: vec![],
        };

        let err = engine.execute_batch_atomic(&mut state, &[tx1, tx2]).unwrap_err();
        assert_eq!(err, L2ExecutionError::InsufficientBalance);

        // Pastikan Tx 1 juga dibatalkan (rollback atomik)
        assert_eq!(state.get_account(&sender).unwrap().balance.as_u128(), 1_000_000_000);
        assert_eq!(state.get_account(&sender).unwrap().nonce, 0);
        assert!(state.get_account(&recipient).is_none());
    }
}
