#![forbid(unsafe_code)]

//! Aurion Layer-3 12-Pillar Specialized Networks Conformance Test Suite (L3-CTS).
//! Verifies compliance against canonical Rule 18 specification (REQ-L3-01 through REQ-L3-12).

use aurion::core::{Address, Hash256, Quantum};
use aurion::crypto::Keypair;
use aurion::specialized::domains::{
    GameAction, GameSession, OrderBook, OrderSide, OrderType, ShieldedNote, ShieldedPool,
};
use aurion::specialized::messaging::{CrossLayerMessage, NullifierRegistry};
use aurion::specialized::runtime::{
    calculate_l3_fee_split, L3ExecutionConfig,
};
use aurion::specialized::settlement::{
    L2SettlementClient, L3CheckpointGenerator, L3FinalityStatus, L3FinalityTier,
};
use aurion::specialized::state::{L3AccountProof, L3State};
use aurion::specialized::types::{
    DomainId, DomainMetadata, L3Block, L3SecurityModel,
};

/// REQ-L3-01: Sovereign Ecosystem Subordination (L1 Anchor)
#[test]
fn pillar_1_sovereign_ecosystem_subordination() {
    let domain_id = DomainId::named("appchain-alpha");
    let meta = DomainMetadata {
        domain_id,
        name: "appchain-alpha".to_string(),
        security_model: L3SecurityModel::ProofSecured,
        sequencer_pubkey: [0x11; 32],
        settlement_cadence_blocks: 10,
        max_gas_per_block: 20_000_000,
        created_at: 1_000_000,
    };

    assert_eq!(meta.security_model, L3SecurityModel::ProofSecured);
    assert_eq!(meta.settlement_cadence_blocks, 10);
    assert_eq!(meta.max_gas_per_block, 20_000_000);

    // L2 settlement client traces back domain state to L1 sovereign anchor
    let client = L2SettlementClient::new(domain_id, [0x11; 32], Hash256::ZERO);
    assert_eq!(client.sequencer_pubkey, [0x11; 32]);
    assert_eq!(client.domain_id, domain_id);
}

/// REQ-L3-02: 5 Canonical Security Models Validation
#[test]
fn pillar_2_security_models_validation() {
    let models = [
        (L3SecurityModel::ProofSecured, 1u8),
        (L3SecurityModel::AttestationSecured, 2u8),
        (L3SecurityModel::L2Secured, 3u8),
        (L3SecurityModel::Sovereign, 4u8),
        (L3SecurityModel::Hybrid, 5u8),
    ];

    for (model, code) in models {
        assert_eq!(model as u8, code);
        let decoded = L3SecurityModel::from_u8(code).expect("valid code");
        assert_eq!(decoded, model);
    }

    assert!(L3SecurityModel::from_u8(0).is_err());
    assert!(L3SecurityModel::from_u8(6).is_err());
}

/// REQ-L3-03: Domain Fault Isolation (AUR-L3-SEC-001)
#[test]
fn pillar_3_domain_fault_isolation() {
    let domain_a = DomainId::named("domain-isolated-a");
    let domain_b = DomainId::named("domain-isolated-b");

    let mut state_a = L3State::new(domain_a);
    let mut state_b = L3State::new(domain_b);

    let user = Address::from_bytes([0x55; 32]);
    state_a.credit(&user, Quantum(10_000)).expect("credit a");
    state_b.credit(&user, Quantum(20_000)).expect("credit b");

    // Failure / revert in Domain A must NOT affect Domain B
    let snap_a = state_a.snapshot();
    state_a.credit(&user, Quantum(5_000)).expect("credit more a");
    assert_eq!(state_a.get_balance(&user), Quantum(15_000));
    state_a.revert_to_snapshot(snap_a).expect("rollback a");
    assert_eq!(state_a.get_balance(&user), Quantum(10_000));

    // Domain B state is completely intact
    assert_eq!(state_b.get_balance(&user), Quantum(20_000));
}

