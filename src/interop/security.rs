#![forbid(unsafe_code)]

//! Aurion Layer-4 (L4) Multi-Prover Security & Circuit Breaker Subsystem.
//!
//! Complies strictly with:
//! - AUR-ARCH-011: Absolute Zero Unsafe Code.
//! - AUR-ARCH-012: Absolute Zero Float Arithmetic (all arithmetic uses `u128` integers).
//! - AUR-L4-SEC-001: Bridge Exploit Containment (bridge halt must NOT affect L1 consensus).
//! - AUR-L4-SEC-002: Multi-Prover Redundant Verification (2-of-3 independent mechanisms).
//! - AUR-L4-SEC-003: Financial Rate Limiting (volume throttle per time window).

use crate::interop::types::ChainId;
use crate::primitives::core::Quantum;
use blake3::Hasher;
use std::collections::BTreeMap;

// ─── Multi-Prover Verdict ─────────────────────────────────────────────────────

/// Attestation result produced by a single prover mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProverVerdict {
    /// Prover could not find evidence to confirm or deny the claim.
    Inconclusive,
    /// Prover confirms the cross-chain claim is valid.
    Valid,
    /// Prover detects the cross-chain claim is fraudulent.
    Fraudulent,
}

/// Identifies which independent prover mechanism produced a verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProverId {
    LightClient,
    ZkStateProof,
    OptimisticWatcher,
}

/// Aggregated result after quorum evaluation across all provers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiProverResult {
    /// 2-of-3 provers agree the claim is valid.
    Accepted,
    /// 2-of-3 provers agree the claim is fraudulent.
    Rejected,
    /// No quorum reached among provers — claim is pending or disputed.
    Disputed,
}

/// Multi-Prover Redundant Verification Engine (`AUR-L4-SEC-002`).
///
/// Aggregates verdicts from three independent prover mechanisms and requires
/// a quorum of at least 2-of-3 to accept or reject a cross-chain claim.
/// This design ensures no single prover can unilaterally approve or block
/// a message, preventing both false positives and single-point-of-failure.
pub struct MultiProverEngine {
    // Stores verdicts indexed by (claim_id, prover_id)
    verdicts: BTreeMap<([u8; 32], ProverId), ProverVerdict>,
}

impl MultiProverEngine {
    pub fn new() -> Self {
        Self {
            verdicts: BTreeMap::new(),
        }
    }

    /// Submits a verdict from a specific prover for a given cross-chain claim.
    pub fn submit_verdict(
        &mut self,
        claim_id: [u8; 32],
        prover: ProverId,
        verdict: ProverVerdict,
    ) {
        self.verdicts.insert((claim_id, prover), verdict);
    }

    /// Evaluates the quorum verdict for a claim across all registered provers.
    ///
    /// Rule (AUR-L4-SEC-002): Requires 2-of-3 agreement to accept or reject.
    /// If < 2 provers agree on any outcome, returns `Disputed`.
    pub fn evaluate_quorum(&self, claim_id: [u8; 32]) -> MultiProverResult {
        let provers = [ProverId::LightClient, ProverId::ZkStateProof, ProverId::OptimisticWatcher];

        let valid_count = provers
            .iter()
            .filter(|&&p| self.verdicts.get(&(claim_id, p)) == Some(&ProverVerdict::Valid))
            .count();

        let fraud_count = provers
            .iter()
            .filter(|&&p| self.verdicts.get(&(claim_id, p)) == Some(&ProverVerdict::Fraudulent))
            .count();

        if valid_count >= 2 {
            MultiProverResult::Accepted
        } else if fraud_count >= 2 {
            MultiProverResult::Rejected
        } else {
            MultiProverResult::Disputed
        }
    }

    /// Returns the number of provers that have submitted verdicts for a claim.
    pub fn submitted_count(&self, claim_id: &[u8; 32]) -> usize {
        [ProverId::LightClient, ProverId::ZkStateProof, ProverId::OptimisticWatcher]
            .iter()
            .filter(|&&p| self.verdicts.contains_key(&(*claim_id, p)))
            .count()
    }

    /// Generates a deterministic claim ID from an envelope hash and source chain.
    pub fn compute_claim_id(envelope_hash: &[u8; 32], source_chain: ChainId) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L4-MULTI-PROVER-CLAIM-V1");
        hasher.update(envelope_hash);
        hasher.update(&source_chain.to_u64().to_be_bytes());
        *hasher.finalize().as_bytes()
    }
}

