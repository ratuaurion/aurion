#![forbid(unsafe_code)]

//! Transport abstraction for event-driven BFT consensus.
//!
//! The in-memory adapter is deliberately network-agnostic. A Zenoh adapter can
//! implement the same trait without coupling the reactor to a transport SDK.

use crate::codec::CanonicalEncode;
use crate::consensus::bft::block::{Block, BlockProposalEnvelope};
use crate::consensus::bft::vote::{Vote, VOTE_BYTES};
use crate::transaction::types::Transaction;
use std::sync::Arc;
use tokio::sync::broadcast;

pub const MAX_PROPOSAL_WIRE_BYTES: usize = 32 * 1024;
pub const MAX_TRANSACTION_WIRE_BYTES: usize = 24_764;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TransportError {
    #[error("proposal exceeds wire limit: {actual} > {max} bytes")]
    ProposalTooLarge { actual: usize, max: usize },
    #[error("transaction exceeds wire limit: {actual} > {max} bytes")]
    TransactionTooLarge { actual: usize, max: usize },
    #[error("vote has invalid canonical size: {actual} != {expected} bytes")]
    InvalidVoteSize { actual: usize, expected: usize },
    #[error("transport channel closed: {0}")]
    ChannelClosed(String),
    #[error("transport receiver lagged by {0} messages")]
    Lagged(u64),
    #[error("invalid transport payload: {0}")]
    InvalidPayload(String),
    #[error("Zenoh operation failed: {0}")]
    Zenoh(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsensusMessage {
    Proposal(BlockProposalEnvelope),
    Vote(Vote),
    Transaction {
        transaction: Transaction,
        sender_pubkey: [u8; 32],
    },
    /// Sinyal liveness view-change: validator meminta semua peer maju ke
    /// (height, round) karena proposer terpilih tidak menghasilkan proposal
    /// dalam batas waktu (AUR-ISSUE-011, round-advance yang direlay).
    RoundAdvance { height: u64, round: u64 },
    /// Blok terkomit yang disiarkan observer/validator; dipakai untuk
    /// catch-up near-tip tanpa menunggu kuorum 2-phase (AUR-ISSUE-011).
    CommittedBlock(Block),
}

#[allow(async_fn_in_trait)]
pub trait BftTransport: Send + Sync {
    async fn broadcast_proposal(
        &self,
        proposal: BlockProposalEnvelope,
    ) -> Result<(), TransportError>;
    async fn broadcast_vote(&self, vote: Vote) -> Result<(), TransportError>;
    async fn broadcast_transaction(
        &self,
        transaction: Transaction,
        sender_pubkey: [u8; 32],
    ) -> Result<(), TransportError>;
    async fn broadcast_round_advance(&self, height: u64, round: u64)
        -> Result<(), TransportError>;
    async fn recv(&mut self) -> Result<ConsensusMessage, TransportError>;
}

#[derive(Clone)]
pub struct InMemoryNetworkHub {
    sender: broadcast::Sender<(u32, ConsensusMessage)>,
}

impl InMemoryNetworkHub {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn connect(&self, validator_index: u32) -> InMemoryBftTransport {
        InMemoryBftTransport {
            validator_index,
            sender: self.sender.clone(),
            receiver: self.sender.subscribe(),
        }
    }

    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

pub struct InMemoryBftTransport {
    validator_index: u32,
    sender: broadcast::Sender<(u32, ConsensusMessage)>,
    receiver: broadcast::Receiver<(u32, ConsensusMessage)>,
}

impl InMemoryBftTransport {
    pub fn validator_index(&self) -> u32 {
        self.validator_index
    }
}

impl BftTransport for InMemoryBftTransport {
    async fn broadcast_proposal(
        &self,
        proposal: BlockProposalEnvelope,
    ) -> Result<(), TransportError> {
        let actual = proposal.to_canonical_bytes().len();
        if actual > MAX_PROPOSAL_WIRE_BYTES {
            return Err(TransportError::ProposalTooLarge {
                actual,
                max: MAX_PROPOSAL_WIRE_BYTES,
            });
        }

        self.sender
            .send((self.validator_index, ConsensusMessage::Proposal(proposal)))
            .map(|_| ())
            .map_err(|error| TransportError::ChannelClosed(error.to_string()))
    }

    async fn broadcast_vote(&self, vote: Vote) -> Result<(), TransportError> {
        let actual = vote.to_canonical_bytes().len();
        if actual != VOTE_BYTES {
            return Err(TransportError::InvalidVoteSize {
                actual,
                expected: VOTE_BYTES,
            });
        }

        self.sender
            .send((self.validator_index, ConsensusMessage::Vote(vote)))
            .map(|_| ())
            .map_err(|error| TransportError::ChannelClosed(error.to_string()))
    }

    async fn broadcast_transaction(
        &self,
        transaction: Transaction,
        sender_pubkey: [u8; 32],
    ) -> Result<(), TransportError> {
        let actual = transaction.to_canonical_bytes().len();
        if actual > MAX_TRANSACTION_WIRE_BYTES {
            return Err(TransportError::TransactionTooLarge {
                actual,
                max: MAX_TRANSACTION_WIRE_BYTES,
            });
        }

        self.sender
            .send((
                self.validator_index,
                ConsensusMessage::Transaction {
                    transaction,
                    sender_pubkey,
                },
            ))
            .map(|_| ())
            .map_err(|error| TransportError::ChannelClosed(error.to_string()))
    }

    async fn broadcast_round_advance(
        &self,
        height: u64,
        round: u64,
    ) -> Result<(), TransportError> {
        self.sender
            .send((
                self.validator_index,
                ConsensusMessage::RoundAdvance { height, round },
            ))
            .map(|_| ())
            .map_err(|error| TransportError::ChannelClosed(error.to_string()))
    }

    async fn recv(&mut self) -> Result<ConsensusMessage, TransportError> {
        loop {
            match self.receiver.recv().await {
                Ok((sender_index, message)) if sender_index != self.validator_index => {
                    return Ok(message);
                }
                Ok(_) => continue,
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    return Err(TransportError::Lagged(skipped));
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(TransportError::ChannelClosed(
                        "network hub closed".to_string(),
                    ));
                }
            }
        }
    }
}

pub type SharedInMemoryNetworkHub = Arc<InMemoryNetworkHub>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::bft::block::Block;
    use crate::consensus::bft::header::BlockHeader;
    use crate::core::Hash256;
    use crate::crypto::Keypair;

    fn empty_block() -> Block {
        Block::new(
            BlockHeader {
                version: 1,
                height: 1,
                round: 0,
                timestamp: 1,
                prev_block_hash: Hash256::ZERO,
                tx_merkle_root: Hash256::ZERO,
                state_root: Hash256::ZERO,
            },
            Vec::new(),
            None,
        )
    }

    #[tokio::test]
    async fn four_nodes_gossip_and_filter_self_echo() {
        let hub = InMemoryNetworkHub::new(128);
        let mut node0 = hub.connect(0);
        let mut node1 = hub.connect(1);
        let mut node2 = hub.connect(2);
        let mut node3 = hub.connect(3);
        let vote = Vote::new_signed(
            &Keypair::from_seed(&[0x11; 32]),
            crate::consensus::bft::PHASE_PRECOMMIT,
            1,
            0,
            Hash256::ZERO,
            0,
        )
        .expect("deterministic vote");

        node0.broadcast_vote(vote.clone()).await.unwrap();
        assert_eq!(node1.recv().await.unwrap(), ConsensusMessage::Vote(vote.clone()));
        assert_eq!(node2.recv().await.unwrap(), ConsensusMessage::Vote(vote.clone()));
        assert_eq!(node3.recv().await.unwrap(), ConsensusMessage::Vote(vote));

        tokio::select! {
            result = node0.recv() => panic!("self echo received: {result:?}"),
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => {}
        }
    }

    #[tokio::test]
    async fn canonical_proposal_is_forwarded() {
        let hub = InMemoryNetworkHub::new(8);
        let mut receiver = hub.connect(1);
        let sender = hub.connect(0);

        let proposal = crate::consensus::bft::block::BlockProposalEnvelope {
            block: empty_block(),
            proposer_index: 0,
            signature: [0u8; 64],
        };
        sender.broadcast_proposal(proposal).await.unwrap();
        assert!(matches!(
            receiver.recv().await.unwrap(),
            ConsensusMessage::Proposal(_)
        ));
    }
}
