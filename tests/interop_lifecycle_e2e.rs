//! Aurion Layer-4 End-to-End Cross-Chain Lifecycle Simulation.
//!
//! Mensimulasikan siklus penuh cross-chain bridge dengan injeksi anomali adversarial:
//!
//!   Buat Pesan → Encode/Decode Wire → Sync Header → Verifikasi State EVM + ZK →
//!   Relay Trust-Minimized → Multi-Prover Quorum → Rate Limiter →
//!   Vault Lock-and-Mint → Oracle-Free State Read → Identity Binding →
//!   Anti-Replay Nullifier → Anomali Critical → Circuit Break → Governance Reset →
//!   Vault Burn-and-Unlock (Siklus Selesai)

use aurion::interop::{
    decode_envelope, encode_envelope, AnomalyEvent, AnomalySeverity, BridgeCircuitBreaker,
    BridgeStatus, ChainId, CrossChainAssetVault, CrossChainMessage, CrossChainMessageParams,
    CrossDomainIdentityBinding, DecentralizedStateReadRelay, EvmStateVerifier, ExternalHeaderEntry,
    FinancialRateLimiter, HeaderSyncTracker, L4SecurityGate, MultiProverEngine, ProofPayload,
    ProtocolId, ProverId, ProverVerdict, SovereignIdentityResolver, StateReadQuery,
    ThresholdCustodyAdapter, TrustMinimizedRelayer, UniversalNullifierRegistry,
    ZkStateProofVerifier,
};
use aurion::primitives::core::Quantum;
use blake3::Hasher;

fn q(v: u128) -> Quantum {
    Quantum::new(v)
}

/// Builds a state root compatible with EvmStateVerifier::verify_account_state.
fn build_evm_state_root(
    target: [u8; 32],
    slot: [u8; 32],
    value: [u8; 32],
    node: [u8; 32],
) -> [u8; 32] {
    let mut h = Hasher::new();
    h.update(b"AURION-EVM-STATE-V1");
    h.update(&target);
    h.update(&slot);
    h.update(&value);
    h.update(&node);
    *h.finalize().as_bytes()
}

/// Builds a HeaderSyncTracker with `n_headers` headers ingested, using given state_root.
fn build_tracker(
    chain: ChainId,
    confirmations: u64,
    n_headers: u64,
    state_root: [u8; 32],
) -> HeaderSyncTracker {
    let mut tracker = HeaderSyncTracker::new(chain, confirmations);
    let mut prev = [0u8; 32];
    for height in 0..=n_headers {
        let mut h = Hasher::new();
        h.update(&prev);
        h.update(&height.to_be_bytes());
        let bh = *h.finalize().as_bytes();
        tracker
            .ingest_header(ExternalHeaderEntry {
                height,
                block_hash: bh,
                parent_hash: prev,
                root_commitment: state_root,
                timestamp: 1_700_000_000 + height * 12,
            })
            .expect("build_tracker: header ingest must succeed");
        prev = bh;
    }
    tracker
}

