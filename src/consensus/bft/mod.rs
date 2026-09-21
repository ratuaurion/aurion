//! Modul Konsensus BFT Single-Slot Finality Aurion.

pub mod block;
pub mod certificate;
pub mod engine;
pub mod epoch;
pub mod governance;
pub mod header;
pub mod reactor;
pub mod transport;
pub mod vote;
pub mod zenoh;

pub use block::{Block, BlockProposalEnvelope, ProposalEnvelopeError, DST_MERKLE_BRANCH};
pub use certificate::{
    CertificateError, CommitCertificate, ValidatorEntry, ValidatorSet, VALIDATOR_ENTRY_BYTES,
};
pub use engine::{BftEngine, BftEngineError};
pub use epoch::{
    compute_epoch_id, is_epoch_boundary, resolve_epoch_blocks, rotate_validator_set,
    EpochError, EpochInfo, EpochTransition, CANONICAL_EPOCH_BLOCKS, DEFAULT_EPOCH_BLOCKS,
    DEV_EPOCH_BLOCKS,
};
pub use governance::{GovernanceEngine, ProposalStatus, ProposalSummary, UpgradeProposal};
pub use header::{BlockHeader, BLOCK_HEADER_BYTES};
pub use reactor::{BftReactor, ReactorError, VoteAccumulator};
pub use transport::{
    BftTransport, ConsensusMessage, InMemoryBftTransport, InMemoryNetworkHub,
    MAX_PROPOSAL_WIRE_BYTES, MAX_TRANSACTION_WIRE_BYTES, TransportError,
};
pub use zenoh::{publish_committed_block, ZenohBftObserver, ZenohBftTransport};
pub use vote::{Vote, VoteError, PHASE_PRECOMMIT, PHASE_PREVOTE, VOTE_BYTES};