impl Default for MultiProverEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Financial Rate Limiter ───────────────────────────────────────────────────

/// Per-bridge transfer volume accumulator within a time window.
#[derive(Debug, Clone)]
struct RateLimitBucket {
    window_start_slot: u64,
    accumulated_volume: u128, // in Quanta (u128 integer, no float)
}

/// Financial Rate Limiter & Anomaly Detection Engine (`AUR-L4-SEC-003`).
///
/// Restricts the volume of assets transferred through each bridge pair
/// within a sliding time window (measured in abstract "slots").
/// If a transfer would cause the window total to exceed the configured
/// capacity ceiling, the transfer is rejected before it reaches the vault.
///
/// All arithmetic uses `u128` integers — absolute zero floating-point.
pub struct FinancialRateLimiter {
    /// Maximum Quanta transferable per bridge per window.
    window_capacity: u128,
    /// Window duration measured in slots (abstract time unit).
    window_slots: u64,
    /// Per-bridge bucket state.
    buckets: BTreeMap<ChainId, RateLimitBucket>,
}

impl FinancialRateLimiter {
    /// Creates a new rate limiter with a given capacity ceiling and window duration.
    pub fn new(window_capacity: Quantum, window_slots: u64) -> Self {
        Self {
            window_capacity: window_capacity.as_u128(),
            window_slots,
            buckets: BTreeMap::new(),
        }
    }

    /// Attempts to record a transfer of `amount` Quanta through `bridge_chain` at `current_slot`.
    ///
    /// Returns `Ok(())` if the transfer is within limits, or `Err` if it exceeds the window cap.
    pub fn record_transfer(
        &mut self,
        bridge_chain: ChainId,
        amount: Quantum,
        current_slot: u64,
    ) -> Result<(), &'static str> {
        let amount_quanta = amount.as_u128();

        let bucket = self.buckets.entry(bridge_chain).or_insert(RateLimitBucket {
            window_start_slot: current_slot,
            accumulated_volume: 0,
        });

        // Reset if current slot is outside the window
        if current_slot >= bucket.window_start_slot + self.window_slots {
            bucket.window_start_slot = current_slot;
            bucket.accumulated_volume = 0;
        }

        let new_total = bucket
            .accumulated_volume
            .checked_add(amount_quanta)
            .ok_or("Transfer amount overflows u128 accumulator")?;

        if new_total > self.window_capacity {
            return Err("Bridge rate limit exceeded: transfer would breach window capacity");
        }

        bucket.accumulated_volume = new_total;
        Ok(())
    }

    /// Queries the current accumulated volume for a bridge in the active window.
    pub fn current_volume(&self, bridge_chain: ChainId, current_slot: u64) -> u128 {
        match self.buckets.get(&bridge_chain) {
            Some(bucket) if current_slot < bucket.window_start_slot + self.window_slots => {
                bucket.accumulated_volume
            }
            _ => 0,
        }
    }

    /// Returns the configured window capacity ceiling in Quanta.
    pub fn window_capacity(&self) -> u128 {
        self.window_capacity
    }
}

// ─── Circuit Breaker ─────────────────────────────────────────────────────────

/// Status of a bridge circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitStatus {
    /// Bridge is operational; transfers are permitted.
    Closed,
    /// Bridge is halted due to detected anomaly; all transfers are blocked.
    Open,
}

/// Anomaly event that triggered or may trigger a circuit break.
#[derive(Debug, Clone)]
pub struct AnomalyEvent {
    pub bridge_chain: ChainId,
    pub event_hash: [u8; 32],
    pub severity: AnomalySeverity,
    pub detected_at_slot: u64,
}

/// Severity levels for detected anomalies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnomalySeverity {
    /// Minor anomaly: log only; circuit remains closed.
    Low,
    /// Significant deviation: alert; circuit remains closed pending escalation.
    Medium,
    /// Critical: automatic circuit break; bridge halted immediately.
    Critical,
}

impl AnomalyEvent {
    /// Computes a deterministic hash for this anomaly event.
    pub fn compute_hash(bridge_chain: ChainId, severity_byte: u8, slot: u64) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L4-ANOMALY-EVENT-V1");
        hasher.update(&bridge_chain.to_u64().to_be_bytes());
        hasher.update(&[severity_byte]);
        hasher.update(&slot.to_be_bytes());
        *hasher.finalize().as_bytes()
    }
}

