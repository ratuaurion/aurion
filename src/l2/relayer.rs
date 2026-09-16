//! Relayer & Cross-Layer Messaging untuk Aurion Layer-2.
//! Mematuhi Invariant L2-RELAY-001 (Two-Way Messaging) dan L2-CENSOR-001 (Censorship Resistance via Forced Inclusion).

use crate::core::{Address, Hash256, Quantum};
use crate::crypto::blake3_hash;

/// Arah pesan lintas-layer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageDirection {
    L1ToL2,
    L2ToL1,
}

/// Status siklus hidup pesan lintas-layer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStatus {
    Pending,
    Relayed,
    Executed,
    TimedOut,
}

/// Pesan komunikasi dua arah antara L1 dan L2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossLayerMessage {
    pub id: Hash256,
    pub direction: MessageDirection,
    pub sender: Address,
    pub target: Address,
    pub amount: Quantum,
    pub payload: Vec<u8>,
    pub nonce: u64,
    pub fee: Quantum,
    pub enqueued_at_l1_block: u64,
    pub status: MessageStatus,
}

impl CrossLayerMessage {
    /// Membuat pesan lintas-layer baru dan menghitung ID kanonikal
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        direction: MessageDirection,
        sender: Address,
        target: Address,
        amount: Quantum,
        payload: Vec<u8>,
        nonce: u64,
        fee: Quantum,
        enqueued_at_l1_block: u64,
    ) -> Self {
        let id = Self::compute_id(direction, &sender, &target, amount, &payload, nonce);
        Self {
            id,
            direction,
            sender,
            target,
            amount,
            payload,
            nonce,
            fee,
            enqueued_at_l1_block,
            status: MessageStatus::Pending,
        }
    }

    /// Menghitung ID pesan berbasis hash Blake3
    #[must_use]
    pub fn compute_id(
        direction: MessageDirection,
        sender: &Address,
        target: &Address,
        amount: Quantum,
        payload: &[u8],
        nonce: u64,
    ) -> Hash256 {
        let mut data = Vec::with_capacity(1 + 64 + 64 + 16 + payload.len() + 8);
        data.push(match direction {
            MessageDirection::L1ToL2 => 1,
            MessageDirection::L2ToL1 => 2,
        });
        data.extend_from_slice(sender.as_bytes());
        data.extend_from_slice(target.as_bytes());
        data.extend_from_slice(&amount.as_u128().to_be_bytes());
        data.extend_from_slice(payload);
        data.extend_from_slice(&nonce.to_be_bytes());
        blake3_hash(&data)
    }
}

/// Bukti Merkle untuk klaim penarikan (withdrawal) L2 ke L1
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawalProof {
    pub withdrawal_hash: Hash256,
    pub merkle_branch: Vec<Hash256>,
    pub root: Hash256,
    pub leaf_index: usize,
}

impl WithdrawalProof {
    /// Memverifikasi apakah withdrawal_hash termuat dalam root Merkle state L2
    #[must_use]
    pub fn verify(&self) -> bool {
        let mut current = self.withdrawal_hash;
        let mut idx = self.leaf_index;

        for sibling in &self.merkle_branch {
            let mut combined = Vec::with_capacity(64);
            if idx.is_multiple_of(2) {
                combined.extend_from_slice(current.as_bytes());
                combined.extend_from_slice(sibling.as_bytes());
            } else {
                combined.extend_from_slice(sibling.as_bytes());
                combined.extend_from_slice(current.as_bytes());
            }
            current = blake3_hash(&combined);
            idx /= 2;
        }

        current == self.root
    }
}

/// Antrean Inklusi Paksa (Forced Inclusion Queue) di L1
/// Menjamin anti-sensor: jika Sequencer menyensor transaksi pengguna, transaksi dapat
/// di-enqueue langsung di L1 dan wajib dieksekusi dalam batas waktu (timeout window).
#[derive(Debug, Clone)]
pub struct ForcedInclusionQueue {
    pub max_timeout_blocks: u64,
    pub queue: Vec<CrossLayerMessage>,
}

