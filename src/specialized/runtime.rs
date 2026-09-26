//! Mesin Eksekusi Runtime Layer-3 Specialized Networks (L3 STF & Gas Metering).
//! Menjalankan State Transition Function (STF) L3, Gas Metering Khusus Domain,
//! Atomic Transaction Snapshot & Rollback, serta Isolasi Total Fault Domain.
//! Mematuhi Invariant AUR-ARCH-011 (#![forbid(unsafe_code)]), AUR-ARCH-012 (Zero-Float Quantum u128),
//! AUR-L3-ARCH-002 (Domain Modular STF), dan AUR-L3-SEC-001 (Domain Fault Isolation).

use crate::core::{Hash256, Quantum};
use crate::specialized::state::L3State;
use crate::specialized::types::{DomainId, L3Block, L3Receipt, L3Transaction};
use thiserror::Error;

/// Biaya gas dasar untuk eksekusi transaksi di domain L3 (5.000 gas, ultra-ringan)
pub const L3_BASE_TX_GAS: u64 = 5_000;

/// Biaya gas tambahan per byte payload domain L3 (2 gas per byte)
pub const L3_GAS_PER_PAYLOAD_BYTE: u64 = 2;

/// Harga gas minimum dalam satuan Quanta (1 Quanta per unit gas)
pub const L3_MIN_GAS_PRICE_QUANTA: u128 = 1;

/// Persentase pembagian fee untuk Sequencer/Operator Domain L3 (80%)
pub const L3_FEE_SPLIT_SEQUENCER_PERCENT: u128 = 80;

/// Persentase pembagian fee untuk Cadangan Settlement L2/L1 (20%)
pub const L3_FEE_SPLIT_SETTLEMENT_PERCENT: u128 = 20;

/// Batas maksimum gas default per blok L3
pub const L3_DEFAULT_MAX_GAS_PER_BLOCK: u64 = 10_000_000;

/// Kesalahan Eksekusi Runtime Domain L3
#[derive(Debug, Error, PartialEq, Eq)]
pub enum L3ExecutionError {
    #[error("Domain ID tidak cocok: diharapkan {expected}, aktual {actual}")]
    DomainMismatch {
        expected: DomainId,
        actual: DomainId,
    },

    #[error("Pengirim akun L3 tidak ditemukan atau saldo tidak terdaftar")]
    SenderNotFound,

    #[error("Saldo akun L3 tidak mencukupi untuk transfer dan gas fee (tersedia {available} quanta, dibutuhkan {required} quanta)")]
    InsufficientBalance { available: u128, required: u128 },

    #[error("Nonce L3 tidak valid: diharapkan {expected}, aktual {actual}")]
    InvalidNonce { expected: u64, actual: u64 },

    #[error("Fee transaksi tidak mencukupi gas: disediakan {provided} quanta, dibutuhkan minimal {required} quanta")]
    InsufficientFeeForGas { provided: u128, required: u128 },

    #[error("Akumulasi gas blok melebihi batas maksimum: gas blok {current_gas}, batas {max_gas}")]
    GasLimitExceeded { current_gas: u64, max_gas: u64 },

    #[error("Ketidaksesuaian State Root blok L3: diharapkan {expected}, aktual {actual}")]
    StateRootMismatch { expected: Hash256, actual: Hash256 },

    #[error("Ketidaksesuaian Transactions Root blok L3")]
    TransactionsRootMismatch,

    #[error("Ketidaksesuaian Hash Blok Sebelumnya: diharapkan {expected}, aktual {actual}")]
    PreviousBlockMismatch { expected: Hash256, actual: Hash256 },

    #[error("Terjadi overflow atau underflow pada perhitungan integer Quantum")]
    ArithmeticOverflow,

    #[error("Eksekusi instruksi domain L3 dibatalkan (reverted): {0}")]
    InstructionReverted(String),
}

/// Konfigurasi Eksekusi Runtime Domain L3
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L3ExecutionConfig {
    pub domain_id: DomainId,
    pub base_tx_gas: u64,
    pub gas_per_payload_byte: u64,
    pub min_gas_price: Quantum,
    pub sequencer_fee_pct: u128,
    pub settlement_reserve_pct: u128,
    pub max_gas_per_block: u64,
}