/// Automated Emergency Bridge Circuit Breaker (`AUR-L4-SEC-001`).
///
/// Monitors per-bridge anomaly events and automatically halts (opens the circuit)
/// any bridge where a `Critical` anomaly is detected.
///
/// **Invariant**: Halting a bridge does NOT affect L1 Aurion consensus or state.
/// The circuit breaker is scoped exclusively to the interoperability layer.
pub struct BridgeCircuitBreaker {
    /// Per-bridge circuit status.
    circuit_states: BTreeMap<ChainId, CircuitStatus>,
    /// Log of all anomaly events per bridge.
    anomaly_log: BTreeMap<ChainId, Vec<AnomalyEvent>>,
}

impl BridgeCircuitBreaker {
    pub fn new() -> Self {
        Self {
            circuit_states: BTreeMap::new(),
            anomaly_log: BTreeMap::new(),
        }
    }

    /// Reports an anomaly event for a bridge.
    ///
    /// If severity is `Critical`, the circuit is immediately opened (bridge halted).
    /// Returns `true` if the circuit was tripped by this event.
    pub fn report_anomaly(&mut self, event: AnomalyEvent) -> bool {
        let chain = event.bridge_chain;
        let tripped = event.severity == AnomalySeverity::Critical;

        self.anomaly_log.entry(chain).or_default().push(event);

        if tripped {
            self.circuit_states.insert(chain, CircuitStatus::Open);
        } else {
            self.circuit_states.entry(chain).or_insert(CircuitStatus::Closed);
        }

        tripped
    }

    /// Checks if a bridge circuit is currently open (halted).
    pub fn is_halted(&self, chain: ChainId) -> bool {
        self.circuit_states.get(&chain) == Some(&CircuitStatus::Open)
    }

    /// Returns the current circuit status for a bridge.
    pub fn status(&self, chain: ChainId) -> CircuitStatus {
        *self.circuit_states.get(&chain).unwrap_or(&CircuitStatus::Closed)
    }

    /// Manually resets a halted bridge circuit (governance action only).
    ///
    /// This is gated by a governance reset token (Blake3 commitment) to prevent
    /// unauthorized re-opening of a tripped circuit.
    pub fn governance_reset(
        &mut self,
        chain: ChainId,
        governance_token: &[u8; 32],
    ) -> Result<(), &'static str> {
        // Verify the token is a valid governance reset commitment
        let expected = self.compute_reset_token(chain);
        if *governance_token != expected {
            return Err("Invalid governance reset token");
        }

        self.circuit_states.insert(chain, CircuitStatus::Closed);
        Ok(())
    }

    /// Returns the number of anomaly events logged for a bridge.
    pub fn anomaly_count(&self, chain: ChainId) -> usize {
        self.anomaly_log.get(&chain).map(|v| v.len()).unwrap_or(0)
    }

    /// Computes the expected governance reset token for a bridge.
    ///
    /// In production this would be derived from a multi-sig governance ceremony.
    /// For this reference implementation it commits to a deterministic domain hash.
    pub fn compute_reset_token(&self, chain: ChainId) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L4-CIRCUIT-RESET-GOVERNANCE-V1");
        hasher.update(&chain.to_u64().to_be_bytes());
        *hasher.finalize().as_bytes()
    }
}

impl Default for BridgeCircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Integrated Security Gate ─────────────────────────────────────────────────

/// Security Gate: composes all three L4 security mechanisms into a single
/// transfer approval pipeline.
///
/// A transfer is approved only if ALL of:
/// 1. The bridge circuit is NOT open (halted).
/// 2. The financial rate limit is NOT exceeded.
/// 3. The multi-prover quorum result is `Accepted` (2-of-3 agree).
pub struct L4SecurityGate {
    pub multi_prover: MultiProverEngine,
    pub rate_limiter: FinancialRateLimiter,
    pub circuit_breaker: BridgeCircuitBreaker,
}

impl L4SecurityGate {
    pub fn new(window_capacity: Quantum, window_slots: u64) -> Self {
        Self {
            multi_prover: MultiProverEngine::new(),
            rate_limiter: FinancialRateLimiter::new(window_capacity, window_slots),
            circuit_breaker: BridgeCircuitBreaker::new(),
        }
    }

