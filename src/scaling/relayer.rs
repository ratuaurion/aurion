//! Relayer & Cross-Layer Messaging untuk Aurion Layer-2.
//! Mematuhi Invariant:
//! - L2-RELAY-001 (Two-Way Messaging L1 <-> L2)
//! - L2-CENSOR-001 (Censorship Resistance via Forced Inclusion)
//! - L2-LIFE-003 (Emergency Exit / Escape Hatch Mechanism)
//! - AUR-ARCH-011 (#![forbid(unsafe_code)])
//! - AUR-ARCH-012 (Zero-Float Quantum u128)

use crate::core::{Address, Hash256, Quantum};
use crate::crypto::blake3_hash;
use crate::l2::bridge::{BridgeError, L2SettlementBridgeClient};
use crate::l2::state::{L2Account, L2AccountProof, L2StateStore};
use crate::l2::vm::L2ExecutionError;
use std::collections::BTreeSet;
use thiserror::Error;

/// Kesalahan Operasi Relayer L2 & Cross-Layer Messaging
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RelayerError {
    #[error("Pesan lintas-layer dengan ID {0} tidak ditemukan")]
    MessageNotFound(Hash256),

    #[error("Pesan lintas-layer sudah pernah dieksekusi")]
    AlreadyExecuted,

    #[error("Pesan inklusi paksa telah kedaluwarsa (timed out)")]
    TimedOut,

    #[error("Bukti penarikan atau bukti status akun L2 tidak valid")]
    InvalidProof,

    #[error("Mekanisme Escape Hatch belum aktif (sequencer beroperasi normal)")]
    EscapeHatchNotActive,

    #[error("Akun {0} sudah pernah mencairkan dana melalui Escape Hatch")]
    AlreadyClaimed(Address),

    #[error("Saldo akun L2 tidak mencukupi untuk penarikan")]
    InsufficientL2Balance,

    #[error("Kesalahan pada kontrak Bridge L1: {0}")]
    BridgeError(#[from] BridgeError),

    #[error("Kesalahan eksekusi VM L2: {0}")]
    ExecutionError(#[from] L2ExecutionError),
}

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

/// Daemon Relayer Dua Arah L1 <-> L2 & Pengelola Escape Hatch
#[derive(Debug)]
pub struct L2Relayer {
    pub bridge: L2SettlementBridgeClient,
    pub forced_queue: ForcedInclusionQueue,
    pub claimed_escape_hatch: BTreeSet<Address>,
    pub is_sequencer_frozen: bool,
    pub processed_deposits: u64,
}

impl L2Relayer {
    /// Inisialisasi Relayer baru
    #[must_use]
    pub fn new(bridge: L2SettlementBridgeClient, max_timeout_blocks: u64) -> Self {
        Self {
            bridge,
            forced_queue: ForcedInclusionQueue::new(max_timeout_blocks),
            claimed_escape_hatch: BTreeSet::new(),
            is_sequencer_frozen: false,
            processed_deposits: 0,
        }
    }

    /// Memproses deposit L1 -> L2 (Lock di L1 Vault & Mint di L2 State)
    pub fn process_l1_deposit_to_l2(
        &mut self,
        state: &mut L2StateStore,
        sender_l1: Address,
        recipient_l2: Address,
        amount: Quantum,
    ) -> Result<u64, RelayerError> {
        // 1. Kunci dana di vault bridge L1
        let nonce = self
            .bridge
            .process_deposit_full(sender_l1, recipient_l2, amount)?;

        // 2. Mint / kreditkan saldo pada state L2
        let existing = state
            .get_account(&recipient_l2)
            .cloned()
            .unwrap_or(L2Account::new(recipient_l2, Quantum::ZERO, 0));

        let new_balance = existing
            .balance
            .as_u128()
            .checked_add(amount.as_u128())
            .ok_or(BridgeError::ArithmeticOverflow)?;

        let updated = L2Account {
            address: recipient_l2,
            balance: Quantum::new(new_balance),
            nonce: existing.nonce,
            storage_root: existing.storage_root,
        };
        state.set_account(updated);
        self.processed_deposits += 1;

        Ok(nonce)
    }

    /// Memproses penarikan L2 -> L1 (Burn di L2 State & Unlock di L1 Vault via Merkle Proof)
    pub fn process_l2_withdrawal_to_l1(
        &mut self,
        state: &mut L2StateStore,
        sender_l2: Address,
        recipient_l1: Address,
        amount: Quantum,
        proof: WithdrawalProof,
    ) -> Result<(), RelayerError> {
        // 1. Validasi bukti penarikan Merkle terhadap root terkini di L1
        if !proof.verify() {
            return Err(RelayerError::InvalidProof);
        }
        if proof.root != self.bridge.latest_state_root {
            return Err(RelayerError::InvalidProof);
        }

        // 2. Bakar / kurangi saldo di L2
        let existing = state
            .get_account(&sender_l2)
            .cloned()
            .ok_or(RelayerError::InsufficientL2Balance)?;

        if existing.balance.as_u128() < amount.as_u128() {
            return Err(RelayerError::InsufficientL2Balance);
        }

        let new_balance = existing
            .balance
            .as_u128()
            .checked_sub(amount.as_u128())
            .ok_or(BridgeError::ArithmeticOverflow)?;

        let updated = L2Account {
            address: sender_l2,
            balance: Quantum::new(new_balance),
            nonce: existing.nonce,
            storage_root: existing.storage_root,
        };
        state.set_account(updated);

        // 3. Buka kunci dana di vault bridge L1 kepada penerima
        let _ = recipient_l1;
        self.bridge.process_withdrawal(amount, true)?;

        Ok(())
    }

    /// Mengeksekusi pesan-pesan dari antrean inklusi paksa ke dalam state L2
    pub fn process_forced_inclusion_batch(
        &mut self,
        state: &mut L2StateStore,
        limit: usize,
    ) -> Result<Vec<CrossLayerMessage>, RelayerError> {
        let mut messages = self.forced_queue.dequeue_batch(limit);

        for msg in &mut messages {
            if msg.direction == MessageDirection::L1ToL2 {
                let existing = state
                    .get_account(&msg.target)
                    .cloned()
                    .unwrap_or(L2Account::new(msg.target, Quantum::ZERO, 0));

                let new_bal = existing
                    .balance
                    .as_u128()
                    .checked_add(msg.amount.as_u128())
                    .ok_or(BridgeError::ArithmeticOverflow)?;

                let updated = L2Account {
                    address: msg.target,
                    balance: Quantum::new(new_bal),
                    nonce: existing.nonce,
                    storage_root: existing.storage_root,
                };
                state.set_account(updated);
            }
            msg.status = MessageStatus::Executed;
        }

        Ok(messages)
    }

    /// Memicu pembekuan sequencer (Sequencer Freeze)
    pub fn trigger_emergency_freeze(&mut self) {
        self.is_sequencer_frozen = true;
    }

    /// Memproses klaim penarikan darurat sepihak (Emergency Exit / Escape Hatch)
    /// Berbasis bukti keanggotaan akun L2AccountProof SMT Blake3 terhadap latest_state_root di L1
    pub fn process_escape_hatch(
        &mut self,
        proof: &L2AccountProof,
        recipient_l1: Address,
        current_l1_block: u64,
        account_balance: Quantum,
    ) -> Result<Quantum, RelayerError> {
        // 1. Periksa apakah jendela darurat aktif (sequencer freeze atau ada forced tx timed out)
        let has_timed_out = self
            .forced_queue
            .queue
            .iter()
            .any(|m| self.forced_queue.is_timed_out(&m.id, current_l1_block));

        if !self.is_sequencer_frozen && !has_timed_out {
            return Err(RelayerError::EscapeHatchNotActive);
        }

        // 2. Proteksi Anti-Klaim Ganda (Double Claim Protection)
        if self.claimed_escape_hatch.contains(&proof.address) {
            return Err(RelayerError::AlreadyClaimed(proof.address));
        }

        // 3. Verifikasi bukti SMT terhadap state root yang dikomitkan ke L1
        if !proof.verify() {
            return Err(RelayerError::InvalidProof);
        }
        if proof.root != self.bridge.latest_state_root {
            return Err(RelayerError::InvalidProof);
        }

        // 4. Buka kunci aset langsung dari vault L1
        let _ = recipient_l1;
        self.bridge.process_withdrawal(account_balance, true)?;

        // 5. Tandai bahwa akun ini sudah dicairkan
        self.claimed_escape_hatch.insert(proof.address);

        Ok(account_balance)
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

    #[test]
    fn test_relayer_l1_deposit_to_l2_mint() {
        let bridge_addr = Address::from_bytes([0x99; 32]);
        let bridge = L2SettlementBridgeClient::new(bridge_addr, Hash256::ZERO);
        let mut relayer = L2Relayer::new(bridge, 100);
        let mut state = L2StateStore::new();

        let sender_l1 = Address::from_bytes([1u8; 32]);
        let recipient_l2 = Address::from_bytes([2u8; 32]);
        let deposit_amount = Quantum::new(3_000_000_000); // 30 AUR

        let nonce = relayer
            .process_l1_deposit_to_l2(&mut state, sender_l1, recipient_l2, deposit_amount)
            .expect("Deposit L1->L2 gagal");

        assert_eq!(nonce, 1);
        assert_eq!(relayer.bridge.vault_balance.as_u128(), 3_000_000_000);

        // Periksa saldo di L2 state
        let l2_acc = state.get_account(&recipient_l2).unwrap();
        assert_eq!(l2_acc.balance.as_u128(), 3_000_000_000);
        assert_eq!(l2_acc.nonce, 0);
    }

    #[test]
    fn test_relayer_l2_withdrawal_to_l1_unlock() {
        let root = Hash256::from_bytes([0x77; 32]);
        let bridge_addr = Address::from_bytes([0x99; 32]);
        let mut bridge = L2SettlementBridgeClient::new(bridge_addr, root);
        bridge.process_deposit(Quantum::new(5_000_000_000)).unwrap();

        let mut relayer = L2Relayer::new(bridge, 100);
        let mut state = L2StateStore::new();

        let sender_l2 = Address::from_bytes([3u8; 32]);
        let recipient_l1 = Address::from_bytes([4u8; 32]);
        state.set_account(L2Account::new(sender_l2, Quantum::new(2_000_000_000), 0));

        // Bukti penarikan valid terhadap root bridge
        let proof = WithdrawalProof {
            withdrawal_hash: root,
            merkle_branch: vec![],
            root,
            leaf_index: 0,
        };

        relayer
            .process_l2_withdrawal_to_l1(
                &mut state,
                sender_l2,
                recipient_l1,
                Quantum::new(1_000_000_000),
                proof,
            )
            .expect("Penarikan L2->L1 gagal");

        // Saldo di L2 berkurang 10 AUR
        assert_eq!(
            state.get_account(&sender_l2).unwrap().balance.as_u128(),
            1_000_000_000
        );
        // Saldo vault L1 berkurang 10 AUR
        assert_eq!(relayer.bridge.vault_balance.as_u128(), 4_000_000_000);
    }

    #[test]
    fn test_forced_inclusion_queue_batch_execution() {
        let bridge = L2SettlementBridgeClient::new(Address::ZERO, Hash256::ZERO);
        let mut relayer = L2Relayer::new(bridge, 50);
        let mut state = L2StateStore::new();

        let target = Address::from_bytes([5u8; 32]);
        let msg = CrossLayerMessage::new(
            MessageDirection::L1ToL2,
            Address::ZERO,
            target,
            Quantum::new(400_000),
            vec![],
            1,
            Quantum::ZERO,
            10,
        );

        relayer.forced_queue.enqueue(msg);
        assert_eq!(relayer.forced_queue.len(), 1);

        let executed = relayer
            .process_forced_inclusion_batch(&mut state, 10)
            .unwrap();

        assert_eq!(executed.len(), 1);
        assert_eq!(executed[0].status, MessageStatus::Executed);
        assert_eq!(
            state.get_account(&target).unwrap().balance.as_u128(),
            400_000
        );
        assert!(relayer.forced_queue.is_empty());
    }

    #[test]
    fn test_escape_hatch_unilateral_claim_success() {
        let mut state = L2StateStore::new();
        let user_addr = Address::from_bytes([0x88; 32]);
        let user_acc = L2Account::new(user_addr, Quantum::new(750_000_000), 2);
        state.set_account(user_acc.clone());

        let l2_state_root = state.compute_state_root();

        let bridge_addr = Address::from_bytes([0x99; 32]);
        let mut bridge = L2SettlementBridgeClient::new(bridge_addr, l2_state_root);
        bridge.process_deposit(Quantum::new(1_000_000_000)).unwrap(); // 10 AUR di vault

        let mut relayer = L2Relayer::new(bridge, 100);

        // Hasilkan bukti SMT dari L2StateStore
        let proof = state
            .generate_account_proof(&user_addr)
            .expect("Proof SMT gagal");

        // 1. Klaim sebelum freeze harus ditolak
        let err = relayer
            .process_escape_hatch(&proof, user_addr, 50, user_acc.balance)
            .unwrap_err();
        assert_eq!(err, RelayerError::EscapeHatchNotActive);

        // 2. Bekukan sequencer
        relayer.trigger_emergency_freeze();

        // 3. Klaim berhasil saat freeze
        let claimed = relayer
            .process_escape_hatch(&proof, user_addr, 50, user_acc.balance)
            .expect("Klaim escape hatch gagal");

        assert_eq!(claimed.as_u128(), 750_000_000);
        assert_eq!(relayer.bridge.vault_balance.as_u128(), 250_000_000);

        // 4. Klaim kedua kalinya harus ditolak (Anti-Klaim Ganda)
        let double_err = relayer
            .process_escape_hatch(&proof, user_addr, 50, user_acc.balance)
            .unwrap_err();
        assert_eq!(double_err, RelayerError::AlreadyClaimed(user_addr));
    }
}
