//! Klien Settlement & Pembangkit Checkpoint Layer-3 ke Layer-2 (`src/specialized/settlement.rs`).
//! Menghubungkan komitmen state L3 ke dalam settlement contract di L2.
//! Mematuhi Invariant AUR-ARCH-011 (#![forbid(unsafe_code)]), AUR-ARCH-012 (Zero-Float Quantum u128),
//! AUR-L3-ARCH-001 (L1 Sovereignty Root), AUR-L3-ARCH-003 (Zero Float Mandate),
//! dan AUR-L3-SEC-002 (Settlement Verification Mandate).

use thiserror::Error;
use crate::core::{Hash256, Signature};
use crate::crypto::Keypair;
use crate::specialized::types::{DomainId, L3Block, L3Checkpoint};

/// Kesalahan Operasi Settlement & Checkpointing L3
#[derive(Debug, Error, PartialEq, Eq)]
pub enum L3SettlementError {
    #[error("Domain ID tidak cocok: diharapkan {expected}, aktual {actual}")]
    DomainMismatch { expected: DomainId, actual: DomainId },

    #[error("ID Checkpoint tidak berurutan: diharapkan {expected}, aktual {actual}")]
    NonSequentialCheckpointId { expected: u64, actual: u64 },

    #[error("Ketidaksesuaian Previous State Root: diharapkan {expected}, aktual {actual}")]
    PreviousStateRootMismatch { expected: Hash256, actual: Hash256 },

    #[error("Rentang blok tidak valid: start {start} > end {end}")]
    InvalidBlockRange { start: u64, end: u64 },

    #[error("Tanda tangan digital checkpoint tidak valid")]
    InvalidSignature,

    #[error("Tidak ada blok baru untuk dibuatkan checkpoint")]
    NoBlocksToCheckpoint,
}

/// Tahapan Finalitas Bertingkat Layer-3 (Rule 18 §3 & §4.7)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum L3FinalityTier {
    /// 1. Finalitas Lokal Instan (<10ms): Dikonfirmasi oleh runtime domain L3
    InstantLocal = 1,
    /// 2. Finalitas Soft di L2 (<50ms): Komitmen checkpoint tersimpan di L2 Rollup
    SoftL2Settled = 2,
    /// 3. Finalitas Hard di L1 (Round-Based BFT): Batch L2 difinalisasi di L1 Base
    HardL1Finalized = 3,
}

/// Status Finalitas Transaksi/Blok L3
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L3FinalityStatus {
    pub domain_id: DomainId,
    pub tx_id: Hash256,
    pub block_number: u64,
    pub tier: L3FinalityTier,
    pub l2_checkpoint_id: Option<u64>,
    pub l1_settlement_height: Option<u64>,
}

impl L3FinalityStatus {
    /// Membuat status awal dengan finalitas lokal instan
    #[must_use]
    pub fn new_local(domain_id: DomainId, tx_id: Hash256, block_number: u64) -> Self {
        Self {
            domain_id,
            tx_id,
            block_number,
            tier: L3FinalityTier::InstantLocal,
            l2_checkpoint_id: None,
            l1_settlement_height: None,
        }
    }

    /// Meningkatkan status finalitas ke Soft L2
    pub fn promote_to_l2(&mut self, checkpoint_id: u64) {
        self.tier = L3FinalityTier::SoftL2Settled;
        self.l2_checkpoint_id = Some(checkpoint_id);
    }

    /// Meningkatkan status finalitas ke Hard L1
    pub fn promote_to_l1(&mut self, l1_height: u64) {
        self.tier = L3FinalityTier::HardL1Finalized;
        self.l1_settlement_height = Some(l1_height);
    }
}

/// Generator Checkpoint State Periodik L3 (L3-TSK-301)
#[derive(Debug)]
pub struct L3CheckpointGenerator {
    pub domain_id: DomainId,
    pub cadence_blocks: u64,
    next_checkpoint_id: u64,
    last_checkpoint_end_block: u64,
    last_checkpoint_state_root: Hash256,
    pending_blocks: Vec<L3Block>,
}

impl L3CheckpointGenerator {
    /// Membuat generator checkpoint baru
    #[must_use]
    pub fn new(domain_id: DomainId, cadence_blocks: u64, initial_state_root: Hash256) -> Self {
        Self {
            domain_id,
            cadence_blocks,
            next_checkpoint_id: 1,
            last_checkpoint_end_block: 0,
            last_checkpoint_state_root: initial_state_root,
            pending_blocks: Vec::new(),
        }
    }