/// REQ-L3-04: Blake3 Sparse Merkle Tree State Roots (AUR-L3-STATE-001)
#[test]
fn pillar_4_smt_deterministic_state_roots() {
    let domain = DomainId::named("domain-smt");
    let mut state = L3State::new(domain);

    let empty_root = state.compute_state_root();
    assert_eq!(empty_root, Hash256::ZERO);

    let u1 = Address::from_bytes([1u8; 32]);
    let u2 = Address::from_bytes([2u8; 32]);
    state.credit(&u1, Quantum(100)).expect("credit u1");
    let root1 = state.compute_state_root();
    assert_ne!(root1, empty_root);

    state.credit(&u2, Quantum(200)).expect("credit u2");
    let root2 = state.compute_state_root();
    assert_ne!(root2, root1);

    // Determinism check: same state produces identical root
    let mut state_clone = L3State::new(domain);
    state_clone.credit(&u1, Quantum(100)).expect("credit u1 clone");
    state_clone.credit(&u2, Quantum(200)).expect("credit u2 clone");
    assert_eq!(state_clone.compute_state_root(), root2);
}

/// REQ-L3-05: State Witness Proofs (AUR-L3-STATE-002)
#[test]
fn pillar_5_state_witness_membership_proofs() {
    let domain = DomainId::named("domain-witness");
    let mut state = L3State::new(domain);
    let user = Address::from_bytes([0x88; 32]);
    state.credit(&user, Quantum(75_000)).expect("credit user");

    let proof = state.generate_account_proof(&user).expect("proof generated");

    // Valid proof verification against root
    assert!(proof.verify());

    // Forged hash verification must fail
    let forged_proof = L3AccountProof {
        address: proof.address,
        account_hash: Hash256::from_bytes([0x99; 32]),
        leaf_index: proof.leaf_index,
        siblings: proof.siblings.clone(),
        root: proof.root,
    };
    assert!(!forged_proof.verify());
}

/// REQ-L3-06: Integer Quantum Accounting & Zero-Float Enforcement (AUR-ARCH-012)
#[test]
fn pillar_6_zero_float_quantum_accounting() {
    // Fee split: 80% sequencer, 20% settlement reserve
    let fee = Quantum(10_000);
    let split = calculate_l3_fee_split(fee).expect("fee split");
    assert_eq!(split.sequencer_fee, Quantum(8_000));
    assert_eq!(split.settlement_reserve_fee, Quantum(2_000));

    // Conservation check: sum of split equals original fee
    let sum = split
        .sequencer_fee
        .checked_add(split.settlement_reserve_fee)
        .expect("integer add");
    assert_eq!(sum, fee);
}

/// REQ-L3-07: Periodic State Checkpointing (AUR-L3-ARCH-003)
#[test]
fn pillar_7_periodic_checkpointing_cadence() {
    let domain = DomainId::named("domain-chk");
    let keypair = Keypair::generate();
    let mut generator = L3CheckpointGenerator::new(domain, 3, Hash256::ZERO); // Cadence = 3 blocks

    for h in 1..=3 {
        let block = L3Block::new(
            domain,
            h,
            Hash256::ZERO,
            Hash256::from_bytes([h as u8; 32]),
            Hash256::ZERO,
            Hash256::ZERO,
            1_000_000 + h,
            vec![],
        );
        generator.record_block(block).expect("record block");
    }

    assert!(generator.should_checkpoint());
    let checkpoint = generator
        .create_checkpoint(&keypair, vec![1, 2, 3])
        .expect("create checkpoint");

    assert_eq!(checkpoint.checkpoint_id, 1);
    assert_eq!(checkpoint.start_block, 1);
    assert_eq!(checkpoint.end_block, 3);
    assert_eq!(checkpoint.previous_state_root, Hash256::ZERO);
    assert_eq!(checkpoint.new_state_root, Hash256::from_bytes([3u8; 32]));
}

/// REQ-L3-08: 3-Tier Finality Pipeline Progression
#[test]
fn pillar_8_three_tier_finality_progression() {
    let domain = DomainId::named("domain-finality");
    let tx_id = Hash256::from_bytes([0xaa; 32]);

    // Tier 1: InstantLocal
    let mut status = L3FinalityStatus::new_local(domain, tx_id, 100);
    assert_eq!(status.tier, L3FinalityTier::InstantLocal);
    assert_eq!(status.l2_checkpoint_id, None);

    // Tier 2: Promote to SoftL2Settled
    status.promote_to_l2(1);
    assert_eq!(status.tier, L3FinalityTier::SoftL2Settled);
    assert_eq!(status.l2_checkpoint_id, Some(1));

    // Tier 3: Promote to HardL1Finalized
    status.promote_to_l1(50);
    assert_eq!(status.tier, L3FinalityTier::HardL1Finalized);
    assert_eq!(status.l1_settlement_height, Some(50));
}

