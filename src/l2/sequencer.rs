//! Modul Sequencer dan Batch Assembler Layer-2 Aurion.
//! Menangani antrean transaksi mempool L2, perakitan batch, dan soft finality.
//! Mematuhi Invariant:
//! - L2-ARCH-001 (Monolithic Execution)
//! - L2-ARCH-003 (Zero-Float Quantum)
//! - L2-DA-001..002 (Kanonikalitas Calldata L1 & Blake3 DA Commitment)
//! - L2-LIFE-001..002 (Sequencer Lifecycle & Soft Finality <50ms)
//! - AUR-ARCH-011 (#![forbid(unsafe_code)])

use thiserror::Error;
use crate::core::{Address, Hash256};
use crate::l2::codec::{L2BatchFrame, L2CodecError};
use crate::l2::state::L2StateStore;
use crate::l2::types::{
    compute_txs_root, L2Batch, L2Block, L2BlockHeader, L2Transaction, L2_TX_BASE_SIZE,
};
use crate::l2::vm::{L2ExecutionEngine, L2ExecutionError};

/// Kapasitas maksimum transaksi dalam antrean mempool L2 (Anti-DoS)
pub const MAX_L2_MEMPOOL_CAPACITY: usize = 10_000;

/// Jumlah transaksi maksimum bawaan per blok L2
pub const DEFAULT_MAX_TXS_PER_BLOCK: usize = 250;

/// Jumlah blok bawaan dalam satu paket batch calldata
pub const DEFAULT_BLOCKS_PER_BATCH: u64 = 10;

/// Kesalahan pada Operasi Sequencer L2
#[derive(Debug, Error, PartialEq, Eq)]
pub enum L2SequencerError {
    #[error("Kapasitas antrean mempool L2 penuh ({0} transaksi)")]
    MempoolFull(usize),

    #[error("Tanda tangan transaksi L2 tidak valid")]
    InvalidSignature,

    #[error("Pengirim akun L2 {0} tidak ditemukan dalam state")]
    SenderNotFound(Address),

    #[error("Nonce transaksi terlalu rendah (diharapkan minimal {expected}, diterima {actual})")]
    NonceTooLow { expected: u64, actual: u64 },

    #[error("Transaksi dengan nonce {nonce} untuk pengirim {sender} sudah ada dalam mempool")]
    DuplicateNonce { sender: Address, nonce: u64 },

    #[error("Saldo akun L2 pengirim tidak mencukupi untuk transfer dan fee")]
    InsufficientBalance,

    #[error("Eksekusi transaksi atau blok L2 gagal: {0}")]
    ExecutionFailed(#[from] L2ExecutionError),

    #[error("Kesalahan pengemasan payload calldata biner: {0}")]
    CodecError(#[from] L2CodecError),

    #[error("Tidak ada blok baru yang tersedia untuk dirakit ke dalam batch")]
    NoBlocksForBatch,
}

/// Tanda Pengesahan Finalitas Lunak (Soft Finality Attestation) oleh Sequencer L2 (<50ms)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2SoftFinalityReceipt {
    pub block_number: u64,
    pub block_hash: Hash256,
    pub state_root: Hash256,
    pub txs_root: Hash256,
    pub tx_count: usize,
    pub timestamp: u64,
}

/// Mengemas transaksi L2 ke calldata biner sekuensial (Raw Canonical)
#[must_use]
pub fn pack_batch_transactions(txs: &[L2Transaction]) -> Vec<u8> {
    let mut payload = Vec::new();
    for tx in txs {
        payload.extend_from_slice(&tx.encode_canonical());
    }
    payload
}

/// Membaca kembali kumpulan transaksi L2 dari calldata payload
pub fn unpack_batch_transactions(mut payload: &[u8]) -> Result<Vec<L2Transaction>, L2CodecError> {
    let mut txs = Vec::new();
    while !payload.is_empty() {
        if payload.len() < L2_TX_BASE_SIZE {
            return Err(L2CodecError::HeaderTooShort {
                expected: L2_TX_BASE_SIZE,
                actual: payload.len(),
            });
        }
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&payload[168..172]);
        let p_len = u32::from_be_bytes(len_bytes) as usize;
        let total_tx_len = L2_TX_BASE_SIZE + p_len;
        if payload.len() < total_tx_len {
            return Err(L2CodecError::PayloadLengthMismatch {
                expected: total_tx_len,
                actual: payload.len(),
            });
        }
        let tx = L2Transaction::decode_canonical(&payload[..total_tx_len])?;
        txs.push(tx);
        payload = &payload[total_tx_len..];
    }
    Ok(txs)
}

