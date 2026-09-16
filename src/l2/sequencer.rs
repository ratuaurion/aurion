//! Modul Sequencer dan Batch Assembler Layer-2 Aurion.
//! Menangani antrean transaksi mempool L2, perakitan batch, dan soft finality.

use std::collections::VecDeque;
use crate::core::Hash256;
use crate::l2::state::L2StateStore;
use crate::l2::types::{L2Batch, L2Block, L2BlockHeader, L2Transaction};
use crate::l2::vm::{L2ExecutionEngine, L2ExecutionError};

/// Antrean Transaksi Mempool L2
#[derive(Debug, Default, Clone)]
pub struct L2Mempool {
    queue: VecDeque<L2Transaction>,
}

impl L2Mempool {
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    /// Memasukkan transaksi ke mempool
    pub fn push(&mut self, tx: L2Transaction) {
        self.queue.push_back(tx);
    }

    /// Mengambil hingga N transaksi untuk dirakit ke dalam blok
    pub fn drain_batch(&mut self, max_count: usize) -> Vec<L2Transaction> {
        let count = max_count.min(self.queue.len());
        self.queue.drain(..count).collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// Daemon Sequencer L2
#[derive(Debug, Default)]
pub struct L2Sequencer {
    pub mempool: L2Mempool,
    pub state: L2StateStore,
    pub engine: L2ExecutionEngine,
    pub current_block: u64,
    pub last_block_hash: Hash256,
}

impl L2Sequencer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Menerima transaksi dari klien
    pub fn submit_transaction(&mut self, tx: L2Transaction) {
        self.mempool.push(tx);
    }

    /// Memproduksi blok L2 baru dari mempool
    pub fn produce_block(&mut self, max_txs: usize) -> Result<Option<L2Block>, L2ExecutionError> {
        if self.mempool.is_empty() {
            return Ok(None);
        }

        let pending_txs = self.mempool.drain_batch(max_txs);
        let mut executed_txs = Vec::with_capacity(pending_txs.len());

        for tx in pending_txs {
            // Eksekusi transaksi di state L2
            self.engine.execute_transaction(&mut self.state, &tx)?;
            executed_txs.push(tx);
        }

        self.current_block += 1;
        let new_state_root = self.state.compute_state_root();

        let header = L2BlockHeader {
            block_number: self.current_block,
            prev_hash: self.last_block_hash,
            state_root: new_state_root,
            txs_root: Hash256::ZERO,
            timestamp: 1700000000 + self.current_block,
        };

        let block_hash = header.compute_hash();
        self.last_block_hash = block_hash;

        Ok(Some(L2Block {
            header,
            transactions: executed_txs,
        }))
    }

    /// Merakit sekumpulan blok L2 menjadi satu paket batch rollup
    #[must_use]
    pub fn assemble_batch(
        &self,
        batch_index: u64,
        prev_state_root: Hash256,
        new_state_root: Hash256,
        start_block: u64,
        end_block: u64,
        calldata: Vec<u8>,
    ) -> L2Batch {
        L2Batch {
            batch_index,
            prev_state_root,
            new_state_root,
            start_block,
            end_block,
            transactions_calldata: calldata,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Address, Quantum, Signature};
    use crate::l2::state::L2Account;

    #[test]
    fn test_sequencer_block_production_lifecycle() {
        let mut sequencer = L2Sequencer::new();

        let sender = Address::from_bytes([1u8; 32]);
        let recipient = Address::from_bytes([2u8; 32]);

        sequencer.state.set_account(L2Account {
            address: sender,
            balance: Quantum::new(500_000_000),
            nonce: 0,
        });

        let tx = L2Transaction {
            sender,
            recipient,
            amount: Quantum::new(200_000_000),
            fee: Quantum::new(50_000),
            nonce: 0,
            signature: Signature::from_bytes([0u8; 64]),
            payload: vec![],
        };

        sequencer.submit_transaction(tx);
        assert_eq!(sequencer.mempool.len(), 1);

        let block_opt = sequencer.produce_block(10).unwrap();
        assert!(block_opt.is_some());
        let block = block_opt.unwrap();

        assert_eq!(block.header.block_number, 1);
        assert_eq!(block.transactions.len(), 1);
        assert!(sequencer.mempool.is_empty());
    }
}
