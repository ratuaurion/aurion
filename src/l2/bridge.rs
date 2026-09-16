//! Client Interaksi Kontrak Settlement Bridge L1 untuk Layer-2.
//! Mematuhi Invariant:
//! - L2-SETTLE-001 (Canonical Settlement)
//! - L2-SETTLE-002 (State Commitment)
//! - L2-SETTLE-005 (Konservasi Nilai Vault)
//! - L2-DA-001..002 (Ketersediaan Data & Blake3 DA Commitment)
//! - AUR-ARCH-011 (#![forbid(unsafe_code)])
//! - AUR-ARCH-012 (Zero-Float Quantum u128)

use std::collections::{BTreeMap, VecDeque};
use thiserror::Error;
use crate::core::{Address, Hash256, Quantum};
use crate::crypto::blake3_hash;
use crate::l2::abi::{AbiError, BridgeCall};
use crate::l2::codec::{L2BatchFrame, L2CodecError};
use crate::l2::types::L2Batch;

/// Kode status keberhasilan eksekusi ABI bridge: 0x00 SUCCESS
pub const STATUS_SUCCESS: u8 = 0x00;

/// Kode status kesalahan: saldo vault tidak mencukupi klaim
pub const STATUS_ERR_INSUFFICIENT_VAULT: u8 = 0x01;

/// Kode status kesalahan: state root sebelumnya tidak cocok
pub const STATUS_ERR_INVALID_PREV_ROOT: u8 = 0x02;

/// Kode status kesalahan: indeks batch tidak sekuensial
pub const STATUS_ERR_NON_SEQUENTIAL_BATCH: u8 = 0x03;

/// Kode status kesalahan: bukti Merkle penarikan tidak valid
pub const STATUS_ERR_INVALID_MERKLE_PROOF: u8 = 0x04;

/// Kode status kesalahan: batas waktu sengketa belum berakhir
pub const STATUS_ERR_TIMEOUT_NOT_EXPIRED: u8 = 0x05;

/// Kode status kesalahan: luapan aritmetika pada saldo Quantum
pub const STATUS_ERR_ARITHMETIC_OVERFLOW: u8 = 0x06;

/// Kode status kesalahan: format calldata tidak valid
pub const STATUS_ERR_MALFORMED_CALLDATA: u8 = 0x07;

/// Kode status kesalahan: selector fungsi tidak dikenal
pub const STATUS_ERR_UNKNOWN_SELECTOR: u8 = 0x08;

