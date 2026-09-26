#![forbid(unsafe_code)]

//! Subregister Faucet Testnet Publik Aurion (NET-012).
//!
//! Mematuhi Invariant:
//! - AUR-ARCH-001: Single Sovereign Binary (/bin/aurion).
//! - AUR-ARCH-011: Absolute Zero Unsafe Code (#![forbid(unsafe_code)]).
//! - AUR-ARCH-012: Absolute Zero Float Arithmetic (Fixed Precision Quantum u128).
//!
//! Submodul ini menyediakan dispenser dana uji coba testnet terotentikasi
//! untuk pengembang komunitas dan dompet pihak ketiga dengan penegakan batas
//! frekuensi (cooldown anti-abuse) dan pengiriman transaksi native ke mempool.

use std::collections::{HashMap, VecDeque};
use thiserror::Error;

use crate::core::{Address, Hash256, Quantum, Signature};
use crate::crypto::{derive_address_from_pubkey, Keypair};
use crate::mempool::MempoolEngine;
use crate::state::account::Account;
use crate::transaction::types::{Transaction, TxType};

pub const DEFAULT_FAUCET_DISPENSE_QUANTA: u128 = 1_000_000_000; // 10 AUR (10 * 10^8 Quanta)
pub const DEFAULT_FAUCET_FEE_QUANTA: u128 = 2_000; // 0.00002 AUR
pub const DEFAULT_FAUCET_COOLDOWN_SECS: u64 = 60; // 60 detik cooldown per alamat

