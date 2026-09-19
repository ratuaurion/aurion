//! Mesin Kolam Memori Transaksi (Mempool Engine) dengan Mandat RBF.
//! Mematuhi Dokumen 03 (03-TRANSACTION-LIFECYCLE.md).

use crate::core::{Address, Hash256, MonetaryError, Quantum};
use crate::crypto::derive_address_from_pubkey;
use crate::mempool::types::MempoolEntry;
use crate::state::account::Account;
use crate::transaction::types::{transaction_wire_size, Transaction};
use crate::transaction::validator::validate_transaction_stateless;
use std::collections::HashMap;
use thiserror::Error;

pub const DEFAULT_MAX_MEMPOOL_CAPACITY: usize = 50_000;
pub const DEFAULT_MEMPOOL_TTL_SECS: u64 = 3_600; // 1 Jam (Sekitar 60 Blok)
pub const RBF_MIN_FEE_BUMP_PERCENT: u128 = 10; // Mandat RBF: Kenaikan fee minimal +10%

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MempoolError {
    #[error("Sender address does not match public key: derived {derived}, got {declared}")]
    SenderMismatch { derived: Address, declared: Address },
    #[error("Stateless transaction validation failed: {0}")]
    StatelessValidation(String),
    #[error("Transaction nonce {received} is lower than account on-chain nonce {expected}")]
    NonceTooLow { expected: u64, received: u64 },
    #[error("Insufficient balance: account has {balance}, required {required}")]
    InsufficientBalance { balance: Quantum, required: Quantum },
    #[error("Mempool full and transaction fee is too low for eviction")]
    MempoolFull,
    #[error("RBF Rejected: Replacement fee must be at least +10% higher (existing: {existing}, required: {required}, provided: {provided})")]
    InsufficientFeeForReplacement {
        existing: Quantum,
        required: Quantum,
        provided: Quantum,
    },
    #[error("Monetary calculation error: {0}")]
    Monetary(#[from] MonetaryError),
}

/// Mesin penyimpanan dan prioritisasi transaksi mempool.
pub struct MempoolEngine {
    pub entries: HashMap<Hash256, MempoolEntry>,
    pub by_sender_nonce: HashMap<(Address, u64), Hash256>,
    pub max_capacity: usize,
    pub ttl_secs: u64,
}

impl Default for MempoolEngine {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_MEMPOOL_CAPACITY, DEFAULT_MEMPOOL_TTL_SECS)
    }
}

impl MempoolEngine {
    pub fn new(max_capacity: usize, ttl_secs: u64) -> Self {
        Self {
            entries: HashMap::new(),
            by_sender_nonce: HashMap::new(),
            max_capacity,
            ttl_secs,
        }
    }

    /// Memasukkan transaksi ke mempool dengan evaluasi RBF dan saldo.
    pub fn submit_transaction(
        &mut self,
        tx: Transaction,
        sender_pubkey: &[u8; 32],
        current_time: u64,
        account_state: &Account,
    ) -> Result<Hash256, MempoolError> {
        // 1. Verifikasi Kecocokan Kunci Publik Pengirim
        let derived_addr = derive_address_from_pubkey(sender_pubkey);
        if derived_addr != tx.sender {
            return Err(MempoolError::SenderMismatch {
                derived: derived_addr,
                declared: tx.sender,
            });
        }

        // 2. Validasi Nir-Status (Stateless: Tanda Tangan, Ukuran, Format)
        validate_transaction_stateless(&tx, sender_pubkey)
            .map_err(|e| MempoolError::StatelessValidation(format!("{e:?}")))?;

        // 3. Validasi Nonce terhadap State On-Chain
        if tx.nonce < account_state.nonce {
            return Err(MempoolError::NonceTooLow {
                expected: account_state.nonce,
                received: tx.nonce,
            });
        }

        // 4. Validasi Saldo Akun (amount + fee)
        let total_required = tx.amount.checked_add(tx.fee)?;
        if account_state.balance < total_required {
            return Err(MempoolError::InsufficientBalance {
                balance: account_state.balance,
                required: total_required,
            });
        }

        let sender_nonce_key = (tx.sender, tx.nonce);

        // 5. Evaluasi Mandat Replace-By-Fee (RBF)
        if let Some(&existing_tx_id) = self.by_sender_nonce.get(&sender_nonce_key) {
            let existing_entry = self
                .entries
                .get(&existing_tx_id)
                .expect("mempool index inconsistency");

            let existing_fee_u128 = existing_entry.tx.fee.as_u128();
            let bump = existing_fee_u128.saturating_mul(RBF_MIN_FEE_BUMP_PERCENT) / 100;
            let required_fee_u128 = existing_fee_u128.saturating_add(bump.max(1));
            let required_fee = Quantum::new(required_fee_u128);

            if tx.fee < required_fee {
                return Err(MempoolError::InsufficientFeeForReplacement {
                    existing: existing_entry.tx.fee,
                    required: required_fee,
                    provided: tx.fee,
                });
            }

            // Hapus transaksi lama yang digantikan (Status: Replaced)
            self.entries.remove(&existing_tx_id);
            self.by_sender_nonce.remove(&sender_nonce_key);
        }

        // 6. Pemeriksaan Kapasitas & Penggusuran (Eviction Anti-DoS)
        if self.entries.len() >= self.max_capacity {
            let lowest_id = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.tx.fee)
                .map(|(id, _)| *id);

            if let Some(low_id) = lowest_id {
                let lowest_fee = self.entries.get(&low_id).unwrap().tx.fee;
                if tx.fee <= lowest_fee {
                    return Err(MempoolError::MempoolFull);
                }

                // Gusur transaksi berbiaya terendah
                if let Some(evicted) = self.entries.remove(&low_id) {
                    self.by_sender_nonce
                        .remove(&(evicted.tx.sender, evicted.tx.nonce));
                }
            }
        }

