#![forbid(unsafe_code)]

//! Event-driven BFT reactor.
//!
//! The reactor owns consensus message handling but not ledger persistence.
//! Callers commit a returned, verified certificate through `BftEngine` and
//! their storage transaction.

use crate::consensus::bft::block::Block;
use crate::consensus::bft::certificate::{CommitCertificate, ValidatorSet};
use crate::consensus::bft::engine::{BftEngine, BftEngineError};
use crate::consensus::bft::transport::{BftTransport, ConsensusMessage, TransportError};
use crate::consensus::bft::vote::{Vote, PHASE_PRECOMMIT};
use crate::crypto::Keypair;
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use tokio::time::{self, Instant};

#[derive(Debug, Error)]
pub enum ReactorError {
    #[error("transport failure: {0}")]
    Transport(#[from] TransportError),
    #[error("invalid proposal: {0}")]
    InvalidProposal(String),
    #[error("invalid vote: {0}")]
    InvalidVote(String),
    #[error("consensus engine failure: {0}")]
    Engine(#[from] BftEngineError),
}

#[derive(Default)]
pub struct VoteAccumulator {
    votes: HashMap<(crate::core::Hash256, u64), Vec<Vote>>,
}

impl VoteAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_vote(&mut self, vote: Vote) -> usize {
        let entry = self.votes.entry((vote.block_hash, vote.round)).or_default();
        if !entry
            .iter()
            .any(|existing| existing.validator_index == vote.validator_index)
        {
            entry.push(vote);
        }
        entry.len()
    }

    pub fn votes_for(
        &self,
        block_hash: &crate::core::Hash256,
        round: u64,
    ) -> Option<&[Vote]> {
        self.votes.get(&(*block_hash, round)).map(Vec::as_slice)
    }

    pub fn prune_below_height(&mut self, _current_height: u64) {
        // Vote keys do not include height; callers clear the accumulator on
        // height advancement to avoid retaining stale rounds.
        self.votes.clear();
    }
}

pub struct BftReactor<T: BftTransport> {
    pub validator_index: u32,
    pub validator_set: ValidatorSet,
    pub transport: T,
    pub current_height: u64,
    pub current_round: u64,
    pub vote_accumulator: VoteAccumulator,
    pub round_timeout: Duration,
    pub pending_proposal: Option<Block>,
    engine: BftEngine,
}

impl<T: BftTransport> BftReactor<T> {
    pub fn new(
        validator_index: u32,
        keypair: Keypair,
        validator_set: ValidatorSet,
        transport: T,
        initial_height: u64,
        round_timeout: Duration,
    ) -> Self {
        Self {
            validator_index,
            validator_set,
            transport,
            current_height: initial_height,
            current_round: 0,
            vote_accumulator: VoteAccumulator::new(),
            round_timeout,
            pending_proposal: None,
            engine: BftEngine::new(Some(keypair), Some(validator_index)),
        }
    }

    pub fn quorum_threshold(&self) -> usize {
        self.validator_set.quorum_threshold() as usize
    }

    pub async fn step(&mut self) -> Result<Option<CommitCertificate>, ReactorError> {
        let deadline = Instant::now() + self.round_timeout;
        let timeout = time::sleep_until(deadline);
        tokio::pin!(timeout);

        tokio::select! {
            message = self.transport.recv() => {
                match message? {
                    ConsensusMessage::Proposal(block) => self.handle_proposal(block).await?,
                    ConsensusMessage::Vote(vote) => {
                        if let Some(certificate) = self.handle_vote(vote)? {
                            self.advance_height(certificate.height.saturating_add(1));
                            return Ok(Some(certificate));
                        }
                    }
                    ConsensusMessage::Transaction(_) => {}
                }
            }
            _ = &mut timeout => self.handle_round_timeout(),
        }

        Ok(None)
    }

    async fn handle_proposal(&mut self, block: Block) -> Result<(), ReactorError> {
        if block.header.height != self.current_height
            || block.header.round != self.current_round
        {
            return Ok(());
        }

        if self.pending_proposal.is_some() {
            return Ok(());
        }

        let block_hash = block.hash();
        let vote = self
            .engine
            .produce_precommit(block_hash, self.current_height, self.current_round)?;
        self.pending_proposal = Some(block);
        self.vote_accumulator.add_vote(vote.clone());
        self.transport.broadcast_vote(vote).await?;
        Ok(())
    }

    fn handle_vote(&mut self, vote: Vote) -> Result<Option<CommitCertificate>, ReactorError> {
        if vote.phase != PHASE_PRECOMMIT
            || vote.height != self.current_height
            || vote.round != self.current_round
        {
            return Ok(None);
        }

        vote.verify(&self.validator_set)
            .map_err(|error| ReactorError::InvalidVote(error.to_string()))?;
        let block_hash = vote.block_hash;
        self.vote_accumulator.add_vote(vote);

        if self
            .vote_accumulator
            .votes_for(&block_hash, self.current_round)
            .map_or(0, |votes| votes.len())
            < self.quorum_threshold()
        {
            return Ok(None);
        }

        let votes = self
            .vote_accumulator
            .votes_for(&block_hash, self.current_round)
            .map(ToOwned::to_owned)
            .ok_or_else(|| ReactorError::InvalidVote("quorum votes disappeared".into()))?;
        let certificate = self.engine.create_commit_certificate(
            &self.validator_set,
            block_hash,
            self.current_height,
            self.current_round,
            votes,
        )?;
        Ok(Some(certificate))
    }

    fn handle_round_timeout(&mut self) {
        self.current_round = self.current_round.saturating_add(1);
        self.pending_proposal = None;
        self.vote_accumulator.prune_below_height(self.current_height);
    }

    fn advance_height(&mut self, new_height: u64) {
        self.current_height = new_height;
        self.current_round = 0;
        self.pending_proposal = None;
        self.vote_accumulator.prune_below_height(new_height);
    }
}