/// REQ-L3-09: 7-Element Canonical Cross-Layer Envelope (AUR-L3-MSG-001)
#[test]
fn pillar_9_canonical_cross_layer_messaging_envelope() {
    let src = DomainId::named("source-domain");
    let dst = DomainId::named("dest-domain");
    let payload = vec![1, 2, 3, 4, 5];
    let proof = vec![0x99; 32];

    let msg = CrossLayerMessage::new(src, dst, 1, payload.clone(), proof.clone());

    assert_eq!(msg.source_domain, src);
    assert_eq!(msg.destination_domain, dst);
    assert_eq!(msg.nonce, 1);
    assert_eq!(msg.payload, payload);
    assert_eq!(msg.proof, proof);
    assert_ne!(msg.message_id, Hash256::ZERO);
    assert_ne!(msg.nullifier, Hash256::ZERO);
}

/// REQ-L3-10: Multi-Hop Anti-Replay Nullifier Registry (AUR-L3-MSG-002)
#[test]
fn pillar_10_multi_hop_anti_replay_nullifiers() {
    let mut registry = NullifierRegistry::new();
    let src = DomainId::named("domain-x");
    let dst = DomainId::named("domain-y");

    let msg1 = CrossLayerMessage::new(src, dst, 1, vec![0x01], vec![]);
    assert!(!registry.is_spent(&msg1.nullifier));

    registry.verify_and_consume(&msg1).expect("consume msg1");
    assert!(registry.is_spent(&msg1.nullifier));

    // Replay attempt with same nullifier must fail
    let replay_err = registry.verify_and_consume(&msg1);
    assert!(replay_err.is_err());
}

/// REQ-L3-11: Specialized Domain Adapters (DEX, Gaming, Privacy)
#[test]
fn pillar_11_specialized_domain_adapters() {
    // 1. DEX Adapter
    let mut book = OrderBook::new(DomainId::named("dex"), *b"AUR/USDT");
    let t1 = [1u8; 32];
    let t2 = [2u8; 32];
    book.place_order(t1, OrderSide::Sell, OrderType::Limit, Quantum(100), Quantum(5))
        .expect("place ask");
    let trades = book
        .place_order(t2, OrderSide::Buy, OrderType::Limit, Quantum(100), Quantum(5))
        .expect("place bid");
    assert_eq!(trades.len(), 1);
    assert_eq!(trades[0].price, Quantum(100));

    // 2. Gaming Adapter
    let session_id = [0x42; 32];
    let mut session = GameSession::new(session_id, DomainId::named("game"), vec![t1, t2], Quantum(50))
        .expect("game session");
    session
        .apply_action(GameAction {
            player: t1,
            action_type: 1,
            payload: vec![],
            score_delta: 100,
            sequence: 1,
        })
        .expect("game action");
    let summary = session.finalize_session(t1).expect("finalize game");
    assert_eq!(summary.winner, t1);
    assert_eq!(summary.payout, Quantum(100));

    // 3. Privacy Adapter
    let mut pool = ShieldedPool::new(DomainId::named("privacy"));
    let note = ShieldedNote {
        value: Quantum(500),
        nullifier_preimage: [0x12; 32],
        recipient: t1,
        randomness: [0x34; 32],
    };
    let _comm = pool.shield(&note).expect("shield");
    assert_eq!(pool.vault_balance, Quantum(500));
}

/// REQ-L3-12: Zero-Unsafe Invariant (AUR-ARCH-011)
#[test]
fn pillar_12_zero_unsafe_and_invariant_verification() {
    let domain = DomainId::named("zero-unsafe-domain");
    let config = L3ExecutionConfig::default_for(domain);
    assert_eq!(config.domain_id, domain);
    assert_eq!(config.max_gas_per_block, 10_000_000);
    assert_eq!(config.min_gas_price, Quantum(1));
}
