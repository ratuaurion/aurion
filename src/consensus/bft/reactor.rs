#![forbid(unsafe_code)]

//! Event-driven two-phase BFT reactor.

use crate::consensus::bft::block::Block;
use crate::consensus::bft::block::BlockProposalEnvelope;
use crate::consensus::bft::certificate::{CommitCertificate, ValidatorSet};
use crate::consensus::bft::engine::{BftEngine, BftEngineError};
use crate::consensus::bft::transport::{BftTransport, ConsensusMessage, TransportError};
use crate::consensus::bft::vote::{Vote, PHASE_PRECOMMIT, PHASE_PREVOTE};
use crate::crypto::Keypair;
use crate::genesis::builder::GENESIS_CHAIN_ID;
use crate::state::chain::{ChainError, ChainLedger};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::time::{self, Instant};

pub const MAX_ROUND_DRIFT: u64 = 10;

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
    #[error("ledger failure: {0}")]
    Ledger(#[from] ChainError),
    #[error("reactor requires a ledger-aware constructor before commit")]
    MissingLedger,
}

#[derive(Default)]
pub struct VoteAccumulator {
    votes: HashMap<(crate::core::Hash256, u64, u8), Vec<Vote>>,
    vote_slots: HashMap<(u64, u64, u8, u32), crate::core::Hash256>,
}

impl VoteAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_vote(&mut self, vote: Vote) -> usize {
        let key = (vote.block_hash, vote.round, vote.phase);
        let entry = self.votes.entry(key).or_default();
        if !entry
            .iter()
            .any(|existing| existing.validator_index == vote.validator_index)
        {
            entry.push(vote);
        }
        entry.len()
    }

    pub fn add_vote_checked(&mut self, vote: Vote) -> Result<usize, ReactorError> {
        let slot = (vote.height, vote.round, vote.phase, vote.validator_index);
        if let Some(existing_hash) = self.vote_slots.get(&slot) {
            if *existing_hash != vote.block_hash {
                return Err(ReactorError::InvalidVote(format!(
                    "equivocation from validator {} at height {}, round {}, phase {:#04x}",
                    vote.validator_index, vote.height, vote.round, vote.phase
                )));
            }
            return Ok(self
                .votes
                .get(&(vote.block_hash, vote.round, vote.phase))
                .map_or(0, Vec::len));
        }

        let key = (vote.block_hash, vote.round, vote.phase);
        let entry = self.votes.entry(key).or_default();
        self.vote_slots.insert(slot, vote.block_hash);
        entry.push(vote);
        Ok(entry.len())
    }

    fn is_duplicate(&self, vote: &Vote) -> Result<bool, ReactorError> {
        let slot = (vote.height, vote.round, vote.phase, vote.validator_index);
        match self.vote_slots.get(&slot) {
            Some(existing_hash) if *existing_hash == vote.block_hash => Ok(true),
            Some(_) => Err(ReactorError::InvalidVote(format!(
                "equivocation from validator {} at height {}, round {}, phase {:#04x}",
                vote.validator_index, vote.height, vote.round, vote.phase
            ))),
            None => Ok(false),
        }
    }

    pub fn votes_for(
        &self,
        block_hash: &crate::core::Hash256,
        round: u64,
        phase: u8,
    ) -> Option<&[Vote]> {
        self.votes
            .get(&(*block_hash, round, phase))
            .map(Vec::as_slice)
    }

    pub fn prune_below_height(&mut self, _current_height: u64) {
        self.votes.clear();
        self.vote_slots.clear();
    }
}