/// Full L4 cross-chain lifecycle — 12 fases berurutan.
#[test]
fn l4_lifecycle_e2e_cross_chain_simulation() {
    // ─── Fase 1: Buat & Encode Pesan ──────────────────────────────────────────
    let params = CrossChainMessageParams {
        source_chain: ChainId::Ethereum,
        destination_chain: ChainId::AurionL1,
        sequence_nonce: 42,
        sender: [0x01; 32],
        target_contract: [0x02; 32],
        payload: vec![0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x01, 0x02, 0x03],
        timeout_timestamp: 1_900_000_000,
        protocol: ProtocolId::EvmSyncCommittee,
        gas_limit: 500_000,
        max_fee: q(1_000_000),
        proof: ProofPayload::ThresholdSignature(vec![0xAA; 64]),
    };

    let msg = CrossChainMessage::new(params).expect("Fase 1: message creation must succeed");
    let encoded = encode_envelope(&msg, BridgeStatus::Active).expect("Fase 1: encode must succeed");
    let (decoded, _decoded_status) =
        decode_envelope(&encoded).expect("Fase 1: decode must succeed");
    assert_eq!(
        msg.packet_id, decoded.packet_id,
        "Fase 1: packet_id must survive roundtrip"
    );
    assert!(msg.verify_integrity(), "Fase 1: Blake3 integrity must pass");

    // ─── Fase 2: Header Sync & Finality ───────────────────────────────────────
    let target_address = [0xDE; 32];
    let storage_slot = [0xAD; 32];
    let storage_value = [0xBE; 32];
    let proof_node = [0xEF; 32];
    let state_root = build_evm_state_root(target_address, storage_slot, storage_value, proof_node);

    let tracker = build_tracker(ChainId::Ethereum, 3, 8, state_root);
    assert_eq!(
        tracker.latest_height(),
        8,
        "Fase 2: latest height must be 8"
    );
    assert_eq!(
        tracker.confirmed_height(),
        5,
        "Fase 2: confirmed = 8 - 3 = 5"
    );

    // ─── Fase 3: EVM State Proof ───────────────────────────────────────────────
    assert!(
        EvmStateVerifier::verify_account_state(
            target_address,
            storage_slot,
            storage_value,
            &[proof_node],
            state_root
        ),
        "Fase 3: EVM state proof must verify"
    );

    // ─── Fase 4: ZK State Proof ────────────────────────────────────────────────
    let public_inputs = [0xAB; 32];
    let zk_proof = ZkStateProofVerifier::generate_test_proof(state_root, public_inputs);
    assert!(
        ZkStateProofVerifier::verify_zk_state_proof(state_root, public_inputs, &zk_proof),
        "Fase 4: ZK proof must verify"
    );

    // ─── Fase 5: Trust-Minimized Relayer ──────────────────────────────────────
    let mut relayer = TrustMinimizedRelayer::new();
    relayer.register_chain_tracker(tracker);

    let relay_result = relayer.verify_inbound_message(&msg, 4, 1_700_000_100);
    // Should be Verified or PendingFinality depending on tracker state
    // We accept both as the tracker is freshly registered
    let is_valid_relay = matches!(
        relay_result,
        aurion::interop::RelayVerificationResult::Verified { .. }
            | aurion::interop::RelayVerificationResult::PendingFinality { .. }
    );
    assert!(
        is_valid_relay,
        "Fase 5: relay result must be Verified or PendingFinality"
    );

    // ─── Fase 6: Multi-Prover 2-of-3 Quorum ──────────────────────────────────
    let mut prover_engine = MultiProverEngine::new();
    let claim_id = MultiProverEngine::compute_claim_id(&msg.packet_id, ChainId::Ethereum);

    prover_engine.submit_verdict(claim_id, ProverId::LightClient, ProverVerdict::Valid);
    prover_engine.submit_verdict(claim_id, ProverId::ZkStateProof, ProverVerdict::Valid);
    prover_engine.submit_verdict(
        claim_id,
        ProverId::OptimisticWatcher,
        ProverVerdict::Inconclusive,
    );

    assert_eq!(
        prover_engine.evaluate_quorum(claim_id),
        aurion::interop::MultiProverResult::Accepted,
        "Fase 6: 2-of-3 Valid must be Accepted"
    );

    // ─── Fase 7: Financial Rate Limiter ───────────────────────────────────────
    let mut rate_limiter = FinancialRateLimiter::new(q(5_000_000), 100);
    rate_limiter
        .record_transfer(ChainId::Ethereum, q(1_000_000), 10)
        .expect("Fase 7: transfer within limits must succeed");

    let volume = rate_limiter.current_volume(ChainId::Ethereum, 10);
    assert_eq!(
        volume, 1_000_000,
        "Fase 7: volume must equal transferred amount"
    );

    // ─── Fase 8: Vault Lock-and-Mint ─────────────────────────────────────────
    let custody = ThresholdCustodyAdapter::new(vec![[0x11; 32], [0x22; 32], [0x33; 32]])
        .expect("Fase 8: custody creation must succeed");
    let mut vault = CrossChainAssetVault::new(ChainId::Ethereum, *b"wETH0000", custody);

    let mint_id = vault
        .process_external_lock_and_mint(q(1_000_000), ChainId::Ethereum, [0x99; 32], 1_700_000_000)
        .expect("Fase 8: lock-and-mint must succeed");
    assert_ne!(mint_id, [0u8; 32]);

    assert!(
        vault.audit_conservation().unwrap(),
        "Fase 8: value conservation must hold"
    );
    assert_eq!(vault.net_active_wrapped_supply(), q(1_000_000));

    // ─── Fase 9: Oracle-Free State Read Relay ─────────────────────────────────
    let read_tracker = build_tracker(ChainId::Ethereum, 3, 8, state_root);
    let query = StateReadQuery::new(ChainId::Ethereum, target_address, storage_slot, 4);
    let read_resp = DecentralizedStateReadRelay::verify_state_read(
        &query,
        storage_value,
        &[proof_node],
        &read_tracker,
    )
    .expect("Fase 9: oracle-free state read must succeed");
    assert!(read_resp.verified, "Fase 9: state read must be verified");

    // ─── Fase 10: Cross-Domain Identity Resolution ───────────────────────────
    let mut resolver = SovereignIdentityResolver::new();
    let aurion_key = [0xAA; 32];
    let digest = CrossDomainIdentityBinding::commitment_digest(
        &aurion_key,
        ChainId::Ethereum,
        &target_address,
    );
    let sig = CrossDomainIdentityBinding::generate_test_signature(&target_address, &digest);

    resolver
        .register_binding(CrossDomainIdentityBinding {
            aurion_pubkey: aurion_key,
            foreign_chain: ChainId::Ethereum,
            foreign_address: target_address,
            attestation_sig: sig,
        })
        .expect("Fase 10: identity binding must succeed");

    let resolved = resolver.resolve_foreign_address(ChainId::Ethereum, &target_address);
    assert_eq!(
        resolved,
        Some(&aurion_key),
        "Fase 10: identity must resolve correctly"
    );

    // ─── Fase 11: Anti-Replay Nullifier ──────────────────────────────────────
    let mut nullifier_reg = UniversalNullifierRegistry::new();
    let nullifier = msg.compute_nullifier();

    nullifier_reg
        .register_nullifier(nullifier, ChainId::Ethereum, msg.packet_id, 1_700_100_000)
        .expect("Fase 11: nullifier registration must succeed");

    let replay = nullifier_reg.register_nullifier(
        nullifier,
        ChainId::Ethereum,
        msg.packet_id,
        1_700_100_001,
    );
    assert!(replay.is_err(), "Fase 11: replay must be rejected");

    // ─── Fase 12: Anomali → Circuit Break → Governance Reset ─────────────────
    let mut circuit_breaker = BridgeCircuitBreaker::new();

    let tripped = circuit_breaker.report_anomaly(AnomalyEvent {
        bridge_chain: ChainId::Ethereum,
        event_hash: AnomalyEvent::compute_hash(ChainId::Ethereum, 2, 500),
        severity: AnomalySeverity::Critical,
        detected_at_slot: 500,
    });

    assert!(tripped, "Fase 12: Critical anomaly must trip circuit");
    assert!(
        circuit_breaker.is_halted(ChainId::Ethereum),
        "Fase 12: Ethereum must be halted"
    );
    assert!(
        !circuit_breaker.is_halted(ChainId::AurionL1),
        "Fase 12: AurionL1 must NOT be halted"
    );
    assert!(
        !circuit_breaker.is_halted(ChainId::Bitcoin),
        "Fase 12: Bitcoin must NOT be halted"
    );

    let gov_token = circuit_breaker.compute_reset_token(ChainId::Ethereum);
    circuit_breaker
        .governance_reset(ChainId::Ethereum, &gov_token)
        .expect("Fase 12: governance reset must succeed");
    assert!(
        !circuit_breaker.is_halted(ChainId::Ethereum),
        "Fase 12: bridge must resume"
    );

    // ─── Final: Burn-and-Unlock (Siklus Lengkap) ────────────────────────────
    let burn_id = vault
        .process_burn_for_external_unlock(
            q(1_000_000),
            ChainId::Ethereum,
            [0x01; 32],
            1_700_100_000,
        )
        .expect("Final: burn-for-unlock must succeed");

    let k1 = [0x11; 32];
    let k2 = [0x22; 32];
    let k3 = [0x33; 32];
    let s1 = ThresholdCustodyAdapter::generate_test_signature(&k1, &burn_id);
    let s2 = ThresholdCustodyAdapter::generate_test_signature(&k2, &burn_id);
    let s3 = ThresholdCustodyAdapter::generate_test_signature(&k3, &burn_id);

    vault
        .authorize_external_unlock(burn_id, &[(k1, s1), (k2, s2), (k3, s3)])
        .expect("Final: authorize unlock must succeed");

    assert_eq!(
        vault.net_active_wrapped_supply(),
        q(0),
        "Final: wrapped supply must be zero after full cycle"
    );
    assert!(
        vault.audit_conservation().unwrap(),
        "Final: conservation invariant must hold"
    );
}

