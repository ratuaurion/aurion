//! # Aurion L3 Specialized Domain: High-Frequency Ephemeral Gaming
//!
//! Sub-millisecond state transitions for ephemeral gaming sessions with deterministic action
//! sequence commits and final state settlement to L3 state trees and L2 settlement contracts.
//!
//! Conforms strictly to:
//! - AUR-ARCH-011: Zero unsafe code (`#![forbid(unsafe_code)]`)
//! - AUR-ARCH-012: Zero floating-point arithmetic (`Quantum(u128)`)
//! - AUR-L3-ARCH-002: Ephemeral high-frequency game loop with final settlement commit

use std::collections::BTreeMap;
use blake3::Hasher;

use crate::primitives::core::Quantum;
use crate::specialized::types::DomainId;

/// Operational status of an ephemeral gaming session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameSessionStatus {
    /// Session is ongoing and accepting player actions.
    Active,
    /// Session concluded normally and state is finalized.
    Completed,
    /// Session was aborted (e.g. timeout / disconnect), stakes refunded.
    Aborted,
}

/// An individual user action or state change within a gaming session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameAction {
    /// Player identifier who performed the action.
    pub player: [u8; 32],
    /// Domain-specific action type identifier.
    pub action_type: u16,
    /// Compact action payload bytes.
    pub payload: Vec<u8>,
    /// Incremental score delta.
    pub score_delta: u64,
    /// Monotonic action sequence counter.
    pub sequence: u64,
}

/// Summary of a finalized game session ready for L3/L2 settlement commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSettlementSummary {
    /// Session unique identifier.
    pub session_id: [u8; 32],
    /// Domain under which the session ran.
    pub domain_id: DomainId,
    /// Declared winner of the match.
    pub winner: [u8; 32],
    /// Total prize payout in Quantum.
    pub payout: Quantum,
    /// Total actions processed during the session.
    pub total_actions: u64,
    /// Deterministic Blake3 cryptographic digest of the complete session state & actions.
    pub final_state_hash: [u8; 32],
}

/// Specialized gaming domain operational errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GamingError {
    /// Session is not in active state.
    SessionNotActive,
    /// Player is not registered in this session.
    UnregisteredPlayer([u8; 32]),
    /// Action sequence is out of order.
    InvalidSequence { expected: u64, got: u64 },
    /// No players provided for session creation.
    EmptyPlayerList,
    /// Winner is not among the session participants.
    InvalidWinner([u8; 32]),
    /// Arithmetic overflow in score or stake calculations.
    ArithmeticOverflow,
}

/// Ephemeral game session state manager.
#[derive(Debug, Clone)]
pub struct GameSession {
    /// Unique session identifier.
    pub session_id: [u8; 32],
    /// Domain identifier.
    pub domain_id: DomainId,
    /// Registered players in this match.
    pub players: Vec<[u8; 32]>,
    /// Locked stake per player in Quantum.
    pub stakes: BTreeMap<[u8; 32], Quantum>,
    /// Current accumulated score per player.
    pub scores: BTreeMap<[u8; 32], u64>,
    /// Current session status.
    pub status: GameSessionStatus,
    /// Total actions processed so far.
    pub action_count: u64,
    /// Rolling cryptographic state hash.
    pub rolling_state_hash: [u8; 32],
}

impl GameSession {
    /// Creates a new gaming session with locked entry stakes per player.
    pub fn new(
        session_id: [u8; 32],
        domain_id: DomainId,
        players: Vec<[u8; 32]>,
        stake_per_player: Quantum,
    ) -> Result<Self, GamingError> {
        if players.is_empty() {
            return Err(GamingError::EmptyPlayerList);
        }

        let mut stakes = BTreeMap::new();
        let mut scores = BTreeMap::new();

        for &player in &players {
            stakes.insert(player, stake_per_player);
            scores.insert(player, 0);
        }

        let mut hasher = Hasher::new();
        hasher.update(b"AURION_L3_GAME_SESSION_INIT_V1");
        hasher.update(&session_id);
        hasher.update(domain_id.as_bytes());
        hasher.update(&(players.len() as u64).to_be_bytes());
        hasher.update(&stake_per_player.as_u128().to_be_bytes());
        let initial_hash = *hasher.finalize().as_bytes();

        Ok(Self {
            session_id,
            domain_id,
            players,
            stakes,
            scores,
            status: GameSessionStatus::Active,
            action_count: 0,
            rolling_state_hash: initial_hash,
        })
    }