        // 7. Penerimaan Resmi ke dalam Mempool
        let tx_id = tx.compute_tx_id();
        let entry = MempoolEntry::new(tx, *sender_pubkey, current_time);
        self.entries.insert(tx_id, entry);
        self.by_sender_nonce.insert(sender_nonce_key, tx_id);

        Ok(tx_id)
    }

    /// Bersihkan transaksi yang kedaluwarsa melampaui TTL (3.600 detik).
    pub fn evict_expired(&mut self, current_time: u64) -> usize {
        let mut expired_ids = Vec::new();

        for (tx_id, entry) in &self.entries {
            if current_time.saturating_sub(entry.admitted_timestamp) > self.ttl_secs {
                expired_ids.push(*tx_id);
            }
        }

        let count = expired_ids.len();
        for id in expired_ids {
            if let Some(entry) = self.entries.remove(&id) {
                self.by_sender_nonce
                    .remove(&(entry.tx.sender, entry.tx.nonce));
            }
        }
        count
    }

    /// Hapus transaksi yang telah dimasukkan dan difinalisasi di blok kanonikal.
    pub fn remove_finalized(&mut self, tx_ids: &[Hash256]) {
        for id in tx_ids {
            if let Some(entry) = self.entries.remove(id) {
                self.by_sender_nonce
                    .remove(&(entry.tx.sender, entry.tx.nonce));
            }
        }
    }

    /// Mengemas transaksi berprioritas fee tertinggi untuk dijadikan kandidat blok.
    /// Memastikan transaksi dari pengirim yang sama dieksekusi dengan urutan nonce menaik,
    /// dan antrean antar-pengirim diprioritaskan berdasarkan fee tertinggi secara deterministik.
    pub fn pack_block_candidate(&self, max_payload_bytes: usize) -> Vec<Transaction> {
        // Kelompokkan transaksi per pengirim
        let mut per_sender: std::collections::BTreeMap<Address, Vec<Transaction>> =
            std::collections::BTreeMap::new();
        for entry in self.entries.values() {
            per_sender
                .entry(entry.tx.sender)
                .or_default()
                .push(entry.tx.clone());
        }

        // Urutkan setiap antrean pengirim berdasarkan nonce menaik
        // Disimpan dalam urutan reverse agar pop() mengambil nonce terkecil terlebih dahulu (O(1))
        for queue in per_sender.values_mut() {
            queue.sort_by_key(|tx| std::cmp::Reverse(tx.nonce));
        }

        let mut candidate_txs = Vec::new();
        let mut current_bytes = 0;

        loop {
            // Cari pengirim dengan transaksi terdepan (head) ber-fee tertinggi secara deterministik
            let mut best_sender: Option<Address> = None;
            let mut best_fee = Quantum::ZERO;

            for (sender, queue) in &per_sender {
                if let Some(head_tx) = queue.last() {
                    let is_better = match best_sender {
                        None => true,
                        Some(_) => head_tx.fee > best_fee,
                    };
                    if is_better {
                        best_sender = Some(*sender);
                        best_fee = head_tx.fee;
                    }
                }
            }

            let sender = match best_sender {
                Some(s) => s,
                None => break,
            };

            let queue = per_sender.get_mut(&sender).unwrap();
            let next_tx = queue.pop().unwrap();

            let tx_size = transaction_wire_size(next_tx.payload.len());
            if current_bytes + tx_size <= max_payload_bytes {
                current_bytes += tx_size;
                candidate_txs.push(next_tx);
            } else {
                break;
            }
        }

        candidate_txs
    }

    /// Jumlah transaksi aktif di mempool saat ini.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