    /// Merekam blok baru yang telah dieksekusi di L3
    pub fn record_block(&mut self, block: L3Block) -> Result<(), L3SettlementError> {
        if block.domain_id != self.domain_id {
            return Err(L3SettlementError::DomainMismatch {
                expected: self.domain_id,
                actual: block.domain_id,
            });
        }
        self.pending_blocks.push(block);
        Ok(())
    }

    /// Mengetahui apakah cadens checkpoint sudah tercapai
    #[must_use]
    pub fn should_checkpoint(&self) -> bool {
        self.pending_blocks.len() as u64 >= self.cadence_blocks
    }

    /// Membentuk dan menandatangani komitmen checkpoint periodik
    pub fn create_checkpoint(
        &mut self,
        sequencer_keypair: &Keypair,
        proof_data: Vec<u8>,
    ) -> Result<L3Checkpoint, L3SettlementError> {
        if self.pending_blocks.is_empty() {
            return Err(L3SettlementError::NoBlocksToCheckpoint);
        }

        let start_block = self.pending_blocks.first().map_or(0, |b| b.block_number);
        let end_block = self.pending_blocks.last().map_or(0, |b| b.block_number);
        let new_state_root = self.pending_blocks.last().map_or(Hash256::ZERO, |b| b.state_root);

        let total_txs: u64 = self.pending_blocks.iter().map(|b| b.transactions.len() as u64).sum();

        let mut checkpoint = L3Checkpoint::new(
            self.domain_id,
            self.next_checkpoint_id,
            start_block,
            end_block,
            self.last_checkpoint_state_root,
            new_state_root,
            total_txs,
            Signature::from_bytes([0u8; 64]),
            proof_data,
        );

        let preimage = checkpoint.signing_preimage();
        checkpoint.signature = sequencer_keypair.sign(&preimage);

        // Update state internal generator
        self.next_checkpoint_id += 1;
        self.last_checkpoint_end_block = end_block;
        self.last_checkpoint_state_root = new_state_root;
        self.pending_blocks.clear();

        Ok(checkpoint)
    }
}

/// Klien Penerima & Verifikasi Settlement L3 di Layer-2 (L3-TSK-302)
#[derive(Debug, Clone)]
pub struct L2SettlementClient {
    pub domain_id: DomainId,
    pub sequencer_pubkey: [u8; 32],
    pub latest_checkpoint_id: u64,
    pub latest_settled_root: Hash256,
    pub settled_checkpoints: Vec<L3Checkpoint>,
}

impl L2SettlementClient {
    /// Membuat klien settlement baru di Layer-2
    #[must_use]
    pub fn new(domain_id: DomainId, sequencer_pubkey: [u8; 32], genesis_root: Hash256) -> Self {
        Self {
            domain_id,
            sequencer_pubkey,
            latest_checkpoint_id: 0,
            latest_settled_root: genesis_root,
            settled_checkpoints: Vec::new(),
        }
    }

