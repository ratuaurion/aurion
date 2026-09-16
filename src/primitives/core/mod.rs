//! Modul Core: Tipe primitif bersama Aurion (Shared Core).

pub mod address;
pub mod errors;
pub mod hash;
pub mod quantum;
pub mod signature;

pub use address::Address;
pub use errors::AurionError;
pub use hash::Hash256;
pub use quantum::{
    MonetaryError, Quantum, COMMUNITY_MINING_QUANTA, CREATOR_ALLOCATION_QUANTA,
    DEVELOPER_ALLOCATION_QUANTA, FEE_BURN_PERCENTAGE, FEE_MINER_PERCENTAGE,
    HALVING_INTERVAL_BLOCKS, INITIAL_BLOCK_SUBSIDY_QUANTA, MAX_HALVING_ERAS,
    MAX_SUPPLY_AUR, MAX_SUPPLY_QUANTA, QUANTA_PER_AUR, TARGET_BLOCK_TIME_SECONDS,
};
pub use signature::Signature;