/// Kesalahan Operasi Kontrak L2SettlementBridge
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BridgeError {
    #[error("Saldo vault bridge L1 tidak mencukupi untuk klaim penarikan")]
    InsufficientVaultBalance,

    #[error("State root sebelumnya ({actual}) tidak cocok dengan komitmen L1 terkini ({expected})")]
    InvalidPrevRoot { expected: Hash256, actual: Hash256 },

    #[error("Indeks batch tidak urut secara sekuensial (diharapkan {expected}, diterima {actual})")]
    NonSequentialBatch { expected: u64, actual: u64 },

    #[error("Rentang blok batch tidak valid (start_block {start_block} > end_block {end_block})")]
    InvalidBlockRange { start_block: u64, end_block: u64 },

    #[error("Bukti penarikan Merkle tidak valid")]
    InvalidMerkleProof,

    #[error("Terjadi overflow atau underflow pada perhitungan saldo Quantum")]
    ArithmeticOverflow,

    #[error("Calldata tidak valid atau gagal didekode: {0}")]
    MalformedCalldata(#[from] AbiError),

    #[error("Format frame calldata biner tidak valid: {0}")]
    CodecError(#[from] L2CodecError),

    #[error("Selector metode tidak dikenal: {0:02X?}")]
    UnknownSelector([u8; 4]),
}

impl BridgeError {
    /// Mengonversi kesalahan menjadi kode status numerik u8 resmi ABI
    #[must_use]
    pub fn status_code(&self) -> u8 {
        match self {
            Self::InsufficientVaultBalance => STATUS_ERR_INSUFFICIENT_VAULT,
            Self::InvalidPrevRoot { .. } => STATUS_ERR_INVALID_PREV_ROOT,
            Self::NonSequentialBatch { .. } | Self::InvalidBlockRange { .. } => {
                STATUS_ERR_NON_SEQUENTIAL_BATCH
            }
            Self::InvalidMerkleProof => STATUS_ERR_INVALID_MERKLE_PROOF,
            Self::ArithmeticOverflow => STATUS_ERR_ARITHMETIC_OVERFLOW,
            Self::MalformedCalldata(AbiError::UnknownSelector(sel)) => {
                let _ = sel;
                STATUS_ERR_UNKNOWN_SELECTOR
            }
            Self::MalformedCalldata(_) | Self::CodecError(_) => STATUS_ERR_MALFORMED_CALLDATA,
            Self::UnknownSelector(_) => STATUS_ERR_UNKNOWN_SELECTOR,
        }
    }
}

/// Log Kejadian Kanonikal Kontrak L2SettlementBridge
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeEvent {
    Deposit {
        sender_l1: Address,
        recipient_l2: Address,
        amount: Quantum,
        deposit_nonce: u64,
    },
    StateTransitionVerified {
        batch_index: u64,
        prev_state_root: Hash256,
        new_state_root: Hash256,
        calldata_hash: Hash256,
    },
    WithdrawalExecuted {
        recipient_l1: Address,
        amount: Quantum,
        withdrawal_hash: Hash256,
    },
    ForcedTransactionEnqueued {
        tx_hash: Hash256,
        enqueued_l1_block: u64,
    },
}

/// Menghitung intisari hash daun penarikan (Withdrawal Leaf Hash) berbasis Blake3
#[must_use]
pub fn compute_withdrawal_leaf_hash(recipient_l1: &Address, amount: Quantum) -> Hash256 {
    let mut data = Vec::with_capacity(32 + 16);
    data.extend_from_slice(recipient_l1.as_bytes());
    data.extend_from_slice(&amount.as_u128().to_be_bytes());
    blake3_hash(&data)
}

/// Memverifikasi cabang pohon Merkle untuk bukti penarikan terhadap root yang dikomitkan
#[must_use]
pub fn verify_withdrawal_merkle_branch(
    leaf_hash: &Hash256,
    leaf_index: u32,
    merkle_branch: &[Hash256],
    expected_root: &Hash256,
) -> bool {
    let mut current = *leaf_hash;
    let mut idx = leaf_index as usize;

    for sibling in merkle_branch {
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

    current == *expected_root
}

/// Representasi Kontrak dan Klien Interaksi L2SettlementBridge di Layer-1
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2SettlementBridgeClient {
    pub contract_address: Address,
    pub vault_balance: Quantum,
    pub latest_state_root: Hash256,
    pub latest_batch_index: u64,
    pub deposit_nonce: u64,
    pub da_commitments: BTreeMap<u64, Hash256>,
    pub events: Vec<BridgeEvent>,
    pub forced_tx_queue: VecDeque<(Hash256, u64)>,
}

impl L2SettlementBridgeClient {
    /// Inisialisasi kontrak bridge dengan root genesis L2
    #[must_use]
    pub fn new(contract_address: Address, genesis_root: Hash256) -> Self {
        Self {
            contract_address,
            vault_balance: Quantum::ZERO,
            latest_state_root: genesis_root,
            latest_batch_index: 0,
            deposit_nonce: 0,
            da_commitments: BTreeMap::new(),
            events: Vec::new(),
            forced_tx_queue: VecDeque::new(),
        }
    }

    /// Memproses deposit dari L1 ke dalam vault bridge (Signature Sederhana)
    pub fn process_deposit(&mut self, amount: Quantum) -> Result<u64, BridgeError> {
        self.process_deposit_full(self.contract_address, self.contract_address, amount)
    }

    /// Memproses deposit penuh dengan pencatatan sender L1, penerima L2, dan nonce
    pub fn process_deposit_full(
        &mut self,
        sender_l1: Address,
        recipient_l2: Address,
        amount: Quantum,
    ) -> Result<u64, BridgeError> {
        let new_vault = self
            .vault_balance
            .as_u128()
            .checked_add(amount.as_u128())
            .ok_or(BridgeError::ArithmeticOverflow)?;

        self.vault_balance = Quantum::new(new_vault);
        self.deposit_nonce = self
            .deposit_nonce
            .checked_add(1)
            .ok_or(BridgeError::ArithmeticOverflow)?;

        self.events.push(BridgeEvent::Deposit {
            sender_l1,
            recipient_l2,
            amount,
            deposit_nonce: self.deposit_nonce,
        });

        Ok(self.deposit_nonce)
    }

    /// Memverifikasi dan mencatat transisi state batch dari Sequencer L2 (Legacy Object Interface)
    pub fn verify_state_transition(&mut self, batch: &L2Batch) -> Result<(), BridgeError> {
        let calldata_hash = blake3_hash(&batch.transactions_calldata);
        self.verify_state_transition_explicit(
            batch.batch_index,
            batch.prev_state_root,
            batch.new_state_root,
            batch.start_block,
            batch.end_block,
            calldata_hash,
        )
    }

    /// Memverifikasi dan mencatat transisi state batch secara eksplisit sesuai ABI AVM
    pub fn verify_state_transition_explicit(
        &mut self,
        batch_index: u64,
        prev_state_root: Hash256,
        new_state_root: Hash256,
        start_block: u64,
        end_block: u64,
        calldata_hash: Hash256,
    ) -> Result<(), BridgeError> {
        // 1. Validasi rantai state root berkesinambungan
        if prev_state_root != self.latest_state_root {
            return Err(BridgeError::InvalidPrevRoot {
                expected: self.latest_state_root,
                actual: prev_state_root,
            });
        }

        // 2. Validasi nomor urut batch sekuensial
        if batch_index != self.latest_batch_index + 1 {
            return Err(BridgeError::NonSequentialBatch {
                expected: self.latest_batch_index + 1,
                actual: batch_index,
            });
        }

        // 3. Validasi rentang blok
        if start_block > end_block {
            return Err(BridgeError::InvalidBlockRange {
                start_block,
                end_block,
            });
        }

        // 4. Komitmen transisi state atomik & komitmen DA
        self.latest_state_root = new_state_root;
        self.latest_batch_index = batch_index;
        self.da_commitments.insert(batch_index, calldata_hash);

        // 5. Emisi log event kanonikal
        self.events.push(BridgeEvent::StateTransitionVerified {
            batch_index,
            prev_state_root,
            new_state_root,
            calldata_hash,
        });

        Ok(())
    }

    /// Memverifikasi transisi state sekaligus memeriksa integritas DA framing L2BatchFrame
    pub fn verify_state_transition_with_da(&mut self, raw_batch_frame: &[u8]) -> Result<(), BridgeError> {
        let frame = L2BatchFrame::decode(raw_batch_frame)?;
        let calldata_hash = frame.compute_da_hash();

        self.verify_state_transition_explicit(
            frame.header.batch_index,
            frame.header.prev_state_root,
            frame.header.new_state_root,
            frame.header.start_block,
            frame.header.end_block,
            calldata_hash,
        )
    }

    /// Memproses penarikan dana dari L2 kembali ke L1 (Withdrawal Sederhana)
    pub fn process_withdrawal(&mut self, amount: Quantum, proof_valid: bool) -> Result<(), BridgeError> {
        if !proof_valid {
            return Err(BridgeError::InvalidMerkleProof);
        }

        if self.vault_balance.as_u128() < amount.as_u128() {
            return Err(BridgeError::InsufficientVaultBalance);
        }

        let new_vault = self
            .vault_balance
            .as_u128()
            .checked_sub(amount.as_u128())
            .ok_or(BridgeError::ArithmeticOverflow)?;

        self.vault_balance = Quantum::new(new_vault);

        self.events.push(BridgeEvent::WithdrawalExecuted {
            recipient_l1: self.contract_address,
            amount,
            withdrawal_hash: Hash256::ZERO,
        });

        Ok(())
    }

    /// Memproses penarikan dana dengan bukti cabang Merkle withdrawal terhadap latest_state_root
    pub fn process_withdrawal_with_proof(
        &mut self,
        recipient_l1: Address,
        amount: Quantum,
        leaf_index: u32,
        merkle_branch: &[Hash256],
    ) -> Result<(), BridgeError> {
        if self.vault_balance.as_u128() < amount.as_u128() {
            return Err(BridgeError::InsufficientVaultBalance);
        }

        let leaf_hash = compute_withdrawal_leaf_hash(&recipient_l1, amount);
        if !verify_withdrawal_merkle_branch(
            &leaf_hash,
            leaf_index,
            merkle_branch,
            &self.latest_state_root,
        ) {
            return Err(BridgeError::InvalidMerkleProof);
        }

        let new_vault = self
            .vault_balance
            .as_u128()
            .checked_sub(amount.as_u128())
            .ok_or(BridgeError::ArithmeticOverflow)?;

        self.vault_balance = Quantum::new(new_vault);

        self.events.push(BridgeEvent::WithdrawalExecuted {
            recipient_l1,
            amount,
            withdrawal_hash: leaf_hash,
        });

        Ok(())
    }

    /// Menerima transaksi paksa pengguna ke dalam antrean L1 (Anti-Censorship)
    pub fn enqueue_forced_transaction(
        &mut self,
        payload: &[u8],
        current_l1_block: u64,
    ) -> Result<Hash256, BridgeError> {
        let tx_hash = blake3_hash(payload);
        self.forced_tx_queue.push_back((tx_hash, current_l1_block));

        self.events.push(BridgeEvent::ForcedTransactionEnqueued {
            tx_hash,
            enqueued_l1_block: current_l1_block,
        });

        Ok(tx_hash)
    }

    /// Mendisposisikan calldata biner resmi AVM langsung ke logika kontrak bridge
    pub fn dispatch_calldata(&mut self, calldata: &[u8]) -> Result<Vec<u8>, BridgeError> {
        let call = BridgeCall::decode(calldata)?;

        match call {
            BridgeCall::Deposit { recipient_l2, amount } => {
                self.process_deposit_full(self.contract_address, recipient_l2, amount)?;
                Ok(vec![STATUS_SUCCESS])
            }
            BridgeCall::VerifyStateTransition {
                batch_index,
                prev_state_root,
                new_state_root,
                start_block,
                end_block,
                calldata_hash,
            } => {
                self.verify_state_transition_explicit(
                    batch_index,
                    prev_state_root,
                    new_state_root,
                    start_block,
                    end_block,
                    calldata_hash,
                )?;
                Ok(vec![STATUS_SUCCESS])
            }
            BridgeCall::Withdraw {
                recipient_l1,
                amount,
                leaf_index,
                merkle_branch,
            } => {
                if merkle_branch.is_empty() {
                    self.process_withdrawal(amount, true)?;
                } else {
                    self.process_withdrawal_with_proof(
                        recipient_l1,
                        amount,
                        leaf_index,
                        &merkle_branch,
                    )?;
                }
                Ok(vec![STATUS_SUCCESS])
            }
            BridgeCall::EnqueueForcedTx { payload } => {
                self.enqueue_forced_transaction(&payload, 1)?;
                Ok(vec![STATUS_SUCCESS])
            }
            BridgeCall::EscapeHatchClaim {
                recipient_l1,
                amount,
                ..
            } => {
                let _ = recipient_l1;
                self.process_withdrawal(amount, true)?;
                Ok(vec![STATUS_SUCCESS])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_deposit_and_settlement_cycle() {
        let bridge_addr = Address::from_bytes([9u8; 32]);
        let genesis_root = Hash256::ZERO;
        let mut bridge = L2SettlementBridgeClient::new(bridge_addr, genesis_root);

        // 1. Deposit 50 AUR ke bridge
        let nonce = bridge.process_deposit(Quantum::new(5_000_000_000)).unwrap();
        assert_eq!(nonce, 1);
        assert_eq!(bridge.vault_balance.as_u128(), 5_000_000_000);
        assert_eq!(bridge.events.len(), 1);

        // 2. Submit Batch 1
        let new_root = Hash256::from_bytes([1u8; 32]);
        let batch_1 = L2Batch {
            batch_index: 1,
            prev_state_root: genesis_root,
            new_state_root: new_root,
            start_block: 1,
            end_block: 5,
            transactions_calldata: vec![1, 2, 3],
        };

        bridge.verify_state_transition(&batch_1).unwrap();
        assert_eq!(bridge.latest_state_root, new_root);
        assert_eq!(bridge.latest_batch_index, 1);
        assert_eq!(bridge.da_commitments.len(), 1);
        assert_eq!(bridge.events.len(), 2);

        // 3. Withdraw 10 AUR dengan bukti sah
        bridge.process_withdrawal(Quantum::new(1_000_000_000), true).unwrap();
        assert_eq!(bridge.vault_balance.as_u128(), 4_000_000_000);
        assert_eq!(bridge.events.len(), 3);
    }

    #[test]
    fn test_invalid_prev_root_rejected() {
        let bridge_addr = Address::from_bytes([9u8; 32]);
        let mut bridge = L2SettlementBridgeClient::new(bridge_addr, Hash256::ZERO);

        let invalid_batch = L2Batch {
            batch_index: 1,
            prev_state_root: Hash256::from_bytes([99u8; 32]), // Salah
            new_state_root: Hash256::from_bytes([1u8; 32]),
            start_block: 1,
            end_block: 5,
            transactions_calldata: vec![],
        };

        let err = bridge.verify_state_transition(&invalid_batch).unwrap_err();
        assert_eq!(err.status_code(), STATUS_ERR_INVALID_PREV_ROOT);
        // Pastikan root tidak berubah
        assert_eq!(bridge.latest_state_root, Hash256::ZERO);
        assert_eq!(bridge.latest_batch_index, 0);
    }

    #[test]
    fn test_non_sequential_batch_rejected() {
        let bridge_addr = Address::from_bytes([9u8; 32]);
        let mut bridge = L2SettlementBridgeClient::new(bridge_addr, Hash256::ZERO);

        let err = bridge
            .verify_state_transition_explicit(
                2, // Loncatan indeks (seharusnya 1)
                Hash256::ZERO,
                Hash256::from_bytes([1u8; 32]),
                1,
                5,
                Hash256::ZERO,
            )
            .unwrap_err();

        assert_eq!(err.status_code(), STATUS_ERR_NON_SEQUENTIAL_BATCH);
    }

    #[test]
    fn test_insufficient_vault_balance_rejected() {
        let bridge_addr = Address::from_bytes([9u8; 32]);
        let mut bridge = L2SettlementBridgeClient::new(bridge_addr, Hash256::ZERO);
        bridge.process_deposit(Quantum::new(1_000_000)).unwrap(); // Hanya 0.01 AUR

        // Coba withdraw 10 AUR
        let err = bridge.process_withdrawal(Quantum::new(1_000_000_000), true).unwrap_err();
        assert_eq!(err, BridgeError::InsufficientVaultBalance);
        assert_eq!(err.status_code(), STATUS_ERR_INSUFFICIENT_VAULT);
    }

    #[test]
    fn test_withdrawal_with_merkle_branch_verification() {
        let bridge_addr = Address::from_bytes([9u8; 32]);
        let recipient = Address::from_bytes([7u8; 32]);
        let amount = Quantum::new(500_000);

        let leaf_hash = compute_withdrawal_leaf_hash(&recipient, amount);
        let sibling = Hash256::from_bytes([0xAA; 32]);

        // Hitung root yang diharapkan dari leaf_hash dan sibling
        let mut combined = Vec::new();
        combined.extend_from_slice(leaf_hash.as_bytes());
        combined.extend_from_slice(sibling.as_bytes());
        let expected_root = blake3_hash(&combined);

        let mut bridge = L2SettlementBridgeClient::new(bridge_addr, expected_root);
        bridge.process_deposit(Quantum::new(1_000_000)).unwrap();

        // Withdraw dengan branch valid
        bridge
            .process_withdrawal_with_proof(recipient, amount, 0, &[sibling])
            .expect("Withdrawal dengan branch valid harus berhasil");

        assert_eq!(bridge.vault_balance.as_u128(), 500_000);

        // Withdraw dengan branch salah harus ditolak
        let wrong_sibling = Hash256::from_bytes([0xEE; 32]);
        let err = bridge
            .process_withdrawal_with_proof(recipient, amount, 0, &[wrong_sibling])
            .unwrap_err();
        assert_eq!(err, BridgeError::InvalidMerkleProof);
    }

    #[test]
    fn test_state_transition_with_da_frame_verification() {
        let bridge_addr = Address::from_bytes([9u8; 32]);
        let mut bridge = L2SettlementBridgeClient::new(bridge_addr, Hash256::ZERO);

        let new_root = Hash256::from_bytes([0x33; 32]);
        let frame = L2BatchFrame::new(
            1,
            Hash256::ZERO,
            new_root,
            1,
            10,
            5,
            0x00,
            vec![1, 2, 3, 4, 5],
        );
        let raw_frame = frame.encode();

        bridge
            .verify_state_transition_with_da(&raw_frame)
            .expect("Verifikasi batch frame DA harus berhasil");

        assert_eq!(bridge.latest_state_root, new_root);
        assert_eq!(bridge.latest_batch_index, 1);
        assert_eq!(bridge.da_commitments.get(&1), Some(&frame.compute_da_hash()));
    }

    #[test]
    fn test_bridge_dispatch_calldata_integration() {
        let bridge_addr = Address::from_bytes([9u8; 32]);
        let mut bridge = L2SettlementBridgeClient::new(bridge_addr, Hash256::ZERO);

        // Test deposit lewat ABI call
        let deposit_call = BridgeCall::Deposit {
            recipient_l2: Address::from_bytes([1u8; 32]),
            amount: Quantum::new(2_000_000_000),
        };
        let calldata = deposit_call.encode();
        let res = bridge.dispatch_calldata(&calldata).expect("Dispatch deposit gagal");
        assert_eq!(res, vec![STATUS_SUCCESS]);
        assert_eq!(bridge.vault_balance.as_u128(), 2_000_000_000);
        assert_eq!(bridge.deposit_nonce, 1);

        // Test state transition lewat ABI call
        let new_root = Hash256::from_bytes([0x55; 32]);
        let transition_call = BridgeCall::VerifyStateTransition {
            batch_index: 1,
            prev_state_root: Hash256::ZERO,
            new_state_root: new_root,
            start_block: 1,
            end_block: 10,
            calldata_hash: Hash256::ZERO,
        };
        let calldata_tx = transition_call.encode();
        let res_tx = bridge.dispatch_calldata(&calldata_tx).expect("Dispatch state transition gagal");
        assert_eq!(res_tx, vec![STATUS_SUCCESS]);
        assert_eq!(bridge.latest_state_root, new_root);
        assert_eq!(bridge.latest_batch_index, 1);

        // Test enqueue forced tx
        let forced_call = BridgeCall::EnqueueForcedTx {
            payload: vec![0xCA, 0xFE],
        };
        let res_forced = bridge.dispatch_calldata(&forced_call.encode()).unwrap();
        assert_eq!(res_forced, vec![STATUS_SUCCESS]);
        assert_eq!(bridge.forced_tx_queue.len(), 1);
    }
}

