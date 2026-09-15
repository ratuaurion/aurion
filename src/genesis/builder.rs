//! Pembangun State σ0 dan Blok Genesis Kanonikal Aurion.

use crate::consensus::certificate::{ValidatorEntry, ValidatorSet};
use crate::consensus::header::BlockHeader;
use crate::core::{
    Address, Hash256, Quantum, CREATOR_ALLOCATION_QUANTA, DEVELOPER_ALLOCATION_QUANTA,
};
use crate::state::account::Account;
use crate::state::monetary::MonetaryState;
use std::collections::HashMap;

pub const GENESIS_CHAIN_ID: u32 = 1001;
pub const GENESIS_TIMESTAMP: u64 = 1773532800; // 15 Maret 2026 00:00:00 UTC

/// Hasil inisialisasi blok dan state genesis.
pub struct GenesisInitialization {
    pub header: BlockHeader,
    pub accounts: HashMap<Address, Account>,
    pub monetary: MonetaryState,
    pub validator_set: ValidatorSet,
}

/// Bangun state awal σ0 dan blok genesis kanonikal.
pub fn build_genesis(
    creator_addr: Address,
    developer_addr: Address,
    validators: Vec<ValidatorEntry>,
) -> GenesisInitialization {
    let mut accounts = HashMap::new();

    // 1. Alokasi Creator: 30% Hard Cap (19.800.000 AUR)
    accounts.insert(
        creator_addr,
        Account::new(Quantum::new(CREATOR_ALLOCATION_QUANTA), 0),
    );

    // 2. Alokasi Developer: 5% Hard Cap (3.300.000 AUR)
    accounts.insert(
        developer_addr,
        Account::new(Quantum::new(DEVELOPER_ALLOCATION_QUANTA), 0),
    );

    let total_genesis_allocated = CREATOR_ALLOCATION_QUANTA + DEVELOPER_ALLOCATION_QUANTA;
    let monetary = MonetaryState::new(Quantum::new(total_genesis_allocated), Quantum::ZERO);

    let validator_set = ValidatorSet::new(validators);

    let header = BlockHeader {
        version: 1,
        height: 0,
        round: 0,
        timestamp: GENESIS_TIMESTAMP,
        prev_block_hash: Hash256::ZERO,
        tx_merkle_root: Hash256::ZERO,
        state_root: Hash256::ZERO,
    };

    GenesisInitialization {
        header,
        accounts,
        monetary,
        validator_set,
    }
}