    /// Menerima dan memvalidasi komitmen checkpoint L3
    pub fn ingest_checkpoint(&mut self, checkpoint: L3Checkpoint) -> Result<(), L3SettlementError> {
        // 1. Validasi Domain ID
        if checkpoint.domain_id != self.domain_id {
            return Err(L3SettlementError::DomainMismatch {
                expected: self.domain_id,
                actual: checkpoint.domain_id,
            });
        }

        // 2. Validasi Urutan Sekuensial ID Checkpoint
        let expected_cp_id = self.latest_checkpoint_id + 1;
        if checkpoint.checkpoint_id != expected_cp_id {
            return Err(L3SettlementError::NonSequentialCheckpointId {
                expected: expected_cp_id,
                actual: checkpoint.checkpoint_id,
            });
        }

        // 3. Validasi Kontinuitas Root State
        if checkpoint.previous_state_root != self.latest_settled_root {
            return Err(L3SettlementError::PreviousStateRootMismatch {
                expected: self.latest_settled_root,
                actual: checkpoint.previous_state_root,
            });
        }

        // 4. Validasi Rentang Blok
        if checkpoint.start_block > checkpoint.end_block {
            return Err(L3SettlementError::InvalidBlockRange {
                start: checkpoint.start_block,
                end: checkpoint.end_block,
            });
        }

        // 5. Verifikasi Tanda Tangan Kriptografis Sequencer L3
        if !checkpoint.verify_signature(&self.sequencer_pubkey) {
            return Err(L3SettlementError::InvalidSignature);
        }

        // Mutasi komitmen state settlement di L2
        self.latest_checkpoint_id = checkpoint.checkpoint_id;
        self.latest_settled_root = checkpoint.new_state_root;
        self.settled_checkpoints.push(checkpoint);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specialized::types::L3Transaction;

    #[test]
    fn test_checkpoint_generator_cadence_and_creation() {
        let domain_id = DomainId::DEX_DEFAULT;
        let sk = [0x77; 32];
        let kp = Keypair::from_seed(&sk);
        let pk = kp.public_key_bytes();

        let initial_root = Hash256::from_bytes([0xaa; 32]);
        let mut generator = L3CheckpointGenerator::new(domain_id, 2, initial_state_root(initial_root));

        let block1 = L3Block::new(
            domain_id,
            1,
            Hash256::ZERO,
            Hash256::from_bytes([0x11; 32]),
            Hash256::ZERO,
            Hash256::ZERO,
            1_700_000_000,
            vec![L3Transaction::new(
                domain_id,
                crate::core::Address::from_bytes([0x01; 32]),
                crate::core::Address::from_bytes([0x02; 32]),
                crate::core::Quantum::new(100),
                crate::core::Quantum::new(10),
                0,
                Signature::from_bytes([0u8; 64]),
                vec![],
            )],
        );

        let block2 = L3Block::new(
            domain_id,
            2,
            block1.compute_block_hash(),
            Hash256::from_bytes([0x22; 32]),
            Hash256::ZERO,
            Hash256::ZERO,
            1_700_000_010,
            vec![],
        );

        generator.record_block(block1).unwrap();
        assert!(!generator.should_checkpoint());

        generator.record_block(block2).unwrap();
        assert!(generator.should_checkpoint());

        let checkpoint = generator.create_checkpoint(&kp, vec![0xca, 0xfe]).unwrap();
        assert_eq!(checkpoint.checkpoint_id, 1);
        assert_eq!(checkpoint.start_block, 1);
        assert_eq!(checkpoint.end_block, 2);
        assert_eq!(checkpoint.previous_state_root, initial_root);
        assert_eq!(checkpoint.new_state_root, Hash256::from_bytes([0x22; 32]));
        assert_eq!(checkpoint.transactions_count, 1);
        assert!(checkpoint.verify_signature(&pk));
    }

    fn initial_state_root(r: Hash256) -> Hash256 {
        r
    }

    #[test]
    fn test_l2_settlement_client_ingestion_and_finality_tiers() {
        let domain_id = DomainId::APP_CHAIN_DEFAULT;
        let sk = [0x88; 32];
        let kp = Keypair::from_seed(&sk);
        let pk = kp.public_key_bytes();

        let genesis_root = Hash256::from_bytes([0x10; 32]);
        let mut client = L2SettlementClient::new(domain_id, pk, genesis_root);

        let mut cp1 = L3Checkpoint::new(
            domain_id,
            1,
            1,
            10,
            genesis_root,
            Hash256::from_bytes([0x20; 32]),
            150,
            Signature::from_bytes([0u8; 64]),
            vec![],
        );
        cp1.signature = kp.sign(&cp1.signing_preimage());

        client.ingest_checkpoint(cp1).expect("Ingest CP 1 harus berhasil");
        assert_eq!(client.latest_checkpoint_id, 1);
        assert_eq!(client.latest_settled_root, Hash256::from_bytes([0x20; 32]));

        // Uji multi-tier finality tracking
        let tx_hash = Hash256::from_bytes([0xee; 32]);
        let mut finality = L3FinalityStatus::new_local(domain_id, tx_hash, 5);
        assert_eq!(finality.tier, L3FinalityTier::InstantLocal);

        finality.promote_to_l2(1);
        assert_eq!(finality.tier, L3FinalityTier::SoftL2Settled);
        assert_eq!(finality.l2_checkpoint_id, Some(1));

        finality.promote_to_l1(100);
        assert_eq!(finality.tier, L3FinalityTier::HardL1Finalized);
        assert_eq!(finality.l1_settlement_height, Some(100));
    }

    #[test]
    fn test_l2_settlement_client_rejections() {
        let domain_id = DomainId::GAMING_DEFAULT;
        let sk = [0x55; 32];
        let kp = Keypair::from_seed(&sk);
        let pk = kp.public_key_bytes();

        let genesis_root = Hash256::from_bytes([0x01; 32]);
        let mut client = L2SettlementClient::new(domain_id, pk, genesis_root);

        // CP dengan checkpoint_id salah (melompat ke 2)
        let mut bad_cp = L3Checkpoint::new(
            domain_id,
            2, // Harusnya 1
            1,
            5,
            genesis_root,
            Hash256::from_bytes([0x02; 32]),
            10,
            Signature::from_bytes([0u8; 64]),
            vec![],
        );
        bad_cp.signature = kp.sign(&bad_cp.signing_preimage());

        let res = client.ingest_checkpoint(bad_cp);
        assert!(matches!(res, Err(L3SettlementError::NonSequentialCheckpointId { .. })));
    }
}