impl L3ExecutionConfig {
    /// Membuat konfigurasi default untuk domain tertentu
    #[must_use]
    pub fn default_for(domain_id: DomainId) -> Self {
        Self {
            domain_id,
            base_tx_gas: L3_BASE_TX_GAS,
            gas_per_payload_byte: L3_GAS_PER_PAYLOAD_BYTE,
            min_gas_price: Quantum::new(L3_MIN_GAS_PRICE_QUANTA),
            sequencer_fee_pct: L3_FEE_SPLIT_SEQUENCER_PERCENT,
            settlement_reserve_pct: L3_FEE_SPLIT_SETTLEMENT_PERCENT,
            max_gas_per_block: L3_DEFAULT_MAX_GAS_PER_BLOCK,
        }
    }
}

/// Rincian Pembagian Fee Transaksi L3 (Zero-Float)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct L3FeeSplit {
    pub sequencer_fee: Quantum,
    pub settlement_reserve_fee: Quantum,
}

/// Menghitung pembagian fee transaksi (80% Sequencer, 20% Settlement Reserve)
pub fn calculate_l3_fee_split(total_fee: Quantum) -> Result<L3FeeSplit, L3ExecutionError> {
    let total = total_fee.as_u128();
    let sequencer_quanta = total
        .checked_mul(L3_FEE_SPLIT_SEQUENCER_PERCENT)
        .ok_or(L3ExecutionError::ArithmeticOverflow)?
        / 100;
    let settlement_quanta = total.saturating_sub(sequencer_quanta);

    Ok(L3FeeSplit {
        sequencer_fee: Quantum::new(sequencer_quanta),
        settlement_reserve_fee: Quantum::new(settlement_quanta),
    })
}

/// Menghitung kuota gas yang dibutuhkan untuk mengeksekusi transaksi
#[must_use]
pub fn calculate_l3_required_gas(config: &L3ExecutionConfig, payload_len: usize) -> u64 {
    let payload_gas = (payload_len as u64).saturating_mul(config.gas_per_payload_byte);
    config.base_tx_gas.saturating_add(payload_gas)
}

/// Runtime Engine Eksekusi Domain Terspesialisasi L3
#[derive(Debug, Clone)]
pub struct L3ExecutionEngine {
    pub config: L3ExecutionConfig,
}

impl L3ExecutionEngine {
    /// Membuat instance baru engine eksekusi domain L3
    #[must_use]
    pub fn new(config: L3ExecutionConfig) -> Self {
        Self { config }
    }

    /// Mengeksekusi satu transaksi L3 dan memutasi state secara atomik (AUR-L3-SEC-001)
    pub fn execute_transaction(
        &self,
        state: &mut L3State,
        tx: &L3Transaction,
    ) -> Result<L3Receipt, L3ExecutionError> {
        // 1. Verifikasi Kecocokan Domain Eksekusi
        if tx.domain_id != self.config.domain_id {
            return Err(L3ExecutionError::DomainMismatch {
                expected: self.config.domain_id,
                actual: tx.domain_id,
            });
        }

        // 2. Verifikasi Gas Metering & Kecukupan Fee (Zero-Float)
        let gas_needed = calculate_l3_required_gas(&self.config, tx.payload.len());
        let min_required_fee_quanta = (gas_needed as u128)
            .checked_mul(self.config.min_gas_price.as_u128())
            .ok_or(L3ExecutionError::ArithmeticOverflow)?;

        if tx.fee.as_u128() < min_required_fee_quanta {
            return Err(L3ExecutionError::InsufficientFeeForGas {
                provided: tx.fee.as_u128(),
                required: min_required_fee_quanta,
            });
        }

        // 3. Verifikasi Nonce Akun Pengirim
        let current_nonce = state.get_nonce(&tx.sender);
        if tx.nonce != current_nonce {
            return Err(L3ExecutionError::InvalidNonce {
                expected: current_nonce,
                actual: tx.nonce,
            });
        }

        // 4. Verifikasi Kecukupan Saldo Pengirim (Amount + Fee)
        let sender_balance = state.get_balance(&tx.sender);
        let total_cost = tx
            .amount
            .checked_add(tx.fee)
            .map_err(|_| L3ExecutionError::ArithmeticOverflow)?;

        if sender_balance < total_cost {
            return Err(L3ExecutionError::InsufficientBalance {
                available: sender_balance.as_u128(),
                required: total_cost.as_u128(),
            });
        }

        // 5. Eksekusi Atomik State Transition dengan Snapshot Guard (AUR-L3-SEC-001)
        let snapshot_id = state.snapshot();

        // Debit total cost dari pengirim
        if let Err(e) = state.debit(&tx.sender, total_cost) {
            let _ = state.revert_to_snapshot(snapshot_id);
            return Err(L3ExecutionError::InstructionReverted(e.to_string()));
        }

        // Credit amount ke penerima
        if let Err(e) = state.credit(&tx.recipient, tx.amount) {
            let _ = state.revert_to_snapshot(snapshot_id);
            return Err(L3ExecutionError::InstructionReverted(e.to_string()));
        }

        // Naikkan nonce pengirim
        if let Err(e) = state.increment_nonce(&tx.sender) {
            let _ = state.revert_to_snapshot(snapshot_id);
            return Err(L3ExecutionError::InstructionReverted(e.to_string()));
        }

        // Commit snapshot karena mutasi berhasil tanpa pelanggaran invariant
        let _ = state.commit_snapshot(snapshot_id);

        let tx_id = tx.compute_hash();
        let receipt = L3Receipt::new(
            tx_id,
            true,
            gas_needed,
            tx.fee,
            b"L3_TX_SUCCESS".to_vec(),
            vec![b"EVENT_L3_TRANSFER_EXECUTED".to_vec()],
        );

        Ok(receipt)
    }