/// Antrean Transaksi Mempool L2 dengan Pencegahan DoS dan Prioritisasi Fee
#[derive(Debug, Clone)]
pub struct L2Mempool {
    queue: Vec<L2Transaction>,
    max_capacity: usize,
}

impl Default for L2Mempool {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            max_capacity: MAX_L2_MEMPOOL_CAPACITY,
        }
    }
}

impl L2Mempool {
    /// Membuat mempool L2 baru dengan kapasitas default
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Membuat mempool L2 dengan batas kapasitas tertentu
    #[must_use]
    pub fn with_capacity(max_capacity: usize) -> Self {
        Self {
            queue: Vec::new(),
            max_capacity,
        }
    }

    /// Memasukkan transaksi ke mempool dengan verifikasi kapasitas & anti-duplikasi nonce
    pub fn insert(&mut self, tx: L2Transaction) -> Result<(), L2SequencerError> {
        if self.queue.len() >= self.max_capacity {
            return Err(L2SequencerError::MempoolFull(self.max_capacity));
        }

        if self.queue.iter().any(|t| t.sender == tx.sender && t.nonce == tx.nonce) {
            return Err(L2SequencerError::DuplicateNonce {
                sender: tx.sender,
                nonce: tx.nonce,
            });
        }

        self.queue.push(tx);
        Ok(())
    }

    /// Mengambil hingga `max_count` transaksi teratas yang diurutkan berdasarkan fee tertinggi
    pub fn drain_prioritized(&mut self, max_count: usize) -> Vec<L2Transaction> {
        if self.queue.is_empty() {
            return Vec::new();
        }

        // Urutkan prioritas: transaksi bersaldo fee tertinggi dieksekusi lebih dulu.
        // Jika pengirim sama, pertahankan urutan nonce ascending agar transisi state valid.
        self.queue.sort_by(|a, b| {
            if a.sender == b.sender {
                a.nonce.cmp(&b.nonce)
            } else {
                b.fee.as_u128().cmp(&a.fee.as_u128())
            }
        });

        let count = max_count.min(self.queue.len());
        self.queue.drain(..count).collect()
    }

    /// Mengambil seluruh transaksi yang sesuai tanpa mengosongkan antrean
    #[must_use]
    pub fn as_slice(&self) -> &[L2Transaction] {
        &self.queue
    }

    /// Jumlah transaksi aktif dalam antrean mempool
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Status kosong
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Mengosongkan antrean mempool
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

/// Daemon Sequencer L2 & Batch Assembler
#[derive(Debug, Default)]
pub struct L2Sequencer {
    pub mempool: L2Mempool,
    pub state: L2StateStore,
    pub engine: L2ExecutionEngine,
    pub current_block: u64,
    pub last_block_hash: Hash256,
    pub blocks: Vec<L2Block>,
    pub last_batch_end_block: u64,
    pub last_batch_state_root: Hash256,
    pub current_batch_index: u64,
    pub pending_batches: Vec<L2BatchFrame>,
}

impl L2Sequencer {
    /// Membuat instance Sequencer L2 baru dalam kondisi bersih
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Mengirimkan transaksi klien ke sequencer dengan validasi state awal
    pub fn submit_transaction(&mut self, tx: L2Transaction) -> Result<(), L2SequencerError> {
        // Validasi keberadaan akun dan saldo pengirim di state L2
        if let Some(account) = self.state.get_account(&tx.sender) {
            if tx.nonce < account.nonce {
                return Err(L2SequencerError::NonceTooLow {
                    expected: account.nonce,
                    actual: tx.nonce,
                });
            }
            let total_required = tx
                .amount
                .as_u128()
                .checked_add(tx.fee.as_u128())
                .ok_or(L2ExecutionError::ArithmeticOverflow)?;

            if account.balance.as_u128() < total_required {
                return Err(L2SequencerError::InsufficientBalance);
            }
        } else {
            return Err(L2SequencerError::SenderNotFound(tx.sender));
        }

        self.mempool.insert(tx)
    }

