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
    MonetaryError, Quantum, BLOCK_REWARD_PROPOSER_PERCENT, BLOCK_REWARD_QUANTA,
    BLOCK_REWARD_VOTERS_PERCENT, FEE_BURN_PERCENTAGE, FEE_VALIDATOR_PERCENTAGE,
    MASTER_TREASURY_ALLOCATION_QUANTA, MAX_SUPPLY_AUR, MAX_SUPPLY_QUANTA,
    QUANTA_PER_AUR, TARGET_BLOCK_TIME_SECONDS,
};
pub use signature::Signature;
