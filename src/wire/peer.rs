//! Sistem Manajemen Reputasi Peer, Skor Anti-DoS, dan Pembatasan Laju (Rate Limiting).
//! Mematuhi Dokumen 06 (AURION-SERIALIZATION-AND-WIRE-PROTOCOL.md Bagian 9).

use crate::core::Address;
use std::net::SocketAddr;

pub const INITIAL_PEER_SCORE: i32 = 100;
pub const THROTTLE_SCORE_THRESHOLD: i32 = 50;
pub const BAN_SCORE_THRESHOLD: i32 = 0;
pub const BAN_DURATION_SECS: u64 = 86_400; // 24 Jam

pub const MAX_TX_MSGS_PER_SEC: u32 = 200;
pub const MAX_SYNC_MSGS_PER_SEC: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    Connecting,
    Handshaking,
    Connected,
    Throttled,
    Banned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerViolation {
    InvalidMagic,
    ChecksumMismatch,
    ForgedSignature,
    InvalidOrDoubleSpendTx,
    FalseEquivocationProof,
    RateLimitSpam,
    PayloadExceedsCeiling,
    InvalidBlockProposal,
}

impl PeerViolation {
    pub fn penalty_score(&self) -> i32 {
        match self {
            PeerViolation::InvalidMagic => 100,
            PeerViolation::ChecksumMismatch => 50,
            PeerViolation::ForgedSignature => 40,
            PeerViolation::InvalidOrDoubleSpendTx => 20,
            PeerViolation::FalseEquivocationProof => 100,
            PeerViolation::RateLimitSpam => 30,
            PeerViolation::PayloadExceedsCeiling => 100,
            PeerViolation::InvalidBlockProposal => 80,
        }
    }
}

/// Token bucket rate limiter sederhana untuk proteksi DoS jaringan.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    pub max_tokens: u32,
    pub available_tokens: u32,
    pub refill_rate_per_sec: u32,
    pub last_refill_timestamp: u64,
}

impl TokenBucket {
    pub fn new(rate_per_sec: u32, current_time: u64) -> Self {
        Self {
            max_tokens: rate_per_sec,
            available_tokens: rate_per_sec,
            refill_rate_per_sec: rate_per_sec,
            last_refill_timestamp: current_time,
        }
    }

    pub fn try_consume(&mut self, current_time: u64) -> bool {
        self.refill(current_time);
        if self.available_tokens > 0 {
            self.available_tokens -= 1;
            true
        } else {
            false
        }
    }

    fn refill(&mut self, current_time: u64) {
        if current_time > self.last_refill_timestamp {
            let elapsed_secs = current_time - self.last_refill_timestamp;
            let added_tokens = (elapsed_secs as u32).saturating_mul(self.refill_rate_per_sec);
            self.available_tokens = (self.available_tokens + added_tokens).min(self.max_tokens);
            self.last_refill_timestamp = current_time;
        }
    }
}

/// Objek pelacak reputasi dan koneksi sebuah simpul peer tetangga.
#[derive(Debug, Clone)]
pub struct PeerRecord {
    pub addr: SocketAddr,
    pub node_id: Option<Address>,
    pub public_key: Option<[u8; 32]>,
    pub score: i32,
    pub state: PeerState,
    pub banned_until: Option<u64>,
    pub best_height: u64,
    pub tx_rate_limiter: TokenBucket,
    pub sync_rate_limiter: TokenBucket,
}

impl PeerRecord {
    pub fn new(addr: SocketAddr, current_time: u64) -> Self {
        Self {
            addr,
            node_id: None,
            public_key: None,
            score: INITIAL_PEER_SCORE,
            state: PeerState::Connecting,
            banned_until: None,
            best_height: 0,
            tx_rate_limiter: TokenBucket::new(MAX_TX_MSGS_PER_SEC, current_time),
            sync_rate_limiter: TokenBucket::new(MAX_SYNC_MSGS_PER_SEC, current_time),
        }
    }

    /// Terapkan penalti deterministik sesuai matriks pelanggaran Dokumen 06.
    pub fn apply_penalty(&mut self, violation: PeerViolation, current_time: u64) {
        let penalty = violation.penalty_score();
        self.score = self.score.saturating_sub(penalty);

        if self.score <= BAN_SCORE_THRESHOLD {
            self.state = PeerState::Banned;
            self.banned_until = Some(current_time + BAN_DURATION_SECS);
        } else if self.score <= THROTTLE_SCORE_THRESHOLD {
            self.state = PeerState::Throttled;
        }
    }

    /// Evaluasi apakah peer masih dalam masa cekal (banned).
    pub fn is_banned(&self, current_time: u64) -> bool {
        if self.state == PeerState::Banned {
            if let Some(until) = self.banned_until {
                return current_time < until;
            }
            return true;
        }
        false
    }

    /// Periksa batas laju pesan transaksi.
    pub fn allow_transaction_message(&mut self, current_time: u64) -> bool {
        if self.is_banned(current_time) {
            return false;
        }
        let allowed = self.tx_rate_limiter.try_consume(current_time);
        if !allowed {
            self.apply_penalty(PeerViolation::RateLimitSpam, current_time);
        }
        allowed
    }

    /// Periksa batas laju pesan sinkronisasi blok.
    pub fn allow_sync_message(&mut self, current_time: u64) -> bool {
        if self.is_banned(current_time) {
            return false;
        }
        let allowed = self.sync_rate_limiter.try_consume(current_time);
        if !allowed {
            self.apply_penalty(PeerViolation::RateLimitSpam, current_time);
        }
        allowed
    }

    /// Menandai handshake sukses dan mengubah state menjadi Connected.
    pub fn mark_handshake_complete(
        &mut self,
        node_id: Address,
        public_key: [u8; 32],
        best_height: u64,
    ) {
        self.node_id = Some(node_id);
        self.public_key = Some(public_key);
        self.best_height = best_height;
        if self.state != PeerState::Banned && self.state != PeerState::Throttled {
            self.state = PeerState::Connected;
        }
    }
}