    /// Memproduksi blok L2 baru dari transaksi mempool dengan STF deterministik
    pub fn produce_block(&mut self, max_txs: usize) -> Result<Option<L2Block>, L2SequencerError> {
        if self.mempool.is_empty() {
            return Ok(None);
        }

        let pending_txs = self.mempool.drain_prioritized(max_txs);
        if pending_txs.is_empty() {
            return Ok(None);
        }

        // Eksekusi atomik seluruh transaksi via runtime L2 VM
        let _receipts = self.engine.execute_batch_atomic(&mut self.state, &pending_txs)?;

        self.current_block += 1;
        let new_state_root = self.state.compute_state_root();
        let txs_root = compute_txs_root(&pending_txs);

        let header = L2BlockHeader {
            block_number: self.current_block,
            prev_hash: self.last_block_hash,
            state_root: new_state_root,
            txs_root,
            timestamp: 1700000000 + self.current_block,
        };

        let block_hash = header.compute_hash();
        self.last_block_hash = block_hash;

        let block = L2Block {
            header,
            transactions: pending_txs,
        };

        self.blocks.push(block.clone());
        Ok(Some(block))
    }

    /// Memproduksi blok L2 baru beserta struk bukti Soft Finality (<50ms)
    pub fn produce_block_with_attestation(
        &mut self,
        max_txs: usize,
    ) -> Result<Option<(L2Block, L2SoftFinalityReceipt)>, L2SequencerError> {
        match self.produce_block(max_txs)? {
            Some(block) => {
                let receipt = L2SoftFinalityReceipt {
                    block_number: block.header.block_number,
                    block_hash: block.header.compute_hash(),
                    state_root: block.header.state_root,
                    txs_root: block.header.txs_root,
                    tx_count: block.transactions.len(),
                    timestamp: block.header.timestamp,
                };
                Ok(Some((block, receipt)))
            }
            None => Ok(None),
        }
    }

    /// Merakit blok-blok L2 baru menjadi paket pembingkaian kanonikal (L2BatchFrame)
    pub fn assemble_current_batch(&mut self) -> Result<Option<L2BatchFrame>, L2SequencerError> {
        if self.current_block <= self.last_batch_end_block {
            return Ok(None);
        }

        let start_block = self.last_batch_end_block + 1;
        let end_block = self.current_block;

        let mut batch_txs = Vec::new();
        for block in &self.blocks {
            if block.header.block_number >= start_block && block.header.block_number <= end_block {
                batch_txs.extend(block.transactions.clone());
            }
        }

        let payload = pack_batch_transactions(&batch_txs);
        let batch_index = self.current_batch_index + 1;
        let prev_state_root = self.last_batch_state_root;
        let new_state_root = self.state.compute_state_root();
        let tx_count = u32::try_from(batch_txs.len()).unwrap_or(u32::MAX);

        let frame = L2BatchFrame::new(
            batch_index,
            prev_state_root,
            new_state_root,
            start_block,
            end_block,
            tx_count,
            0x00, // 0x00: Raw Canonical Framing
            payload,
        );

        self.current_batch_index = batch_index;
        self.last_batch_end_block = end_block;
        self.last_batch_state_root = new_state_root;
        self.pending_batches.push(frame.clone());

        Ok(Some(frame))
    }

    /// Merakit sekumpulan blok L2 menjadi satu paket batch rollup kanonikal L2Batch (Legacy Helper)
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

        sequencer.state.set_account(L2Account::new(
            sender,
            Quantum::new(500_000_000),
            0,
        ));

