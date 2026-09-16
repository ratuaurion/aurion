//! Aurion Layer-4 Interoperability Conformance Test Harness.
//!
//! 12 Pilar Pengujian Kepatuhan (REQ-L4-01..12) sesuai
//! Dokumen Aturan Aplikasi 19 (19-L4-INTEROPERABILITY-ARCHITECTURE-BLUEPRINT.md).
//!
//! Setiap REQ di-test secara terpisah, menggunakan hanya API aktual yang ter-ekspor.

use aurion::interop::{
    AnomalyEvent, AnomalySeverity, BridgeCircuitBreaker, BridgeStatus, ChainId,
    CrossChainAssetVault, CrossChainMessage, CrossChainMessageParams, CrossDomainIdentityBinding,
    DecentralizedStateReadRelay, EvmStateVerifier, ExternalHeaderEntry, FinancialRateLimiter,
    HeaderSyncTracker, L4SecurityGate, MultiProverEngine, ProofPayload, ProtocolId, ProverId,
    ProverVerdict, SovereignIdentityResolver, StateReadQuery, ThresholdCustodyAdapter,
    UniversalNullifierRegistry, ZkStateProofVerifier,
    decode_envelope, encode_envelope,
};
use aurion::primitives::core::Quantum;
use blake3::Hasher;

fn q(v: u128) -> Quantum {
    Quantum::new(v)
}

/// Builds a valid CrossChainMessage for use in tests.
fn make_msg(src: ChainId, dst: ChainId, nonce: u64) -> CrossChainMessage {
    let params = CrossChainMessageParams {
        source_chain: src,
        destination_chain: dst,
        sequence_nonce: nonce,
        sender: [0x01; 32],
        target_contract: [0x02; 32],
        payload: vec![0xCA, 0xFE, 0xBA, 0xBE],
        timeout_timestamp: 1_900_000_000,
        protocol: ProtocolId::EvmSyncCommittee,
        gas_limit: 500_000,
        max_fee: q(1_000_000),
        proof: ProofPayload::None,
    };
    CrossChainMessage::new(params).expect("make_msg: must succeed")
}

/// REQ-L4-01: Canonical 8-field cross-chain envelope serialization roundtrip.
/// Acceptance: encode → decode produces identical packet_id.
#[test]
fn req_l4_01_canonical_envelope_codec_roundtrip() {
    let msg = make_msg(ChainId::Ethereum, ChainId::AurionL1, 1);
    let encoded = encode_envelope(&msg, BridgeStatus::Active).expect("REQ-L4-01: encode must succeed");
    let (decoded, _status) = decode_envelope(&encoded).expect("REQ-L4-01: decode must succeed");
    assert_eq!(msg.packet_id, decoded.packet_id, "REQ-L4-01: packet_id must survive roundtrip");
    assert_eq!(msg.source_chain, decoded.source_chain);
    assert_eq!(msg.destination_chain, decoded.destination_chain);
    assert_eq!(msg.sequence_nonce, decoded.sequence_nonce);
    assert_eq!(msg.payload, decoded.payload);
}

/// REQ-L4-02: Blake3 packet integrity self-validation.
/// Acceptance: verify_integrity() must return true for well-formed messages.
#[test]
fn req_l4_02_packet_integrity_self_validation() {
    let msg = make_msg(ChainId::Bitcoin, ChainId::AurionL1, 2);
    assert!(msg.verify_integrity(), "REQ-L4-02: packet_id must match Blake3 digest of fields");
}

/// REQ-L4-03: Payload size limit enforcement (DoS prevention — 64 KB cap).
#[test]
fn req_l4_03_payload_dos_size_limit() {
    let oversized = CrossChainMessage::new(CrossChainMessageParams {
        source_chain: ChainId::Ethereum,
        destination_chain: ChainId::AurionL1,
        sequence_nonce: 3,
        sender: [0u8; 32],
        target_contract: [0u8; 32],
        payload: vec![0u8; 65_537], // > 64 KB
        timeout_timestamp: 1_900_000_000,
        protocol: ProtocolId::EvmSyncCommittee,
        gas_limit: 100_000,
        max_fee: q(1_000),
        proof: ProofPayload::None,
    });
    assert!(oversized.is_err(), "REQ-L4-03: oversized payload must be rejected");
}

/// REQ-L4-04: Bitcoin SPV Merkle proof verifier.
#[test]
fn req_l4_04_bitcoin_spv_merkle_verifier() {
    // Single transaction, compute root and verify
    let tx_hash = [0xAB; 32];
    let merkle_root = aurion::interop::BitcoinSpvVerifier::compute_merkle_root(&[tx_hash]);

    // Single-leaf tree: path is empty, index 0
    assert!(
        aurion::interop::BitcoinSpvVerifier::verify_merkle_branch(tx_hash, &[], 0, merkle_root),
        "REQ-L4-04: single-leaf SPV proof must verify against computed root"
    );
}

