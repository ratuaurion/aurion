//! Tipe Data Primitif Layer-2 (L2) Aurion.
//! Mematuhi Invariant L2-ARCH-003 (Zero-Float Quantum u128) dan L2-ARCH-005 (Blake3/Ed25519).

use crate::core::{Address, Hash256, Quantum, Signature};
use crate::crypto::blake3_hash;

/// Identitas Transaksi L2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2Transaction {
    pub sender: Address,
    pub recipient: Address,
    pub amount: Quantum,
    pub fee: Quantum,
    pub nonce: u64,
    pub signature: Signature,
    pub payload: Vec<u8>,
}

impl L2Transaction {
    /// Menghitung hash unik transaksi L2 berbasis Blake3
    #[must_use]
    pub fn compute_hash(&self) -> Hash256 {
        let mut data = Vec::with_capacity(128 + self.payload.len());
        data.extend_from_slice(self.sender.as_bytes());
        data.extend_from_slice(self.recipient.as_bytes());
        data.extend_from_slice(&self.amount.as_u128().to_be_bytes());
        data.extend_from_slice(&self.fee.as_u128().to_be_bytes());
        data.extend_from_slice(&self.nonce.to_be_bytes());
        data.extend_from_slice(self.signature.as_bytes());
        data.extend_from_slice(&self.payload);
        blake3_hash(&data)
    }
}

/// Header Blok Layer-2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2BlockHeader {
    pub block_number: u64,
    pub prev_hash: Hash256,
    pub state_root: Hash256,
    pub txs_root: Hash256,
    pub timestamp: u64,
}

impl L2BlockHeader {
    /// Menghitung hash blok L2 berbasis Blake3
    #[must_use]
    pub fn compute_hash(&self) -> Hash256 {
        let mut data = Vec::with_capacity(8 + 32 + 32 + 32 + 8);
        data.extend_from_slice(&self.block_number.to_be_bytes());
        data.extend_from_slice(self.prev_hash.as_bytes());
        data.extend_from_slice(self.state_root.as_bytes());
        data.extend_from_slice(self.txs_root.as_bytes());
        data.extend_from_slice(&self.timestamp.to_be_bytes());
        blake3_hash(&data)
    }
}

/// Blok Penuh Layer-2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2Block {
    pub header: L2BlockHeader,
    pub transactions: Vec<L2Transaction>,
}

/// Paket Batch Rollup L2 untuk Diposting ke Layer-1
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2Batch {
    pub batch_index: u64,
    pub prev_state_root: Hash256,
    pub new_state_root: Hash256,
    pub start_block: u64,
    pub end_block: u64,
    pub transactions_calldata: Vec<u8>,
}

impl L2Batch {
    /// Menghitung komitmen hash batch untuk verifikasi di L1
    #[must_use]
    pub fn compute_batch_hash(&self) -> Hash256 {
        let mut data = Vec::with_capacity(8 + 32 + 32 + 8 + 8 + self.transactions_calldata.len());
        data.extend_from_slice(&self.batch_index.to_be_bytes());
        data.extend_from_slice(self.prev_state_root.as_bytes());
        data.extend_from_slice(self.new_state_root.as_bytes());
        data.extend_from_slice(&self.start_block.to_be_bytes());
        data.extend_from_slice(&self.end_block.to_be_bytes());
        data.extend_from_slice(&self.transactions_calldata);
        blake3_hash(&data)
    }
}

/// Bukti Eksekusi Transaksi L2 (Receipt)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2Receipt {
    pub tx_hash: Hash256,
    pub success: bool,
    pub gas_used: u64,
    pub fee_paid: Quantum,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2_transaction_hash_deterministic() {
        let sender = Address::from_bytes([1u8; 32]);
        let recipient = Address::from_bytes([2u8; 32]);
        let tx = L2Transaction {
            sender,
            recipient,
            amount: Quantum::new(500_000_000), // 5 AUR
            fee: Quantum::new(100_000),        // 0.001 AUR
            nonce: 1,
            signature: Signature::from_bytes([3u8; 64]),
            payload: vec![0xde, 0xad, 0xbe, 0xef],
        };

        let h1 = tx.compute_hash();
        let h2 = tx.compute_hash();
        assert_eq!(h1, h2);
        assert_ne!(h1, Hash256::ZERO);
    }

    #[test]
    fn test_l2_block_header_hash() {
        let header = L2BlockHeader {
            block_number: 100,
            prev_hash: Hash256::ZERO,
            state_root: Hash256::from_bytes([4u8; 32]),
            txs_root: Hash256::from_bytes([5u8; 32]),
            timestamp: 1700000000,
        };
        let hash = header.compute_hash();
        assert_ne!(hash, Hash256::ZERO);
    }

    #[test]
    fn test_l2_batch_hash() {
        let batch = L2Batch {
            batch_index: 1,
            prev_state_root: Hash256::ZERO,
            new_state_root: Hash256::from_bytes([7u8; 32]),
            start_block: 1,
            end_block: 10,
            transactions_calldata: vec![1, 2, 3, 4],
        };
        assert_ne!(batch.compute_batch_hash(), Hash256::ZERO);
    }
}