        let tx = L2Transaction {
            sender,
            recipient,
            amount: Quantum::new(200_000_000),
            fee: Quantum::new(50_000),
            nonce: 0,
            signature: Signature::from_bytes([0u8; 64]),
            payload: vec![],
        };

        sequencer.submit_transaction(tx).expect("Submit tx gagal");
        assert_eq!(sequencer.mempool.len(), 1);

        let block_opt = sequencer.produce_block(10).unwrap();
        assert!(block_opt.is_some());
        let block = block_opt.unwrap();

        assert_eq!(block.header.block_number, 1);
        assert_eq!(block.transactions.len(), 1);
        assert_ne!(block.header.txs_root, Hash256::ZERO);
        assert!(sequencer.mempool.is_empty());

        // Verifikasi saldo akun termutasi
        let sender_after = sequencer.state.get_account(&sender).unwrap();
        assert_eq!(sender_after.balance.as_u128(), 299_950_000);
        assert_eq!(sender_after.nonce, 1);
    }

    #[test]
    fn test_mempool_capacity_limit_rejection() {
        let mut mempool = L2Mempool::with_capacity(2);
        let sender = Address::from_bytes([1u8; 32]);

        let tx1 = L2Transaction {
            sender,
            recipient: Address::from_bytes([2u8; 32]),
            amount: Quantum::new(10),
            fee: Quantum::new(10_000),
            nonce: 0,
            signature: Signature::ZERO,
            payload: vec![],
        };
        let tx2 = L2Transaction {
            sender,
            recipient: Address::from_bytes([2u8; 32]),
            amount: Quantum::new(10),
            fee: Quantum::new(10_000),
            nonce: 1,
            signature: Signature::ZERO,
            payload: vec![],
        };
        let tx3 = L2Transaction {
            sender,
            recipient: Address::from_bytes([2u8; 32]),
            amount: Quantum::new(10),
            fee: Quantum::new(10_000),
            nonce: 2,
            signature: Signature::ZERO,
            payload: vec![],
        };

        assert!(mempool.insert(tx1).is_ok());
        assert!(mempool.insert(tx2).is_ok());

        let err = mempool.insert(tx3).unwrap_err();
        assert_eq!(err, L2SequencerError::MempoolFull(2));
    }

    #[test]
    fn test_mempool_duplicate_nonce_rejected() {
        let mut mempool = L2Mempool::new();
        let sender = Address::from_bytes([5u8; 32]);

        let tx1 = L2Transaction {
            sender,
            recipient: Address::from_bytes([6u8; 32]),
            amount: Quantum::new(100),
            fee: Quantum::new(10_000),
            nonce: 7,
            signature: Signature::ZERO,
            payload: vec![],
        };

        let tx2 = L2Transaction {
            sender,
            recipient: Address::from_bytes([7u8; 32]),
            amount: Quantum::new(50),
            fee: Quantum::new(20_000),
            nonce: 7, // Nonce duplikat untuk sender yang sama
            signature: Signature::ZERO,
            payload: vec![],
        };

        assert!(mempool.insert(tx1).is_ok());
        let err = mempool.insert(tx2).unwrap_err();
        assert_eq!(
            err,
            L2SequencerError::DuplicateNonce {
                sender,
                nonce: 7,
            }
        );
    }