/// REQ-L4-05: EVM state proof verification against state root.
#[test]
fn req_l4_05_evm_state_proof_verifier() {
    let target = [0xAA; 32];
    let slot = [0xBB; 32];
    let value = [0xCC; 32];
    let proof_node = [0xDD; 32];

    let mut h = Hasher::new();
    h.update(b"AURION-EVM-STATE-V1");
    h.update(&target);
    h.update(&slot);
    h.update(&value);
    h.update(&proof_node);
    let state_root = *h.finalize().as_bytes();

    assert!(
        EvmStateVerifier::verify_account_state(target, slot, value, &[proof_node], state_root),
        "REQ-L4-05: EVM account state proof must verify"
    );
}

/// REQ-L4-06: ZK state proof verifier roundtrip.
#[test]
fn req_l4_06_zk_state_proof_verifier() {
    let state_root = [0x42; 32];
    let public_inputs = [0x99; 32];
    let proof = ZkStateProofVerifier::generate_test_proof(state_root, public_inputs);
    assert!(
        ZkStateProofVerifier::verify_zk_state_proof(state_root, public_inputs, &proof),
        "REQ-L4-06: ZK state proof must verify successfully"
    );
}

/// REQ-L4-07: Trust-minimized relayer header sync + finality (N-confirmation delay).
#[test]
fn req_l4_07_trust_minimized_relayer_finality() {
    let mut tracker = HeaderSyncTracker::new(ChainId::Ethereum, 3);
    let mut prev = [0u8; 32];

    for height in 0u64..=9 {
        let mut h = Hasher::new();
        h.update(&prev);
        h.update(&height.to_be_bytes());
        let bh = *h.finalize().as_bytes();
        tracker
            .ingest_header(ExternalHeaderEntry {
                height,
                block_hash: bh,
                parent_hash: prev,
                root_commitment: [0x11; 32],
                timestamp: 1_700_000_000 + height * 12,
            })
            .expect("REQ-L4-07: header ingest must succeed");
        prev = bh;
    }

    assert_eq!(tracker.latest_height(), 9, "REQ-L4-07: latest height must be 9");
    assert_eq!(tracker.confirmed_height(), 6, "REQ-L4-07: confirmed height = 9 - 3 = 6");
    assert!(tracker.is_confirmed(6), "REQ-L4-07: height 6 must be confirmed");
    assert!(!tracker.is_confirmed(7), "REQ-L4-07: height 7 must NOT be confirmed");
}

/// REQ-L4-08: Cross-chain asset vault lock-and-mint lifecycle with conservation invariant.
#[test]
fn req_l4_08_vault_lock_mint_conservation() {
    let custody = ThresholdCustodyAdapter::new(vec![[0x11; 32], [0x22; 32], [0x33; 32]])
        .expect("REQ-L4-08: TSS custody creation must succeed");

    let mut vault = CrossChainAssetVault::new(ChainId::Ethereum, *b"wETH0000", custody);

    let amount = q(2_000_000);
    vault
        .process_external_lock_and_mint(amount, ChainId::Ethereum, [0x99; 32], 1_700_000_000)
        .expect("REQ-L4-08: lock-and-mint must succeed");

    assert!(vault.audit_conservation().unwrap(), "REQ-L4-08: 1:1 conservation invariant must hold");
    assert_eq!(vault.net_active_wrapped_supply(), amount);
    assert_eq!(vault.net_foreign_reserve(), amount);
}

/// REQ-L4-09: Multi-prover 2-of-3 quorum evaluation.
#[test]
fn req_l4_09_multi_prover_2_of_3_quorum() {
    let mut engine = MultiProverEngine::new();
    let claim = MultiProverEngine::compute_claim_id(&[0xAB; 32], ChainId::Ethereum);

    // 2 valid, 1 inconclusive → Accepted
    engine.submit_verdict(claim, ProverId::LightClient, ProverVerdict::Valid);
    engine.submit_verdict(claim, ProverId::ZkStateProof, ProverVerdict::Valid);
    engine.submit_verdict(claim, ProverId::OptimisticWatcher, ProverVerdict::Inconclusive);

    assert_eq!(
        engine.evaluate_quorum(claim),
        aurion::interop::MultiProverResult::Accepted,
        "REQ-L4-09: 2-of-3 Valid must produce Accepted"
    );
}

/// REQ-L4-10: Financial rate limiter — volume cap enforcement.
#[test]
fn req_l4_10_financial_rate_limiter() {
    let mut limiter = FinancialRateLimiter::new(q(5_000_000), 100);

    // Transfer within window limit
    limiter
        .record_transfer(ChainId::Ethereum, q(3_000_000), 1)
        .expect("REQ-L4-10: transfer within cap must succeed");

    // Exceeds cap within same window
    let over = limiter.record_transfer(ChainId::Ethereum, q(3_000_000), 50);
    assert!(over.is_err(), "REQ-L4-10: transfer exceeding cap must fail");
}