pub struct BftReactor<T: BftTransport> {
    pub validator_index: u32,
    pub validator_set: ValidatorSet,
    pub transport: T,
    pub current_height: u64,
    pub current_round: u64,
    pending_transactions: Vec<(crate::transaction::types::Transaction, [u8; 32])>,
    pub vote_accumulator: VoteAccumulator,
    pub round_timeout: Duration,
    pub pending_proposal: Option<Block>,
    pub pending_proposer_index: Option<u32>,
    engine: BftEngine,
    ledger: Option<Arc<Mutex<ChainLedger>>>,
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
        Self::new_internal(
            validator_index,
            keypair,
            validator_set,
            transport,
            initial_height,
            round_timeout,
            None,
        )
    }

    pub fn new_with_ledger(
        validator_index: u32,
        keypair: Keypair,
        validator_set: ValidatorSet,
        transport: T,
        ledger: Arc<Mutex<ChainLedger>>,
        round_timeout: Duration,
    ) -> Self {
        let initial_height = ledger
            .lock()
            .map(|guard| guard.latest_height() + 1)
            .unwrap_or(0);
        Self::new_internal(
            validator_index,
            keypair,
            validator_set,
            transport,
            initial_height,
            round_timeout,
            Some(ledger),
        )
    }

    fn new_internal(
        validator_index: u32,
        keypair: Keypair,
        validator_set: ValidatorSet,
        transport: T,
        initial_height: u64,
        round_timeout: Duration,
        ledger: Option<Arc<Mutex<ChainLedger>>>,
    ) -> Self {
        Self {
            validator_index,
            validator_set,
            transport,
            current_height: initial_height,
            current_round: 0,
            pending_transactions: Vec::new(),
            vote_accumulator: VoteAccumulator::new(),
            round_timeout,
            pending_proposal: None,
            pending_proposer_index: None,
            engine: BftEngine::new(Some(keypair), Some(validator_index)),
            ledger,
        }
    }

    pub fn quorum_threshold(&self) -> usize {
        self.validator_set.quorum_threshold() as usize
    }

    pub async fn propose_local(
        &mut self,
        envelope: BlockProposalEnvelope,
    ) -> Result<(), ReactorError> {
        self.handle_proposal(envelope).await
    }

    pub async fn step(&mut self) -> Result<Option<CommitCertificate>, ReactorError> {
        let deadline = Instant::now() + self.round_timeout;
        let timeout = time::sleep_until(deadline);
        tokio::pin!(timeout);

        tokio::select! {
            message = self.transport.recv() => {
                match message? {
                    ConsensusMessage::Proposal(envelope) => self.handle_proposal(envelope).await?,
                    ConsensusMessage::Vote(vote) => {
                        if let Some(certificate) = self.handle_vote(vote).await? {
                            self.advance_height(certificate.height.saturating_add(1));
                            return Ok(Some(certificate));
                        }
                    }
                    ConsensusMessage::Transaction {
                        transaction,
                        sender_pubkey,
                    } => self.pending_transactions.push((transaction, sender_pubkey)),
                }

            }
            _ = &mut timeout => self.handle_round_timeout(),
        }
        Ok(None)
    }

    pub fn drain_transactions(
        &mut self,
    ) -> Vec<(crate::transaction::types::Transaction, [u8; 32])> {
        std::mem::take(&mut self.pending_transactions)
    }

    async fn handle_proposal(
        &mut self,
        envelope: BlockProposalEnvelope,
    ) -> Result<(), ReactorError> {
        let block = &envelope.block;
        if block.header.height != self.current_height || block.header.round < self.current_round {
            return Ok(());
        }
        if self.pending_proposal.as_ref().is_some_and(|pending| {
            pending.hash() == block.hash() && pending.header.round == block.header.round
        }) {
            return Ok(());
        }
        if block.header.round.saturating_sub(self.current_round) > MAX_ROUND_DRIFT {
            return Err(ReactorError::InvalidProposal(format!(
                "proposal round {} exceeds local round {} by more than {}",
                block.header.round, self.current_round, MAX_ROUND_DRIFT
            )));
        }
        envelope
            .verify(GENESIS_CHAIN_ID, &self.validator_set)
            .map_err(|error| ReactorError::InvalidProposal(error.to_string()))?;

        if block.header.round > self.current_round {
            self.current_round = block.header.round;
            self.vote_accumulator.prune_below_height(self.current_height);
            self.pending_proposal = None;
            self.pending_proposer_index = None;
        }

        let ledger = self.ledger.as_ref().ok_or(ReactorError::MissingLedger)?;
        let proposer = self
            .validator_set
            .get_validator(envelope.proposer_index)
            .ok_or_else(|| ReactorError::InvalidProposal("unknown proposer".into()))?
            .validator_id;
        ledger
            .lock()
            .map_err(|_| ReactorError::InvalidProposal("ledger lock poisoned".into()))?
            .validate_block_proposal(block, &proposer)
            .map_err(|error| ReactorError::InvalidProposal(error.to_string()))?;

        if self.pending_proposal.is_some() {
            return Ok(());
        }
        self.pending_proposal = Some(block.clone());
        self.pending_proposer_index = Some(envelope.proposer_index);
        let vote =
            self.engine
                .produce_prevote(block.hash(), self.current_height, self.current_round)?;
        self.vote_accumulator.add_vote_checked(vote.clone())?;
        self.transport.broadcast_vote(vote).await?;
        Ok(())
    }

    async fn handle_vote(&mut self, vote: Vote) -> Result<Option<CommitCertificate>, ReactorError> {
        if vote.height != self.current_height
            || vote.round != self.current_round
            || (vote.phase != PHASE_PREVOTE && vote.phase != PHASE_PRECOMMIT)
        {
            return Ok(None);
        }
        if self.vote_accumulator.is_duplicate(&vote)? {
            return Ok(None);
        }
        vote.verify(&self.validator_set)
            .map_err(|error| ReactorError::InvalidVote(error.to_string()))?;
        let block_hash = vote.block_hash;
        self.vote_accumulator.add_vote_checked(vote)?;

        if self
            .vote_accumulator
            .votes_for(&block_hash, self.current_round, PHASE_PREVOTE)
            .is_some_and(|votes| self.has_quorum(votes))
            && self
                .vote_accumulator
                .votes_for(&block_hash, self.current_round, PHASE_PRECOMMIT)
                .is_none()
        {
            let precommit = self.engine.produce_precommit(
                block_hash,
                self.current_height,
                self.current_round,
            )?;
            self.vote_accumulator.add_vote_checked(precommit.clone())?;
            self.transport.broadcast_vote(precommit).await?;
        }

        let votes =
            match self
                .vote_accumulator
                .votes_for(&block_hash, self.current_round, PHASE_PRECOMMIT)
            {
                Some(votes) if self.has_quorum(votes) => votes.to_vec(),
                _ => return Ok(None),
            };
        let certificate = self.engine.create_commit_certificate(
            &self.validator_set,
            block_hash,
            self.current_height,
            self.current_round,
            votes,
        )?;
        let mut block = match self.pending_proposal.clone() {
            Some(b) => b,
            None => return Ok(None),
        };
        block.commit_certificate = Some(certificate.clone());
        let ledger = self.ledger.as_ref().ok_or(ReactorError::MissingLedger)?;
        let proposer_index = match self.pending_proposer_index {
            Some(p) => p,
            None => return Ok(None),
        };
        let proposer = self
            .validator_set
            .get_validator(proposer_index)
            .ok_or_else(|| ReactorError::InvalidVote("unknown local validator".into()))?
            .validator_id;
        ledger
            .lock()
            .map_err(|_| ReactorError::InvalidProposal("ledger lock poisoned".into()))?
            .apply_block(block, &proposer)?;
        Ok(Some(certificate))
    }

    fn has_quorum(&self, votes: &[Vote]) -> bool {
        let voting_power = votes
            .iter()
            .filter_map(|vote| {
                self.validator_set
                    .get_validator(vote.validator_index)
                    .map(|validator| validator.voting_weight)
            })
            .sum::<u64>();
        voting_power >= self.validator_set.quorum_threshold()
    }

    fn handle_round_timeout(&mut self) {
        self.current_round = self.current_round.saturating_add(1);
        self.pending_proposal = None;
        self.pending_proposer_index = None;
        self.vote_accumulator
            .prune_below_height(self.current_height);
    }

    fn advance_height(&mut self, new_height: u64) {
        self.current_height = new_height;
        self.current_round = 0;
        self.pending_proposal = None;
        self.pending_proposer_index = None;
        self.vote_accumulator.prune_below_height(new_height);
    }
}
