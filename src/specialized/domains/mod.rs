//! # Aurion Specialized Domain Adapters
//!
//! Implementations of specialized domain architectures:
//! - `dex`: Microsecond In-Memory Order-Book matching engine with batch commits
//! - `gaming`: High-Frequency Ephemeral Gaming sessions with final state settlement
//! - `privacy`: Zero-Knowledge Confidential Shielded Pool with nullifier anti-double-spend

pub mod dex;
pub mod gaming;
pub mod privacy;

pub use dex::{DexError, Order, OrderBook, OrderSide, OrderType, Trade};
pub use gaming::{GameAction, GameSession, GameSessionStatus, GameSettlementSummary, GamingError};
pub use privacy::{PrivacyError, ShieldedNote, ShieldedPool, ZkProof};
