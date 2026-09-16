#![forbid(unsafe_code)]

//! Aurion Layer-4 (L4) Interoperability Domain: Sovereign Cross-Domain Ecosystem.
//!
//! Submodules:
//! - `types`: Core data primitives, chain IDs, and 8-field universal message envelope.
//! - `codec`: Deterministic canonical big-endian wire envelope codec ("AUL4").
//! - `verifier`: Trust-minimized light client & ZK state proof verifiers.
//! - `relayer`: Trust-minimized cross-chain message relayer.
//! - `vault`: Cross-chain asset bridge & TSS custody vault.
//! - `messaging`: Decentralized state read relay, identity resolver, nullifier registry.
//! - `security`: Multi-prover engine, financial rate limiter, circuit breaker.

pub mod codec;
pub mod messaging;
pub mod relayer;
pub mod security;
pub mod types;
pub mod vault;
pub mod verifier;

// Re-exports for convenient public access
pub use codec::{decode_envelope, encode_envelope, L4_HEADER_BYTES, L4_WIRE_MAGIC};
pub use messaging::{
    CrossDomainIdentityBinding, DecentralizedStateReadRelay, SovereignIdentityResolver,
    StateReadQuery, StateReadResponse, UniversalNullifierRegistry,
};
pub use relayer::{RelayVerificationResult, TrustMinimizedRelayer};
pub use security::{
    AnomalyEvent, AnomalySeverity, BridgeCircuitBreaker, CircuitStatus, FinancialRateLimiter,
    L4SecurityGate, MultiProverEngine, MultiProverResult, ProverId, ProverVerdict,
};
pub use types::{
    BridgeStatus, ChainId, CrossChainMessage, CrossChainMessageParams, MAX_L4_PAYLOAD_BYTES,
    ProofPayload, ProtocolId, RouteDescriptor,
};
pub use vault::{
    CrossChainAssetVault, ThresholdCustodyAdapter, VaultActionType, VaultRecord,
};
pub use verifier::{
    BitcoinSpvVerifier, EvmStateVerifier, ExternalHeaderEntry, HeaderSyncTracker,
    ZkStateProofVerifier,
};
