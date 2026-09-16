//! Tipe Data & Status Siklus Transaksi Mempool Aurion.
//! Mematuhi Dokumen 03 (03-TRANSACTION-LIFECYCLE.md).

use crate::core::{Address, Hash256, MonetaryError, Quantum};
use crate::state::monetary::MonetaryState;
use crate::transaction::types::Transaction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    Created,
    Signed,
    Submitted,
    Mempool,
    Included,
    Finalized,
    Rejected,
    Dropped,
    Expired,
    Replaced,
}

/// Objek pembungkus transaksi di dalam mempool internal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MempoolEntry {
    pub tx: Transaction,
    pub tx_id: Hash256,
    pub state: TransactionState,
    pub admitted_timestamp: u64,
}

impl MempoolEntry {
    pub fn new(tx: Transaction, admitted_timestamp: u64) -> Self {
        let tx_id = tx.compute_tx_id();
        Self {
            tx,
            tx_id,
            state: TransactionState::Mempool,
            admitted_timestamp,
        }
    }
}

/// Tanda terima transaksi kanonikal (Receipt) untuk verifikasi akuntansi & audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionReceipt {
    pub tx_id: Hash256,
    pub block_height: u64,
    pub block_hash: Hash256,
    pub transaction_index: u32,
    pub sender: Address,
    pub recipient: Address,
    pub amount_quanta: Quantum,
    pub fee_quanta: Quantum,
    pub fee_burned_quanta: Quantum,
    pub fee_miner_quanta: Quantum,
    pub nonce: u64,
    pub status: TransactionState,
}

impl TransactionReceipt {
    pub fn from_finalized_tx(
        tx: &Transaction,
        block_height: u64,
        block_hash: Hash256,
        transaction_index: u32,
    ) -> Result<Self, MonetaryError> {
        let fee = tx.fee;
        let (fee_burned, fee_miner) = MonetaryState::split_fee(fee)?;

        Ok(Self {
            tx_id: tx.compute_tx_id(),
            block_height,
            block_hash,
            transaction_index,
            sender: tx.sender,
            recipient: tx.recipient,
            amount_quanta: tx.amount,
            fee_quanta: fee,
            fee_burned_quanta: fee_burned,
            fee_miner_quanta: fee_miner,
            nonce: tx.nonce,
            status: TransactionState::Finalized,
        })
    }
}