impl ForcedInclusionQueue {
    /// Inisialisasi antrean inklusi paksa dengan jendela waktu tunggu tertentu
    #[must_use]
    pub fn new(max_timeout_blocks: u64) -> Self {
        Self {
            max_timeout_blocks,
            queue: Vec::new(),
        }
    }

    /// Memasukkan pesan L1->L2 ke dalam antrean paksa
    pub fn enqueue(&mut self, message: CrossLayerMessage) {
        self.queue.push(message);
    }

    /// Mengambil sejumlah pesan untuk dieksekusi oleh sequencer/relayer
    pub fn dequeue_batch(&mut self, limit: usize) -> Vec<CrossLayerMessage> {
        let count = limit.min(self.queue.len());
        self.queue.drain(0..count).collect()
    }

    /// Memeriksa apakah suatu pesan di dalam antrean telah melewati batas waktu inklusi paksa
    #[must_use]
    pub fn is_timed_out(&self, msg_id: &Hash256, current_l1_block: u64) -> bool {
        for msg in &self.queue {
            if &msg.id == msg_id {
                let elapsed = current_l1_block.saturating_sub(msg.enqueued_at_l1_block);
                return elapsed > self.max_timeout_blocks;
            }
        }
        false
    }

    /// Jumlah antrean aktif
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Status kosong
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_layer_message_creation_and_id() {
        let sender = Address::from_bytes([1u8; 32]);
        let target = Address::from_bytes([2u8; 32]);

        let msg = CrossLayerMessage::new(
            MessageDirection::L1ToL2,
            sender,
            target,
            Quantum::new(500_000_000),
            vec![0xAA, 0xBB],
            1,
            Quantum::new(10_000),
            100,
        );

        assert_eq!(msg.status, MessageStatus::Pending);
        assert_ne!(msg.id, Hash256::ZERO);
    }

    #[test]
    fn test_merkle_withdrawal_proof_verification() {
        let leaf0 = blake3_hash(b"withdrawal_0");
        let leaf1 = blake3_hash(b"withdrawal_1");

        let mut combined = Vec::new();
        combined.extend_from_slice(leaf0.as_bytes());
        combined.extend_from_slice(leaf1.as_bytes());
        let root = blake3_hash(&combined);

        // Bukti untuk leaf 0 (idx 0, sibling = leaf1)
        let proof0 = WithdrawalProof {
            withdrawal_hash: leaf0,
            merkle_branch: vec![leaf1],
            root,
            leaf_index: 0,
        };
        assert!(proof0.verify());

        // Bukti untuk leaf 1 (idx 1, sibling = leaf0)
        let proof1 = WithdrawalProof {
            withdrawal_hash: leaf1,
            merkle_branch: vec![leaf0],
            root,
            leaf_index: 1,
        };
        assert!(proof1.verify());

        // Bukti salah
        let bad_proof = WithdrawalProof {
            withdrawal_hash: leaf0,
            merkle_branch: vec![leaf0],
            root,
            leaf_index: 0,
        };
        assert!(!bad_proof.verify());
    }

    #[test]
    fn test_forced_inclusion_queue_lifecycle() {
        let mut queue = ForcedInclusionQueue::new(50);
        let sender = Address::from_bytes([1u8; 32]);
        let target = Address::from_bytes([2u8; 32]);

        let msg = CrossLayerMessage::new(
            MessageDirection::L1ToL2,
            sender,
            target,
            Quantum::new(1_000_000),
            vec![],
            1,
            Quantum::new(5_000),
            10,
        );

        let msg_id = msg.id;
        queue.enqueue(msg);
        assert_eq!(queue.len(), 1);

        // Pada blok 50 (elapsed = 40 <= 50), belum timed out
        assert!(!queue.is_timed_out(&msg_id, 50));

        // Pada blok 65 (elapsed = 55 > 50), terdeteksi timed out
        assert!(queue.is_timed_out(&msg_id, 65));

        // Dequeue batch
        let batch = queue.dequeue_batch(10);
        assert_eq!(batch.len(), 1);
        assert!(queue.is_empty());
    }
}