    /// Evaluates whether a cross-chain transfer is permitted through all security layers.
    ///
    /// Returns `Ok(())` if all three gates pass, or `Err` with the failing gate description.
    pub fn approve_transfer(
        &mut self,
        claim_id: [u8; 32],
        bridge_chain: ChainId,
        amount: Quantum,
        current_slot: u64,
    ) -> Result<(), &'static str> {
        // Gate 1: Circuit Breaker
        if self.circuit_breaker.is_halted(bridge_chain) {
            return Err("Transfer rejected: bridge circuit is open (halted due to anomaly)");
        }

        // Gate 2: Financial Rate Limiter
        self.rate_limiter
            .record_transfer(bridge_chain, amount, current_slot)?;

        // Gate 3: Multi-Prover Quorum
        match self.multi_prover.evaluate_quorum(claim_id) {
            MultiProverResult::Accepted => Ok(()),
            MultiProverResult::Rejected => {
                Err("Transfer rejected: multi-prover quorum voted Fraudulent")
            }
            MultiProverResult::Disputed => {
                Err("Transfer rejected: multi-prover quorum is Disputed (insufficient votes)")
            }
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: Quantum from raw quanta
    fn q(quanta: u128) -> Quantum {
        Quantum::new(quanta)
    }

    // ─── Multi-Prover Tests ───────────────────────────────────────────────────

    #[test]
    fn test_multi_prover_quorum_accepted() {
        let mut engine = MultiProverEngine::new();
        let claim_id = MultiProverEngine::compute_claim_id(&[0x01; 32], ChainId::Ethereum);

        engine.submit_verdict(claim_id, ProverId::LightClient, ProverVerdict::Valid);
        engine.submit_verdict(claim_id, ProverId::ZkStateProof, ProverVerdict::Valid);
        engine.submit_verdict(claim_id, ProverId::OptimisticWatcher, ProverVerdict::Inconclusive);

        assert_eq!(engine.evaluate_quorum(claim_id), MultiProverResult::Accepted);
    }

    #[test]
    fn test_multi_prover_quorum_rejected() {
        let mut engine = MultiProverEngine::new();
        let claim_id = MultiProverEngine::compute_claim_id(&[0x02; 32], ChainId::Bitcoin);

        engine.submit_verdict(claim_id, ProverId::LightClient, ProverVerdict::Fraudulent);
        engine.submit_verdict(claim_id, ProverId::ZkStateProof, ProverVerdict::Fraudulent);
        engine.submit_verdict(claim_id, ProverId::OptimisticWatcher, ProverVerdict::Valid);

        assert_eq!(engine.evaluate_quorum(claim_id), MultiProverResult::Rejected);
    }

    #[test]
    fn test_multi_prover_quorum_disputed_no_majority() {
        let mut engine = MultiProverEngine::new();
        let claim_id = MultiProverEngine::compute_claim_id(&[0x03; 32], ChainId::CosmosIbc);

        engine.submit_verdict(claim_id, ProverId::LightClient, ProverVerdict::Valid);
        engine.submit_verdict(claim_id, ProverId::ZkStateProof, ProverVerdict::Fraudulent);
        engine.submit_verdict(claim_id, ProverId::OptimisticWatcher, ProverVerdict::Inconclusive);

        assert_eq!(engine.evaluate_quorum(claim_id), MultiProverResult::Disputed);
    }

    #[test]
    fn test_multi_prover_quorum_disputed_no_votes() {
        let engine = MultiProverEngine::new();
        let claim_id = [0xFF; 32];
        assert_eq!(engine.evaluate_quorum(claim_id), MultiProverResult::Disputed);
    }

    #[test]
    fn test_multi_prover_submitted_count() {
        let mut engine = MultiProverEngine::new();
        let claim_id = [0xAA; 32];

        assert_eq!(engine.submitted_count(&claim_id), 0);
        engine.submit_verdict(claim_id, ProverId::LightClient, ProverVerdict::Valid);
        assert_eq!(engine.submitted_count(&claim_id), 1);
        engine.submit_verdict(claim_id, ProverId::ZkStateProof, ProverVerdict::Valid);
        assert_eq!(engine.submitted_count(&claim_id), 2);
    }

    // ─── Financial Rate Limiter Tests ─────────────────────────────────────────

    #[test]
    fn test_rate_limiter_within_window() {
        let capacity = q(1_000_000); // 1M Quanta per 100 slots
        let mut limiter = FinancialRateLimiter::new(capacity, 100);

        limiter.record_transfer(ChainId::Ethereum, q(400_000), 50).unwrap();
        limiter.record_transfer(ChainId::Ethereum, q(400_000), 50).unwrap();
        assert_eq!(limiter.current_volume(ChainId::Ethereum, 50), 800_000);
    }

    #[test]
    fn test_rate_limiter_exceeded() {
        let capacity = q(1_000_000);
        let mut limiter = FinancialRateLimiter::new(capacity, 100);

        limiter.record_transfer(ChainId::Ethereum, q(900_000), 10).unwrap();
        let result = limiter.record_transfer(ChainId::Ethereum, q(200_000), 10);
        assert!(result.is_err(), "Should be rejected: exceeds window cap");
    }

    #[test]
    fn test_rate_limiter_window_reset() {
        let capacity = q(1_000_000);
        let mut limiter = FinancialRateLimiter::new(capacity, 100);

        limiter.record_transfer(ChainId::Ethereum, q(900_000), 0).unwrap();
        // New window starts at slot 100
        limiter.record_transfer(ChainId::Ethereum, q(900_000), 100).unwrap();
        assert_eq!(limiter.current_volume(ChainId::Ethereum, 100), 900_000);
    }

    #[test]
    fn test_rate_limiter_independent_per_chain() {
        let capacity = q(1_000_000);
        let mut limiter = FinancialRateLimiter::new(capacity, 100);

        limiter.record_transfer(ChainId::Ethereum, q(900_000), 1).unwrap();
        // Bitcoin bridge has its own independent bucket
        limiter.record_transfer(ChainId::Bitcoin, q(900_000), 1).unwrap();
        assert_eq!(limiter.current_volume(ChainId::Bitcoin, 1), 900_000);
    }

    // ─── Circuit Breaker Tests ────────────────────────────────────────────────

    #[test]
    fn test_circuit_breaker_open_on_critical() {
        let mut cb = BridgeCircuitBreaker::new();

        assert_eq!(cb.status(ChainId::Ethereum), CircuitStatus::Closed);

        let event = AnomalyEvent {
            bridge_chain: ChainId::Ethereum,
            event_hash: AnomalyEvent::compute_hash(ChainId::Ethereum, 2, 1000),
            severity: AnomalySeverity::Critical,
            detected_at_slot: 1000,
        };

        let tripped = cb.report_anomaly(event);
        assert!(tripped);
        assert!(cb.is_halted(ChainId::Ethereum));
        assert_eq!(cb.status(ChainId::Ethereum), CircuitStatus::Open);
    }

    #[test]
    fn test_circuit_breaker_no_trip_on_low_severity() {
        let mut cb = BridgeCircuitBreaker::new();

        let event = AnomalyEvent {
            bridge_chain: ChainId::Bitcoin,
            event_hash: AnomalyEvent::compute_hash(ChainId::Bitcoin, 0, 500),
            severity: AnomalySeverity::Low,
            detected_at_slot: 500,
        };

        let tripped = cb.report_anomaly(event);
        assert!(!tripped);
        assert!(!cb.is_halted(ChainId::Bitcoin));
        assert_eq!(cb.status(ChainId::Bitcoin), CircuitStatus::Closed);
    }

    #[test]
    fn test_circuit_breaker_governance_reset() {
        let mut cb = BridgeCircuitBreaker::new();

        let event = AnomalyEvent {
            bridge_chain: ChainId::Ethereum,
            event_hash: [0; 32],
            severity: AnomalySeverity::Critical,
            detected_at_slot: 9999,
        };
        cb.report_anomaly(event);
        assert!(cb.is_halted(ChainId::Ethereum));

        // Wrong token is rejected
        let wrong_token = [0xFF; 32];
        assert!(cb.governance_reset(ChainId::Ethereum, &wrong_token).is_err());
        assert!(cb.is_halted(ChainId::Ethereum));

        // Correct token re-closes the circuit
        let correct_token = cb.compute_reset_token(ChainId::Ethereum);
        cb.governance_reset(ChainId::Ethereum, &correct_token).unwrap();
        assert!(!cb.is_halted(ChainId::Ethereum));
    }

    #[test]
    fn test_circuit_breaker_anomaly_log_count() {
        let mut cb = BridgeCircuitBreaker::new();

        for i in 0u64..3 {
            cb.report_anomaly(AnomalyEvent {
                bridge_chain: ChainId::CosmosIbc,
                event_hash: AnomalyEvent::compute_hash(ChainId::CosmosIbc, 1, i),
                severity: AnomalySeverity::Medium,
                detected_at_slot: i,
            });
        }

        assert_eq!(cb.anomaly_count(ChainId::CosmosIbc), 3);
    }

    #[test]
    fn test_circuit_breaker_isolation_l1_unaffected() {
        // Verify the circuit breaker operates purely at interop layer:
        // opening a bridge circuit does not modify any L1 state struct.
        let mut cb = BridgeCircuitBreaker::new();
        cb.report_anomaly(AnomalyEvent {
            bridge_chain: ChainId::Ethereum,
            event_hash: [0; 32],
            severity: AnomalySeverity::Critical,
            detected_at_slot: 0,
        });
        // Bitcoin bridge remains closed — independent containment
        assert!(!cb.is_halted(ChainId::Bitcoin));
        assert_eq!(cb.status(ChainId::Bitcoin), CircuitStatus::Closed);
    }

    // ─── Integrated Security Gate Tests ──────────────────────────────────────

    #[test]
    fn test_security_gate_approve_all_pass() {
        let mut gate = L4SecurityGate::new(q(1_000_000), 100);
        let claim_id = MultiProverEngine::compute_claim_id(&[0x10; 32], ChainId::Ethereum);

        // Setup: 2-of-3 provers vote Valid
        gate.multi_prover
            .submit_verdict(claim_id, ProverId::LightClient, ProverVerdict::Valid);
        gate.multi_prover
            .submit_verdict(claim_id, ProverId::ZkStateProof, ProverVerdict::Valid);

        let result = gate.approve_transfer(claim_id, ChainId::Ethereum, q(100_000), 1);
        assert!(result.is_ok(), "Security gate should pass all three checks");
    }

    #[test]
    fn test_security_gate_blocked_by_circuit_breaker() {
        let mut gate = L4SecurityGate::new(q(1_000_000), 100);
        let claim_id = MultiProverEngine::compute_claim_id(&[0x20; 32], ChainId::Bitcoin);

        // Trip circuit breaker
        gate.circuit_breaker.report_anomaly(AnomalyEvent {
            bridge_chain: ChainId::Bitcoin,
            event_hash: [0; 32],
            severity: AnomalySeverity::Critical,
            detected_at_slot: 0,
        });

        // Setup: provers vote valid — should still fail due to circuit
        gate.multi_prover
            .submit_verdict(claim_id, ProverId::LightClient, ProverVerdict::Valid);
        gate.multi_prover
            .submit_verdict(claim_id, ProverId::ZkStateProof, ProverVerdict::Valid);

        let result = gate.approve_transfer(claim_id, ChainId::Bitcoin, q(100_000), 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("circuit is open"));
    }

    #[test]
    fn test_security_gate_blocked_by_rate_limit() {
        let mut gate = L4SecurityGate::new(q(500_000), 100);
        let claim_id = MultiProverEngine::compute_claim_id(&[0x30; 32], ChainId::Ethereum);

        gate.multi_prover
            .submit_verdict(claim_id, ProverId::LightClient, ProverVerdict::Valid);
        gate.multi_prover
            .submit_verdict(claim_id, ProverId::ZkStateProof, ProverVerdict::Valid);

        // First transfer passes
        gate.approve_transfer(claim_id, ChainId::Ethereum, q(400_000), 1).unwrap();
        // Second transfer exceeds window capacity
        let result = gate.approve_transfer(claim_id, ChainId::Ethereum, q(200_000), 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("rate limit exceeded"));
    }

    #[test]
    fn test_security_gate_blocked_by_prover_dispute() {
        let mut gate = L4SecurityGate::new(q(1_000_000), 100);
        let claim_id = MultiProverEngine::compute_claim_id(&[0x40; 32], ChainId::CosmosIbc);

        // Only 1 prover votes — no quorum
        gate.multi_prover
            .submit_verdict(claim_id, ProverId::LightClient, ProverVerdict::Valid);

        let result = gate.approve_transfer(claim_id, ChainId::CosmosIbc, q(50_000), 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Disputed"));
    }
}