    /// Submits and applies a high-frequency game action, updating rolling state hash and score.
    pub fn apply_action(&mut self, action: GameAction) -> Result<[u8; 32], GamingError> {
        if self.status != GameSessionStatus::Active {
            return Err(GamingError::SessionNotActive);
        }

        if !self.players.contains(&action.player) {
            return Err(GamingError::UnregisteredPlayer(action.player));
        }

        let expected_seq = self.action_count.saturating_add(1);
        if action.sequence != expected_seq {
            return Err(GamingError::InvalidSequence {
                expected: expected_seq,
                got: action.sequence,
            });
        }

        // Update score
        if let Some(score) = self.scores.get_mut(&action.player) {
            *score = score
                .checked_add(action.score_delta)
                .ok_or(GamingError::ArithmeticOverflow)?;
        }

        // Advance sequence & rolling state hash
        self.action_count = expected_seq;

        let mut hasher = Hasher::new();
        hasher.update(b"AURION_L3_GAME_ACTION_STEP_V1");
        hasher.update(&self.rolling_state_hash);
        hasher.update(&action.player);
        hasher.update(&action.action_type.to_be_bytes());
        hasher.update(&(action.payload.len() as u64).to_be_bytes());
        hasher.update(&action.payload);
        hasher.update(&action.score_delta.to_be_bytes());
        hasher.update(&action.sequence.to_be_bytes());

        self.rolling_state_hash = *hasher.finalize().as_bytes();
        Ok(self.rolling_state_hash)
    }

    /// Finalizes the game session, declares a winner, and generates the settlement summary.
    pub fn finalize_session(&mut self, winner: [u8; 32]) -> Result<GameSettlementSummary, GamingError> {
        if self.status != GameSessionStatus::Active {
            return Err(GamingError::SessionNotActive);
        }

        if !self.players.contains(&winner) {
            return Err(GamingError::InvalidWinner(winner));
        }

        // Calculate total payout pot: sum of all stakes
        let mut total_payout = Quantum(0);
        for stake in self.stakes.values() {
            total_payout = total_payout
                .checked_add(*stake)
                .map_err(|_| GamingError::ArithmeticOverflow)?;
        }

        self.status = GameSessionStatus::Completed;

        let mut hasher = Hasher::new();
        hasher.update(b"AURION_L3_GAME_SESSION_FINAL_V1");
        hasher.update(&self.session_id);
        hasher.update(&self.rolling_state_hash);
        hasher.update(&winner);
        hasher.update(&total_payout.as_u128().to_be_bytes());
        hasher.update(&self.action_count.to_be_bytes());
        let final_state_hash = *hasher.finalize().as_bytes();

        Ok(GameSettlementSummary {
            session_id: self.session_id,
            domain_id: self.domain_id,
            winner,
            payout: total_payout,
            total_actions: self.action_count,
            final_state_hash,
        })
    }

    /// Aborts an active session and allows stakes to be refunded.
    pub fn abort_session(&mut self) -> Result<(), GamingError> {
        if self.status != GameSessionStatus::Active {
            return Err(GamingError::SessionNotActive);
        }
        self.status = GameSessionStatus::Aborted;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ephemeral_gaming_session_lifecycle_and_settlement() {
        let domain_id = DomainId::named("game-arena-fast");
        let session_id = [0x55u8; 32];
        let p1 = [1u8; 32];
        let p2 = [2u8; 32];
        let players = vec![p1, p2];
        let stake = Quantum(500);

        let mut session = GameSession::new(session_id, domain_id, players, stake)
            .expect("init game session");

        assert_eq!(session.status, GameSessionStatus::Active);
        assert_eq!(session.action_count, 0);

        // Player 1 performs action 1
        let a1 = GameAction {
            player: p1,
            action_type: 10,
            payload: vec![1, 2, 3],
            score_delta: 25,
            sequence: 1,
        };
        let hash1 = session.apply_action(a1).expect("action 1");
        assert_ne!(hash1, [0u8; 32]);
        assert_eq!(session.scores.get(&p1), Some(&25));

        // Player 2 performs action 2
        let a2 = GameAction {
            player: p2,
            action_type: 10,
            payload: vec![4, 5, 6],
            score_delta: 50,
            sequence: 2,
        };
        let hash2 = session.apply_action(a2).expect("action 2");
        assert_ne!(hash2, hash1);
        assert_eq!(session.scores.get(&p2), Some(&50));

        // Finalize match, P2 wins
        let summary = session.finalize_session(p2).expect("finalize session");
        assert_eq!(summary.winner, p2);
        assert_eq!(summary.payout, Quantum(1000)); // 500 * 2
        assert_eq!(summary.total_actions, 2);
        assert_eq!(session.status, GameSessionStatus::Completed);

        // Cannot apply action to completed session
        let a3 = GameAction {
            player: p1,
            action_type: 10,
            payload: vec![],
            score_delta: 5,
            sequence: 3,
        };
        assert_eq!(session.apply_action(a3), Err(GamingError::SessionNotActive));
    }

    #[test]
    fn test_ephemeral_gaming_session_abort() {
        let domain_id = DomainId::named("game-arena-fast");
        let session_id = [0x77u8; 32];
        let p1 = [1u8; 32];
        let mut session = GameSession::new(session_id, domain_id, vec![p1], Quantum(100))
            .expect("init");

        session.abort_session().expect("abort");
        assert_eq!(session.status, GameSessionStatus::Aborted);
    }
}