/// Adversarial: 2-of-3 Fraudulent provers block transfer via L4SecurityGate.
#[test]
fn l4_lifecycle_e2e_adversarial_fraud_blocked() {
    let mut gate = L4SecurityGate::new(q(10_000_000), 1000);
    let claim = MultiProverEngine::compute_claim_id(&[0xFF; 32], ChainId::Bitcoin);

    gate.multi_prover
        .submit_verdict(claim, ProverId::LightClient, ProverVerdict::Fraudulent);
    gate.multi_prover
        .submit_verdict(claim, ProverId::ZkStateProof, ProverVerdict::Fraudulent);
    gate.multi_prover
        .submit_verdict(claim, ProverId::OptimisticWatcher, ProverVerdict::Valid);

    let result = gate.approve_transfer(claim, ChainId::Bitcoin, q(1_000_000), 1);
    assert!(
        result.is_err(),
        "Adversarial: fraudulent claim must be blocked"
    );
    assert!(
        result.unwrap_err().contains("Fraudulent"),
        "Adversarial: error must indicate Fraudulent verdict"
    );
}

/// Adversarial: rate limiter blocks excessive volume within single window.
#[test]
fn l4_lifecycle_e2e_adversarial_rate_limit_exceeded() {
    let mut limiter = FinancialRateLimiter::new(q(1_000_000), 100);

    limiter
        .record_transfer(ChainId::Ethereum, q(700_000), 1)
        .expect("first: must succeed");

    let over = limiter.record_transfer(ChainId::Ethereum, q(400_000), 50);
    assert!(over.is_err(), "Adversarial: exceeding rate limit must fail");
}