/// Cadangan minimum yang TIDAK BOLEH tersentuh oleh payout (AUR-MON §2.3).
///
/// Faucet boleh menghabiskan saldo operasional, tetapi tidak boleh menguras
/// rekening hingga nol: sedetik setelah saldo nol, faucet tidak lagi dapat
/// melayani siapa pun dan jaringan kehilangan sumber dana pembangun. Dengan
/// reserving, operator melihat "faucet dry" jauh sebelum kejadian.
pub const DEFAULT_FAUCET_RESERVE_FLOOR_QUANTA: u128 = 100_000_000; // 1 AUR

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum FaucetError {
    #[error("Faucet cooldown active: please wait {0} more seconds")]
    CooldownActive(u64),
    #[error("Faucet account has insufficient balance: available {available}, required {required}")]
    InsufficientBalance {
        available: Quantum,
        required: Quantum,
    },
    #[error("Faucet account not found in state")]
    AccountNotFound,
    #[error("Mempool rejected faucet transaction: {0}")]
    MempoolError(String),
    /// Saldo ada, tetapi tidak cukup untuk membayar dispense **dan** menjaga
    /// reserve floor. Ini kondisi "faucet dry", bukan error saldo kosong.
    #[error("Faucet reserve floor reached: balance {available}, reserve floor {reserve}, dispense {dispense}")]
    ReserveFloorBreached {
        available: Quantum,
        reserve: Quantum,
        dispense: Quantum,
    },
    #[error("Faucet has not been funded from Master Treasury: run 'aurion faucet init' first")]
    NotInitialized,
    #[error("Faucet keypair keystore is required: {0}")]
    KeystoreRequired(String),
    #[error("Treasury account not found in genesis state: {0}")]
    TreasuryNotFound(String),
    #[error("Treasury balance insufficient: available {available}, required {required}")]
    TreasuryInsufficient {
        available: Quantum,
        required: Quantum,
    },
    #[error("Faucet already funded with {0} AUR; refusing to re-initialize")]
    AlreadyInitialized(u64),
    #[error("Invalid faucet configuration: {0}")]
    InvalidConfig(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaucetConfig {
    pub dispense_amount: Quantum,
    pub fee: Quantum,
    pub cooldown_secs: u64,
    /// Saldo minimum yang wajib tersisa di rekening faucet.
    pub reserve_floor: Quantum,
    /// Kuota maksimum total yang boleh dicairkan oleh faucet (anti-runaway).
    pub max_total_dispense: Quantum,
}

impl Default for FaucetConfig {
    fn default() -> Self {
        Self {
            dispense_amount: Quantum::new(DEFAULT_FAUCET_DISPENSE_QUANTA),
            fee: Quantum::new(DEFAULT_FAUCET_FEE_QUANTA),
            cooldown_secs: DEFAULT_FAUCET_COOLDOWN_SECS,
            reserve_floor: Quantum::new(DEFAULT_FAUCET_RESERVE_FLOOR_QUANTA),
            // Kuota default longgar; operator testnet boleh menaikkan/menurunkan.
            max_total_dispense: Quantum::new(u128::MAX / 2),
        }
    }
}

impl FaucetConfig {
    /// Validasi konfigurasi sebelum dipakai.
    ///
    /// Menolak dispense/fee/reserve yang tidak masuk akal agar operator tidak
    /// bisa salah konfigurasi faucet menjadi tidak berguna atau bocor.
    ///
    /// # Outputs
    /// - `Ok(())`: konfigurasi konsisten.
    ///
    /// # Errors
    /// `dispense_amount` nol, `max_total_dispense` nol, atau `reserve_floor`
    /// lebih besar dari dispense sehingga faucet tidak pernah bisa membayar.
    pub fn validate(&self) -> Result<(), FaucetError> {
        if self.dispense_amount.is_zero() {
            return Err(FaucetError::InvalidConfig(
                "dispense_amount tidak boleh nol".to_string(),
            ));
        }
        if self.reserve_floor > self.dispense_amount {
            return Err(FaucetError::InvalidConfig(format!(
                "reserve_floor {} melebihi dispense_amount {} sehingga faucet tidak pernah bisa membayar",
                self.reserve_floor.as_u128(),
                self.dispense_amount.as_u128()
            )));
        }
        if self.max_total_dispense.is_zero() {
            return Err(FaucetError::InvalidConfig(
                "max_total_dispense tidak boleh nol".to_string(),
            ));
        }
        Ok(())
    }

    /// Saldo minimum faucet agar dispense berikutnya masih diperbolehkan.
    #[must_use]
    pub fn minimum_funding(&self) -> Quantum {
        // dispense + fee + reserve, dijepit agar tidak overflow pada konfigurasi ekstrem.
        self.dispense_amount
            .checked_add(self.fee)
            .and_then(|v| v.checked_add(self.reserve_floor))
            .unwrap_or(Quantum::MAX_SUPPLY)
    }
}

/// Satu catatan audit distribusi untuk jejak yang dapat ditelusuri.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaucetAuditRecord {
    pub timestamp: u64,
    pub recipient: Address,
    pub amount: Quantum,
    pub fee: Quantum,
    pub tx_hash: Hash256,
    /// `true` bila transaksi ditolak (mis. cooldown / reserve), `false` bila sukses.
    pub rejected: bool,
}

/// Mesin penyalur dana uji coba testnet berdaulat.
pub struct FaucetDispenser {
    pub keypair: Keypair,
    pub faucet_address: Address,
    pub chain_id: u32,
    pub config: FaucetConfig,
    pub last_dispensed: HashMap<Address, u64>,
    pub total_dispensed_quanta: u128,
    pub total_requests: u64,
    /// Jejak audit distribusi (termasuk penolakan) untuk akuntabilitas.
    pub audit_log: VecDeque<FaucetAuditRecord>,
}

/// Batas jumlah catatan audit yang disimpan di memori per simpul.
pub const FAUCET_AUDIT_CAPACITY: usize = 1_024;

impl FaucetDispenser {
    pub fn new(keypair: Keypair, chain_id: u32, config: FaucetConfig) -> Self {
        let faucet_address = derive_address_from_pubkey(&keypair.public_key_bytes());
        Self {
            keypair,
            faucet_address,
            chain_id,
            config,
            last_dispensed: HashMap::new(),
            total_dispensed_quanta: 0,
            total_requests: 0,
            audit_log: VecDeque::with_capacity(FAUCET_AUDIT_CAPACITY),
        }
    }

    #[inline]
    pub fn address(&self) -> Address {
        self.faucet_address
    }

    /// Catat satu peristiwa (sukses atau penolakan) ke jejak audit.
    fn audit(&mut self, record: FaucetAuditRecord) {
        if self.audit_log.len() >= FAUCET_AUDIT_CAPACITY {
            self.audit_log.pop_front();
        }
        self.audit_log.push_back(record);
    }

    /// Catat penolakan dispense beserta alasannya (tanpa mengubah state).
    fn audit_rejection(&mut self, recipient: Address, current_time: u64, _reason: FaucetError) {
        self.total_requests = self.total_requests.saturating_add(1);
        self.audit(FaucetAuditRecord {
            timestamp: current_time,
            recipient,
            amount: self.config.dispense_amount,
            fee: self.config.fee,
            tx_hash: Hash256::ZERO,
            rejected: true,
        });
    }

    /// Jumlah dispense yang masih boleh dilayani sebelum kuota total habis.
    #[must_use]
    pub fn remaining_quota(&self) -> Quantum {
        let dispensed = Quantum::new(self.total_dispensed_quanta);
        self.config
            .max_total_dispense
            .checked_sub(dispensed)
            .unwrap_or(Quantum::ZERO)
    }

    /// Memeriksa apakah alamat penerima masih dalam masa cooldown.
    pub fn check_cooldown(
        &self,
        recipient: &Address,
        current_time: u64,
    ) -> Result<(), FaucetError> {
        if let Some(&last_time) = self.last_dispensed.get(recipient) {
            let elapsed = current_time.saturating_sub(last_time);
            if elapsed < self.config.cooldown_secs {
                return Err(FaucetError::CooldownActive(
                    self.config.cooldown_secs - elapsed,
                ));
            }
        }
        Ok(())
    }

    /// Menyalurkan token testnet langsung ke antrean mempool simpul.
    ///
    /// ## Gerbang keselamatan (berurutan)
    ///
    /// 1. Konfigurasi harus konsisten.
    /// 2. Cooldown per penerima (anti-spam).
    /// 3. Kuota total belum habis (anti-runaway).
    /// 4. Saldo cukup untuk dispense + fee.
    /// 5. Saldo setelah payout **tetap di atas reserve floor**.
    ///
    /// ## Inputs
    /// - `recipient`: alamat tujuan.
    /// - `accounts`: state on-chain terkini.
    /// - `mempool`: mempool simpul.
    /// - `current_time`: waktu blok/sekarang dalam detik.
    ///
    /// ## Outputs
    /// `(tx_hash, transaction)` untuk transaksi yang berhasil disiarkan.
    ///
    /// ## Errors
    /// Setiap gerbang di atas; kegagalan **tidak** mengubah state apa pun dan
    /// dicatat di `audit_log` sebagai `rejected`.
    pub fn dispense(
        &mut self,
        recipient: &Address,
        accounts: &HashMap<Address, Account>,
        mempool: &mut MempoolEngine,
        current_time: u64,
    ) -> Result<(Hash256, Transaction), FaucetError> {
        // Gerbang 1: konfigurasi.
        if let Err(e) = self.config.validate() {
            self.audit_rejection(*recipient, current_time, e.clone());
            return Err(e);
        }

        // Gerbang 2: cooldown per alamat (anti-spam).
        if let Err(e) = self.check_cooldown(recipient, current_time) {
            self.audit_rejection(*recipient, current_time, e.clone());
            return Err(e);
        }

        // Gerbang 3: kuota total (anti-runaway).
        if self.remaining_quota() < self.config.dispense_amount {
            let e = FaucetError::InsufficientBalance {
                available: self.remaining_quota(),
                required: self.config.dispense_amount,
            };
            self.audit_rejection(*recipient, current_time, e.clone());
            return Err(e);
        }

        let faucet_account = accounts
            .get(&self.faucet_address)
            .cloned()
            .ok_or(FaucetError::AccountNotFound)?;
        let balance = faucet_account.balance;

        let total_required = self
            .config
            .dispense_amount
            .checked_add(self.config.fee)
            .map_err(|_| FaucetError::InsufficientBalance {
                available: balance,
                required: Quantum::new(u128::MAX),
            })?;

        // Gerbang 4: saldo cukup untuk dispense + fee.
        if balance < total_required {
            let e = FaucetError::InsufficientBalance {
                available: balance,
                required: total_required,
            };
            self.audit_rejection(*recipient, current_time, e.clone());
            return Err(e);
        }

        // Gerbang 5: reserve floor harus tetap terjaga setelah payout.
        // Inilah yang mencegah rekening faucet diuras hingga nol.
        let remaining = balance.checked_sub(total_required).unwrap_or(Quantum::ZERO);
        if remaining < self.config.reserve_floor {
            let e = FaucetError::ReserveFloorBreached {
                available: balance,
                reserve: self.config.reserve_floor,
                dispense: self.config.dispense_amount,
            };
            self.audit_rejection(*recipient, current_time, e.clone());
            return Err(e);
        }

        // Hitung nonce yang aman dari duplikasi transaksi yang masih antre di mempool
        let mut nonce = faucet_account.nonce;
        while mempool
            .by_sender_nonce
            .contains_key(&(self.faucet_address, nonce))
        {
            nonce += 1;
        }

        let mut tx = Transaction {
            version: 1,
            chain_id: self.chain_id,
            tx_type: TxType::Transfer,
            flags: 0,
            sender: self.faucet_address,
            recipient: *recipient,
            amount: self.config.dispense_amount,
            fee: self.config.fee,
            nonce,
            valid_until: current_time + 3600,
            payload: Vec::new(),
            signature: Signature::from_bytes([0u8; 64]),
        };

        let preimage = tx.signing_preimage();
        tx.signature = self.keypair.sign(&preimage);

        let tx_hash = mempool
            .submit_transaction(
                tx.clone(),
                &self.keypair.public_key_bytes(),
                current_time,
                &faucet_account,
            )
            .map_err(|e| FaucetError::MempoolError(format!("{e:?}")))?;

        self.last_dispensed.insert(*recipient, current_time);
        self.total_dispensed_quanta = self
            .total_dispensed_quanta
            .saturating_add(self.config.dispense_amount.as_u128());
        self.total_requests += 1;
        self.audit(FaucetAuditRecord {
            timestamp: current_time,
            recipient: *recipient,
            amount: self.config.dispense_amount,
            fee: self.config.fee,
            tx_hash,
            rejected: false,
        });

        Ok((tx_hash, tx))
    }

    /// Dana faucet dari **Master Treasury** ke rekening faucet (AUR-MON §2.3).
    ///
    /// ## Aturan konstitusional
    ///
    /// Faucet **tidak pernah** mencetak koin. Ia harus menerima transfer nyata
    /// dari rekening operasional Master Treasury, yang saldonya berasal dari
    /// 100% pasokan Blok 0. Fungsi ini membangun transaksi `Transfer` yang
    /// ditandatangani Treasury dan disiarkan ke mempool, sehingga aliran dana
    /// mengikuti consensus reguler — bukan jalur khusus.
    ///
    /// ## Inputs
    /// - `treasury_keypair`: kunci Master Treasury (penandatangan).
    /// - `amount`: jumlah yang dipindahkan ke faucet.
    /// - `accounts`: state on-chain terkini.
    /// - `mempool`: mempool simpul.
    /// - `current_time`: waktu sekarang.
    ///
    /// ## Outputs
    /// `(tx_hash, transaction)` pendanaan faucet.
    ///
    /// ## Errors
    /// `AlreadyInitialized` bila faucet sudah didanai; `TreasuryNotFound`;
    /// `TreasuryInsufficient`; `InvalidConfig` bila amount di bawah minimum.
    pub fn fund_from_treasury(
        &mut self,
        treasury_keypair: &Keypair,
        amount: Quantum,
        accounts: &HashMap<Address, Account>,
        mempool: &mut MempoolEngine,
        current_time: u64,
    ) -> Result<(Hash256, Transaction), FaucetError> {
        self.config.validate()?;

        // Menolak pendanaan ulang: mencegah operator menumpuk dana faucet
        // tanpa batas hanya dengan mengulang `init`.
        let existing = accounts
            .get(&self.faucet_address)
            .map_or(Quantum::ZERO, |a| a.balance);
        if !existing.is_zero() {
            let aur = existing.as_u128() / 1_000_000_000;
            return Err(FaucetError::AlreadyInitialized(aur as u64));
        }

        // Amount harus menutup dispense pertama DAN menyisakan reserve floor.
        let minimum = self.config.minimum_funding();
        if amount < minimum {
            return Err(FaucetError::InvalidConfig(format!(
                "amount {} Quanta lebih kecil dari minimum {} Quanta (dispense {} + fee {} + reserve {})",
                amount.as_u128(),
                minimum.as_u128(),
                self.config.dispense_amount.as_u128(),
                self.config.fee.as_u128(),
                self.config.reserve_floor.as_u128()
            )));
        }

        let treasury_address = derive_address_from_pubkey(&treasury_keypair.public_key_bytes());
        let treasury_account = accounts
            .get(&treasury_address)
            .cloned()
            .ok_or_else(|| FaucetError::TreasuryNotFound(treasury_address.to_hex()))?;

        // Treasury harus mampu membayar amount + fee.
        let total =
            amount
                .checked_add(self.config.fee)
                .map_err(|_| FaucetError::TreasuryInsufficient {
                    available: treasury_account.balance,
                    required: Quantum::new(u128::MAX),
                })?;
        if treasury_account.balance < total {
            return Err(FaucetError::TreasuryInsufficient {
                available: treasury_account.balance,
                required: total,
            });
        }

        // Nonce Treasury dihindari bentrok dengan entri mempool yang antre.
        let mut nonce = treasury_account.nonce;
        while mempool
            .by_sender_nonce
            .contains_key(&(treasury_address, nonce))
        {
            nonce += 1;
        }

        let mut tx = Transaction {
            version: 1,
            chain_id: self.chain_id,
            tx_type: TxType::Transfer,
            flags: 0,
            sender: treasury_address,
            recipient: self.faucet_address,
            nonce,
            amount,
            fee: self.config.fee,
            valid_until: current_time + 3_600,
            payload: Vec::new(),
            signature: Signature::from_bytes([0u8; 64]),
        };
        tx.signature = treasury_keypair.sign(&tx.signing_preimage());

        let tx_hash = mempool
            .submit_transaction(
                tx.clone(),
                &treasury_keypair.public_key_bytes(),
                current_time,
                &treasury_account,
            )
            .map_err(|e| FaucetError::MempoolError(format!("{e:?}")))?;

        self.audit(FaucetAuditRecord {
            timestamp: current_time,
            recipient: self.faucet_address,
            amount,
            fee: self.config.fee,
            tx_hash,
            rejected: false,
        });

        Ok((tx_hash, tx))
    }
}