    /// Mengeksekusi satu blok penuh transaksi L3 secara atomik
    pub fn execute_block(
        &self,
        state: &mut L3State,
        block: &L3Block,
    ) -> Result<Vec<L3Receipt>, L3ExecutionError> {
        // 1. Verifikasi Kecocokan Domain Blok
        if block.domain_id != self.config.domain_id {
            return Err(L3ExecutionError::DomainMismatch {
                expected: self.config.domain_id,
                actual: block.domain_id,
            });
        }

        // 2. Verifikasi Pohon Transaksi Blok
        let computed_txs_root = L3Block::compute_transactions_root(&block.transactions);
        if block.transactions_root != Hash256::ZERO && block.transactions_root != computed_txs_root
        {
            return Err(L3ExecutionError::TransactionsRootMismatch);
        }

        // 3. Buat Snapshot Tingkat Blok untuk Proteksi Atomik Penuh (All-or-Nothing)
        let block_snapshot = state.snapshot();
        let mut receipts = Vec::with_capacity(block.transactions.len());
        let mut total_block_gas: u64 = 0;

        for tx in &block.transactions {
            match self.execute_transaction(state, tx) {
                Ok(receipt) => {
                    total_block_gas = total_block_gas.saturating_add(receipt.gas_used);
                    if total_block_gas > self.config.max_gas_per_block {
                        let _ = state.revert_to_snapshot(block_snapshot);
                        return Err(L3ExecutionError::GasLimitExceeded {
                            current_gas: total_block_gas,
                            max_gas: self.config.max_gas_per_block,
                        });
                    }
                    receipts.push(receipt);
                }
                Err(err) => {
                    // Batalkan seluruh mutasi blok jika terjadi kegagalan transaksi
                    let _ = state.revert_to_snapshot(block_snapshot);
                    return Err(err);
                }
            }
        }

        // 4. Verifikasi State Root Pasca Eksekusi
        let actual_state_root = state.compute_state_root();
        if block.state_root != Hash256::ZERO && block.state_root != actual_state_root {
            let _ = state.revert_to_snapshot(block_snapshot);
            return Err(L3ExecutionError::StateRootMismatch {
                expected: block.state_root,
                actual: actual_state_root,
            });
        }

        // Commit snapshot blok dan tingkatkan nomor blok
        let _ = state.commit_snapshot(block_snapshot);
        state.block_number = block.block_number;

        Ok(receipts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Address, Signature};

    #[test]
    fn test_l3_execution_transaction_success_and_balance_mutation() {
        let domain_id = DomainId::APP_CHAIN_DEFAULT;
        let config = L3ExecutionConfig::default_for(domain_id);
        let engine = L3ExecutionEngine::new(config);

        let mut state = L3State::new(domain_id);
        let alice = Address::from_bytes([0x11; 32]);
        let bob = Address::from_bytes([0x22; 32]);

        // Berikan saldo awal ke Alice
        state.credit(&alice, Quantum::new(1_000_000)).unwrap();

        let tx = L3Transaction::new(
            domain_id,
            alice,
            bob,
            Quantum::new(400_000),
            Quantum::new(10_000),
            0,
            Signature::from_bytes([0xaa; 64]),
            vec![1, 2, 3],
        );

        let receipt = engine
            .execute_transaction(&mut state, &tx)
            .expect("Eksekusi L3 harus berhasil");
        assert!(receipt.success);
        assert_eq!(receipt.fee_paid, Quantum::new(10_000));

        // Verifikasi saldo pasca mutasi
        assert_eq!(state.get_balance(&alice), Quantum::new(590_000));
        assert_eq!(state.get_balance(&bob), Quantum::new(400_000));
        assert_eq!(state.get_nonce(&alice), 1);
    }

    #[test]
    fn test_l3_execution_rejection_insufficient_balance() {
        let domain_id = DomainId::DEX_DEFAULT;
        let engine = L3ExecutionEngine::new(L3ExecutionConfig::default_for(domain_id));
        let mut state = L3State::new(domain_id);

        let alice = Address::from_bytes([0x11; 32]);
        let bob = Address::from_bytes([0x22; 32]);
        state.credit(&alice, Quantum::new(50_000)).unwrap();

        let tx = L3Transaction::new(
            domain_id,
            alice,
            bob,
            Quantum::new(100_000), // Melebihi saldo
            Quantum::new(10_000),
            0,
            Signature::from_bytes([0xaa; 64]),
            vec![],
        );

        let result = engine.execute_transaction(&mut state, &tx);
        assert!(matches!(
            result,
            Err(L3ExecutionError::InsufficientBalance { .. })
        ));
        assert_eq!(state.get_balance(&alice), Quantum::new(50_000)); // Saldo utuh
    }

    #[test]
    fn test_l3_execution_rejection_domain_mismatch() {
        let dex_domain = DomainId::DEX_DEFAULT;
        let gaming_domain = DomainId::GAMING_DEFAULT;
        let engine = L3ExecutionEngine::new(L3ExecutionConfig::default_for(dex_domain));
        let mut state = L3State::new(dex_domain);

        let alice = Address::from_bytes([0x11; 32]);
        let bob = Address::from_bytes([0x22; 32]);
        state.credit(&alice, Quantum::new(500_000)).unwrap();

        let tx = L3Transaction::new(
            gaming_domain, // Domain tidak cocok
            alice,
            bob,
            Quantum::new(100_000),
            Quantum::new(10_000),
            0,
            Signature::from_bytes([0xaa; 64]),
            vec![],
        );

        let result = engine.execute_transaction(&mut state, &tx);
        assert!(matches!(
            result,
            Err(L3ExecutionError::DomainMismatch { .. })
        ));
    }

    #[test]
    fn test_l3_execute_block_atomic_rollback_on_failure() {
        let domain_id = DomainId::PRIVACY_DEFAULT;
        let engine = L3ExecutionEngine::new(L3ExecutionConfig::default_for(domain_id));
        let mut state = L3State::new(domain_id);

        let alice = Address::from_bytes([0x11; 32]);
        let bob = Address::from_bytes([0x22; 32]);
        state.credit(&alice, Quantum::new(300_000)).unwrap();

        let initial_root = state.compute_state_root();

        // Tx1 valid
        let tx1 = L3Transaction::new(
            domain_id,
            alice,
            bob,
            Quantum::new(100_000),
            Quantum::new(10_000),
            0,
            Signature::from_bytes([0xaa; 64]),
            vec![],
        );

        // Tx2 gagal (nonce melompat)
        let tx2 = L3Transaction::new(
            domain_id,
            alice,
            bob,
            Quantum::new(50_000),
            Quantum::new(10_000),
            99, // Nonce salah
            Signature::from_bytes([0xbb; 64]),
            vec![],
        );

        let block = L3Block::new(
            domain_id,
            1,
            Hash256::ZERO,
            Hash256::ZERO,
            Hash256::ZERO,
            Hash256::ZERO,
            1_700_000_000,
            vec![tx1, tx2],
        );

        let result = engine.execute_block(&mut state, &block);
        assert!(matches!(result, Err(L3ExecutionError::InvalidNonce { .. })));

        // Verifikasi rollback penuh: Tx1 tidak boleh tersisa di state
        assert_eq!(state.get_balance(&alice), Quantum::new(300_000));
        assert_eq!(state.get_balance(&bob), Quantum::ZERO);
        assert_eq!(state.get_nonce(&alice), 0);
        assert_eq!(state.compute_state_root(), initial_root);
    }
}
