//! Pembangun State σ0 dan Blok Genesis Kanonikal Aurion.

use crate::consensus::certificate::{ValidatorEntry, ValidatorSet};
use crate::consensus::header::BlockHeader;
use crate::core::{Address, Hash256, Quantum, MASTER_TREASURY_ALLOCATION_QUANTA};
use crate::state::account::Account;
use crate::state::monetary::MonetaryState;
use std::collections::HashMap;

pub const GENESIS_CHAIN_ID: u32 = 1001;
pub const GENESIS_TIMESTAMP: u64 = 1773532800; // 15 Maret 2026 00:00:00 UTC

/// Hasil inisialisasi blok dan state genesis.
#[derive(Clone)]
pub struct GenesisInitialization {
    pub header: BlockHeader,
    pub accounts: HashMap<Address, Account>,
    pub monetary: MonetaryState,
    pub validator_set: ValidatorSet,
}

/// Bangun state awal σ0 dan blok genesis kanonikal.
/// Model Single Treasury: 100% pasokan genesis dialokasikan ke satu akun Master Treasury.
pub fn build_genesis(
    treasury_addr: Address,
    validators: Vec<ValidatorEntry>,
) -> GenesisInitialization {
    let mut accounts = HashMap::new();

    // 1. Alokasi Eksklusif Master Treasury: 100% Pasokan Genesis (66.000.000 AUR)
    accounts.insert(
        treasury_addr,
        Account::new(Quantum::new(MASTER_TREASURY_ALLOCATION_QUANTA), 0),
    );

    let monetary = MonetaryState::new(
        Quantum::new(MASTER_TREASURY_ALLOCATION_QUANTA),
        Quantum::ZERO,
    );

    let validator_set = ValidatorSet::new(validators);

    let state_root = crate::state::smt::compute_accounts_state_root(&accounts);

    let header = BlockHeader {
        version: 1,
        height: 0,
        round: 0,
        timestamp: GENESIS_TIMESTAMP,
        prev_block_hash: Hash256::ZERO,
        tx_merkle_root: Hash256::ZERO,
        state_root,
    };

    GenesisInitialization {
        header,
        accounts,
        monetary,
        validator_set,
    }
}
