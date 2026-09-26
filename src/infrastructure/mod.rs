#![forbid(unsafe_code)]

//! Domain Fungsional Layer-5 (L5) Global Distributed Infrastructure & Ecosystem Services.
//! Sesuai Invariant Dokumen Aturan Aplikasi 20 (20-L5-GLOBAL-INFRASTRUCTURE-BLUEPRINT.md).
//! Invariant Kunci:
//! - AUR-L5-ARCH-001: Non-Consensus Mandate (Pemisahan mutlak dari konsensus blok L1).
//! - AUR-L5-ARCH-002: Economic Security Anchoring (Jaminan terkunci & slashing terdesentralisasi).
//! - AUR-L5-PREC-001: Zero Floating-Point Mandate (Seluruh nilai moneter dan kuantitas wajib Quantum u128).
//! - AUR-L5-PREC-002: Exact Balance Conservation (Konservasi nilai mutlak pada kanal mikro-pembayaran).
//! - AUR-L5-DATA-001: Blake3 Content Addressing Mandate.
//! - AUR-L5-DATA-002: Zero-Fabrication Data Provenance (Atestasi kueri terikat state root L1).
//! - AUR-L5-SEC-001: Byzantine Infrastructure Resiliency (Toleransi kesalahan Bizantium >= 51%).
//! - AUR-L5-SEC-002: Cryptographic Agent Mandate (Batas pengeluaran dana & waktu kedaluwarsa agen AI).

pub mod agent;
pub mod compute;
pub mod da;
pub mod identity;
pub mod indexing;
pub mod m2m;
pub mod node;
pub mod payment;
pub mod relay;
pub mod slashing;
pub mod storage;
pub mod types;

// Re-export Kanonikal Simbol Utama L5
pub use agent::{AgentExecutive, AgentMandate, DelegatedAction, MandateId};
pub use compute::{ComputeEngine, ComputeJob, ComputeReceipt, ComputeTaskId, ZkComputeAttestation};
pub use da::{DasSample, DasSamplingClient, DataAvailabilityMatrix, DataAvailabilityRoot};
pub use identity::{ReputationEngine, SovereignDid, VerifiableCredential};
pub use indexing::{IndexingMesh, IndexingQuery, QueryAttestation, QueryId};
pub use m2m::{DeviceId, M2MClearingHouse, M2MContract, MeteredUsageReceipt, ServiceMetric};
pub use node::{NodeRegistrationRequest, NodeRegistry, UnbondingRecord};
pub use payment::{
    ChannelStatus, OffChainBalanceProof, PaymentChannelId, StreamingChannel, StreamingPaymentEngine,
};
pub use relay::{AntiDdosShield, DdosFilterDecision, EdgeRelayMesh, RelayPeer};
pub use slashing::{ArbitrationEngine, ArbitrationVerdict, FraudChallenge, ViolationType};
pub use storage::{
    compute_chunk_id, hash_storage_branch, ChunkId, ProofOfRetrievability, StorageGrid,
    StorageManifest,
};
pub use types::{
    compute_node_id, InfrastructureNodeId, NodeLifecycleStatus, NodeMetadata, NodeType,
    L5_CHALLENGE_WINDOW_SLOTS, L5_DEFAULT_MAX_AGENT_CAP_QUANTA, L5_MAX_STREAMING_CHANNELS,
    L5_MIN_NODE_COLLATERAL_QUANTA, L5_STORAGE_CHUNK_BYTES, L5_UNBONDING_DELAY_SLOTS,
};