/// Adversarial: vault burn exceeding active supply is rejected.
#[test]
fn l4_lifecycle_e2e_adversarial_burn_overflow_rejected() {
    let custody = ThresholdCustodyAdapter::new(vec![[0x01; 32]]).unwrap();
    let mut vault = CrossChainAssetVault::new(ChainId::Ethereum, *b"wETH0000", custody);

    // No minted supply → burn must fail
    let result =
        vault.process_burn_for_external_unlock(q(1_000_000), ChainId::Ethereum, [0; 32], 1_000);
    assert!(
        result.is_err(),
        "Adversarial: burn without supply must be rejected"
    );
}

/// Adversarial: invalid governance token must NOT re-open circuit.
#[test]
fn l4_lifecycle_e2e_adversarial_invalid_governance_token() {
    let mut cb = BridgeCircuitBreaker::new();

    cb.report_anomaly(AnomalyEvent {
        bridge_chain: ChainId::Solana,
        event_hash: AnomalyEvent::compute_hash(ChainId::Solana, 1, 999),
        severity: AnomalySeverity::Critical,
        detected_at_slot: 999,
    });

    assert!(
        cb.is_halted(ChainId::Solana),
        "Adversarial: Solana must be halted"
    );

    let bad_token = [0xDE; 32];
    let result = cb.governance_reset(ChainId::Solana, &bad_token);
    assert!(
        result.is_err(),
        "Adversarial: invalid token must NOT reset circuit"
    );
    assert!(
        cb.is_halted(ChainId::Solana),
        "Adversarial: Solana must remain halted"
    );
}
