//! Modul State On-Chain dan Transisi Keadaan Aurion.

pub mod account;
pub mod chain;
pub mod monetary;
pub mod smt;
pub mod stf;

pub use account::Account;
pub use chain::{ChainError, ChainLedger};
pub use monetary::{calculate_block_subsidy, MonetaryState};
pub use smt::{compute_accounts_state_root, smt_branch_hash, smt_leaf_hash};
pub use stf::{apply_transaction, StateTransitionError};

