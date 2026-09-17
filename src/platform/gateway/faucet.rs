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

use std::collections::HashMap;
use thiserror::Error;

use crate::core::{Address, Hash256, Quantum, Signature};
use crate::crypto::{derive_address_from_pubkey, Keypair};
use crate::mempool::MempoolEngine;
use crate::state::account::Account;
use crate::transaction::types::{Transaction, TxType};

pub const DEFAULT_FAUCET_DISPENSE_QUANTA: u128 = 1_000_000_000; // 10 AUR (10 * 10^8 Quanta)
pub const DEFAULT_FAUCET_FEE_QUANTA: u128 = 2_000; // 0.00002 AUR
pub const DEFAULT_FAUCET_COOLDOWN_SECS: u64 = 60; // 60 detik cooldown per alamat

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FaucetError {
    #[error("Faucet cooldown active: please wait {0} more seconds")]
    CooldownActive(u64),
    #[error("Faucet account has insufficient balance: available {available}, required {required}")]
    InsufficientBalance { available: Quantum, required: Quantum },
    #[error("Faucet account not found in state")]
    AccountNotFound,
    #[error("Mempool rejected faucet transaction: {0}")]
    MempoolError(String),
}

#[derive(Clone, Debug)]
pub struct FaucetConfig {
    pub dispense_amount: Quantum,
    pub fee: Quantum,
    pub cooldown_secs: u64,
}

impl Default for FaucetConfig {
    fn default() -> Self {
        Self {
            dispense_amount: Quantum::new(DEFAULT_FAUCET_DISPENSE_QUANTA),
            fee: Quantum::new(DEFAULT_FAUCET_FEE_QUANTA),
            cooldown_secs: DEFAULT_FAUCET_COOLDOWN_SECS,
        }
    }
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
}

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
        }
    }

    #[inline]
    pub fn address(&self) -> Address {
        self.faucet_address
    }

    /// Memeriksa apakah alamat penerima masih dalam masa cooldown.
    pub fn check_cooldown(&self, recipient: &Address, current_time: u64) -> Result<(), FaucetError> {
        if let Some(&last_time) = self.last_dispensed.get(recipient) {
            let elapsed = current_time.saturating_sub(last_time);
            if elapsed < self.config.cooldown_secs {
                return Err(FaucetError::CooldownActive(self.config.cooldown_secs - elapsed));
            }
        }
        Ok(())
    }

    /// Menyalurkan token testnet langsung ke antrean mempool simpul.
    pub fn dispense(
        &mut self,
        recipient: &Address,
        accounts: &HashMap<Address, Account>,
        mempool: &mut MempoolEngine,
        current_time: u64,
    ) -> Result<(Hash256, Transaction), FaucetError> {
        self.check_cooldown(recipient, current_time)?;

        let faucet_account = accounts.get(&self.faucet_address).cloned().unwrap_or_default();
        let total_required = self
            .config
            .dispense_amount
            .checked_add(self.config.fee)
            .map_err(|_| FaucetError::InsufficientBalance {
                available: faucet_account.balance,
                required: Quantum::new(u128::MAX),
            })?;

        if faucet_account.balance < total_required {
            return Err(FaucetError::InsufficientBalance {
                available: faucet_account.balance,
                required: total_required,
            });
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

        Ok((tx_hash, tx))
    }
}