/// REQ-L4-11: Circuit breaker trips on Critical anomaly; isolation invariant: L1 not affected.
#[test]
fn req_l4_11_circuit_breaker_isolation() {
    let mut cb = BridgeCircuitBreaker::new();

    let tripped = cb.report_anomaly(AnomalyEvent {
        bridge_chain: ChainId::Ethereum,
        event_hash: AnomalyEvent::compute_hash(ChainId::Ethereum, 100, 50),
        severity: AnomalySeverity::Critical,
        detected_at_slot: 50,
    });

    assert!(tripped, "REQ-L4-11: Critical anomaly must trip circuit");
    assert!(cb.is_halted(ChainId::Ethereum), "REQ-L4-11: Ethereum bridge must be halted");
    assert!(!cb.is_halted(ChainId::AurionL1), "REQ-L4-11: AurionL1 must NOT be halted");
    assert!(!cb.is_halted(ChainId::Bitcoin), "REQ-L4-11: Bitcoin bridge must NOT be halted");

    // Governance reset re-opens the bridge
    let token = cb.compute_reset_token(ChainId::Ethereum);
    cb.governance_reset(ChainId::Ethereum, &token)
        .expect("REQ-L4-11: governance reset with valid token must succeed");
    assert!(!cb.is_halted(ChainId::Ethereum), "REQ-L4-11: Ethereum bridge must resume after reset");
}

/// REQ-L4-12: Universal nullifier anti-replay registry.
#[test]
fn req_l4_12_universal_nullifier_anti_replay() {
    let mut registry = UniversalNullifierRegistry::new();
    let nullifier = [0x55; 32];
    let packet_id = [0x66; 32];

    // First registration succeeds
    registry
        .register_nullifier(nullifier, ChainId::Ethereum, packet_id, 1_700_000_000)
        .expect("REQ-L4-12: first nullifier registration must succeed");

    assert!(registry.is_nullified(&nullifier));
    assert_eq!(registry.count(), 1);

    // Replay must be rejected
    let replay = registry.register_nullifier(nullifier, ChainId::Ethereum, packet_id, 1_700_000_001);
    assert!(replay.is_err(), "REQ-L4-12: replay must be rejected");
}

// ─── Additional Pillar Tests (Supplementary Coverage) ─────────────────────────

/// Verifies that state read relay enforces oracle-free cryptographic verification.
#[test]
fn supplementary_oracle_free_state_read_relay() {
    let target = [0x11; 32];
    let slot = [0x22; 32];
    let value = [0x33; 32];
    let node = [0x44; 32];

    let mut h = Hasher::new();
    h.update(b"AURION-EVM-STATE-V1");
    h.update(&target);
    h.update(&slot);
    h.update(&value);
    h.update(&node);
    let state_root = *h.finalize().as_bytes();

    let mut tracker = HeaderSyncTracker::new(ChainId::Ethereum, 3);
    let mut prev = [0u8; 32];
    for height in 0u64..=9 {
        let mut bh_hasher = Hasher::new();
        bh_hasher.update(&prev);
        bh_hasher.update(&height.to_be_bytes());
        let bh = *bh_hasher.finalize().as_bytes();
        tracker
            .ingest_header(ExternalHeaderEntry {
                height,
                block_hash: bh,
                parent_hash: prev,
                root_commitment: state_root,
                timestamp: 1_700_000_000 + height * 12,
            })
            .unwrap();
        prev = bh;
    }

    let query = StateReadQuery::new(ChainId::Ethereum, target, slot, 4);
    let resp = DecentralizedStateReadRelay::verify_state_read(&query, value, &[node], &tracker)
        .expect("supplementary: oracle-free state read must succeed");
    assert!(resp.verified);
}

/// Verifies that cross-domain identity binding resolves correctly.
#[test]
fn supplementary_cross_domain_identity_binding() {
    let mut resolver = SovereignIdentityResolver::new();
    let aurion_key = [0xAA; 32];
    let foreign = [0xBB; 32];

    let digest = CrossDomainIdentityBinding::commitment_digest(&aurion_key, ChainId::Ethereum, &foreign);
    let sig = CrossDomainIdentityBinding::generate_test_signature(&foreign, &digest);

    resolver
        .register_binding(CrossDomainIdentityBinding {
            aurion_pubkey: aurion_key,
            foreign_chain: ChainId::Ethereum,
            foreign_address: foreign,
            attestation_sig: sig,
        })
        .expect("supplementary: identity binding must succeed");

    let resolved = resolver.resolve_foreign_address(ChainId::Ethereum, &foreign);
    assert_eq!(resolved, Some(&aurion_key));
}

/// Verifies multi-prover engine blocks on 2-of-3 Fraudulent verdicts.
#[test]
fn supplementary_multi_prover_fraud_blocks() {
    let mut gate = L4SecurityGate::new(q(10_000_000), 1000);
    let claim = MultiProverEngine::compute_claim_id(&[0xFF; 32], ChainId::Bitcoin);

    gate.multi_prover.submit_verdict(claim, ProverId::LightClient, ProverVerdict::Fraudulent);
    gate.multi_prover.submit_verdict(claim, ProverId::ZkStateProof, ProverVerdict::Fraudulent);
    gate.multi_prover.submit_verdict(claim, ProverId::OptimisticWatcher, ProverVerdict::Valid);

    let result = gate.approve_transfer(claim, ChainId::Bitcoin, q(1_000_000), 1);
    assert!(result.is_err(), "supplementary: fraudulent 2-of-3 must block transfer");
}