    #[test]
    fn test_mempool_fee_priority_ordering() {
        let mut mempool = L2Mempool::new();
        let sender_a = Address::from_bytes([1u8; 32]);
        let sender_b = Address::from_bytes([2u8; 32]);

        // Masukkan transaksi dengan fee 10_000 lebih dulu
        let tx_low_fee = L2Transaction {
            sender: sender_a,
            recipient: Address::ZERO,
            amount: Quantum::new(100),
            fee: Quantum::new(10_000),
            nonce: 0,
            signature: Signature::ZERO,
            payload: vec![],
        };

        // Masukkan transaksi dengan fee 50_000 setelahnya
        let tx_high_fee = L2Transaction {
            sender: sender_b,
            recipient: Address::ZERO,
            amount: Quantum::new(100),
            fee: Quantum::new(50_000),
            nonce: 0,
            signature: Signature::ZERO,
            payload: vec![],
        };

        mempool.insert(tx_low_fee).unwrap();
        mempool.insert(tx_high_fee).unwrap();

        // Drain 1 transaksi teratas: harus mengambil tx_high_fee
        let drained = mempool.drain_prioritized(1);
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].fee.as_u128(), 50_000);
        assert_eq!(drained[0].sender, sender_b);

        // Sisanya adalah tx_low_fee
        let remaining = mempool.drain_prioritized(1);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].fee.as_u128(), 10_000);
    }

    #[test]
    fn test_sequencer_produce_block_with_attestation() {
        let mut sequencer = L2Sequencer::new();
        let sender = Address::from_bytes([9u8; 32]);
        sequencer.state.set_account(L2Account::new(sender, Quantum::new(1_000_000), 0));

        let tx = L2Transaction {
            sender,
            recipient: Address::ZERO,
            amount: Quantum::new(100_000),
            fee: Quantum::new(10_000),
            nonce: 0,
            signature: Signature::ZERO,
            payload: vec![],
        };

        sequencer.submit_transaction(tx).unwrap();

        let (block, attestation) = sequencer
            .produce_block_with_attestation(10)
            .unwrap()
            .expect("Harus menghasilkan blok");

        assert_eq!(attestation.block_number, 1);
        assert_eq!(attestation.block_hash, block.header.compute_hash());
        assert_eq!(attestation.state_root, block.header.state_root);
        assert_eq!(attestation.txs_root, block.header.txs_root);
        assert_eq!(attestation.tx_count, 1);
    }

    #[test]
    fn test_sequencer_batch_assembly_and_unpacking() {
        let mut sequencer = L2Sequencer::new();
        let sender = Address::from_bytes([0x11; 32]);
        sequencer.state.set_account(L2Account::new(sender, Quantum::new(1_000_000_000), 0));

        // Buat 2 blok
        for i in 0..2 {
            let tx = L2Transaction {
                sender,
                recipient: Address::from_bytes([0x22; 32]),
                amount: Quantum::new(10_000),
                fee: Quantum::new(20_000),
                nonce: i,
                signature: Signature::ZERO,
                payload: vec![i as u8],
            };
            sequencer.submit_transaction(tx).unwrap();
            sequencer.produce_block(10).unwrap();
        }

        assert_eq!(sequencer.blocks.len(), 2);

        // Rakit batch
        let frame = sequencer
            .assemble_current_batch()
            .unwrap()
            .expect("Batch harus berhasil dirakit");

        assert_eq!(frame.header.batch_index, 1);
        assert_eq!(frame.header.start_block, 1);
        assert_eq!(frame.header.end_block, 2);
        assert_eq!(frame.header.tx_count, 2);
        assert_eq!(frame.header.new_state_root, sequencer.state.compute_state_root());

        // Dekode ulang payload calldata menjadi transaksi
        let unpacked = unpack_batch_transactions(&frame.payload).expect("Unpack gagal");
        assert_eq!(unpacked.len(), 2);
        assert_eq!(unpacked[0].nonce, 0);
        assert_eq!(unpacked[1].nonce, 1);
        assert_eq!(unpacked[0].payload, vec![0]);
        assert_eq!(unpacked[1].payload, vec![1]);

        // Perakitan kedua tanpa blok baru harus mengembalikan None
        assert!(sequencer.assemble_current_batch().unwrap().is_none());
    }

    #[test]
    fn test_submit_tx_insufficient_balance_rejected() {
        let mut sequencer = L2Sequencer::new();
        let sender = Address::from_bytes([0x33; 32]);
        sequencer.state.set_account(L2Account::new(sender, Quantum::new(100), 0));

        let tx = L2Transaction {
            sender,
            recipient: Address::ZERO,
            amount: Quantum::new(500),
            fee: Quantum::new(10_000),
            nonce: 0,
            signature: Signature::ZERO,
            payload: vec![],
        };

        let err = sequencer.submit_transaction(tx).unwrap_err();
        assert_eq!(err, L2SequencerError::InsufficientBalance);
    }
}

