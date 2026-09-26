#![forbid(unsafe_code)]

//! Modul Aurion Layer-3 Specialized Networks (`aurion::specialized`).
//! Mematuhi Invariant Rule 18 (L3 Ecosystem Expansion Blueprint),
//! AUR-ARCH-001 (Single Sovereign Ecosystem), AUR-ARCH-011 (#![forbid(unsafe_code)]),
//! AUR-ARCH-012 (Zero-Float Quantum u128), AUR-L3-ARCH-001 (L1 Sovereignty Root),
//! dan AUR-L3-SEC-001 (Domain Fault Isolation).

pub mod domains;
pub mod messaging;
pub mod runtime;
pub mod settlement;
pub mod state;
pub mod types;

pub use domains::{
    DexError, GameAction, GameSession, GameSessionStatus, GameSettlementSummary, GamingError,
    Order, OrderBook, OrderSide, OrderType, PrivacyError, ShieldedNote, ShieldedPool, Trade,
    ZkProof,
};

pub use messaging::{
    CrossDomainEventRouter, CrossLayerMessage, L2L3TwoWayRelayer, L3MessagingError,
    L3WithdrawalProof, NullifierRegistry, DST_L3_NULLIFIER,
};
pub use runtime::{
    calculate_l3_fee_split, calculate_l3_required_gas, L3ExecutionConfig, L3ExecutionEngine,
    L3ExecutionError, L3FeeSplit, L3_BASE_TX_GAS, L3_DEFAULT_MAX_GAS_PER_BLOCK,
    L3_FEE_SPLIT_SEQUENCER_PERCENT, L3_FEE_SPLIT_SETTLEMENT_PERCENT, L3_GAS_PER_PAYLOAD_BYTE,
    L3_MIN_GAS_PRICE_QUANTA,
};
pub use settlement::{
    L2SettlementClient, L3CheckpointGenerator, L3FinalityStatus, L3FinalityTier, L3SettlementError,
};
pub use state::{
    L3AccountProof, L3AccountState, L3State, L3StateSnapshot, L3_ACCOUNT_ENCODED_SIZE,
};
pub use types::{
    DomainId, DomainMetadata, L3Block, L3Checkpoint, L3CodecError, L3Receipt, L3SecurityModel,
    L3Transaction, DST_L3_CHECKPOINT, DST_L3_TX, L3_BLOCK_HEADER_SIZE, L3_CHECKPOINT_BASE_SIZE,
    L3_RECEIPT_BASE_SIZE, L3_TX_BASE_SIZE, MAX_L3_PROOF_SIZE, MAX_L3_TX_PAYLOAD_SIZE,
};
