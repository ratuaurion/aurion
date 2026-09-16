#![forbid(unsafe_code)]

//! Aurion Layer-5 End-to-End Infrastructure Lifecycle & Byzantine Fault Injection Test Suite.
//!
//! Mensimulasikan siklus hidup node infrastruktur L5, interaksi multi-agen otonom,
//! kanal pembayaran streaming off-chain, grid penyimpanan terdistribusi Blake3,
//! komputasi zero-knowledge, dan injeksi kegagalan Bizantium dengan pemotongan jaminan (slashing).

use aurion::infrastructure::{
    compute_node_id, AgentExecutive, AgentMandate, ArbitrationEngine, DelegatedAction,
    FraudChallenge, NodeLifecycleStatus, NodeRegistrationRequest, NodeRegistry,
    OffChainBalanceProof, ProofOfRetrievability, StorageGrid, StorageManifest,
    StreamingPaymentEngine, ViolationType, L5_MIN_NODE_COLLATERAL_QUANTA,
    L5_UNBONDING_DELAY_SLOTS,
};
use aurion::infrastructure::types::NodeType;
use aurion::primitives::core::{Address, Quantum};
use ed25519_dalek::{Signer, SigningKey};

fn q(v: u128) -> Quantum {
    Quantum::new(v)
}

fn sample_keypair(seed: u8) -> (SigningKey, [u8; 32]) {
    let sk = SigningKey::from_bytes(&[seed; 32]);
    let pk = sk.verifying_key().to_bytes();
    (sk, pk)
}

#[test]
fn test_infra_lifecycle_phase_transitions() {
    let mut registry = NodeRegistry::new();
    let (sk, pk) = sample_keypair(0x10);
    let collateral = q(L5_MIN_NODE_COLLATERAL_QUANTA);
    let endpoint = "https://full-lifecycle-node.aurion.network".to_string();

    let digest = NodeRegistrationRequest::compute_registration_digest(
        &pk,
        NodeType::StorageKeeper,
        collateral,
        &endpoint,
    );
    let sig = sk.sign(&digest).to_bytes();

    let req = NodeRegistrationRequest {
        pubkey: pk,
        node_type: NodeType::StorageKeeper,
        collateral,
        endpoint,
        signature: sig,
    };

    // Phase 1: Registration -> ActiveNode
    let node_id = registry.register_node(req, 1_000).expect("Registration must succeed");
    assert_eq!(registry.get_node(&node_id).unwrap().status, NodeLifecycleStatus::ActiveNode);

    // Phase 2: Serving
    registry.update_status(&node_id, NodeLifecycleStatus::Serving, 1_050).unwrap();
    assert_eq!(registry.get_node(&node_id).unwrap().status, NodeLifecycleStatus::Serving);

    // Phase 3: Auditing
    registry.update_status(&node_id, NodeLifecycleStatus::Auditing, 1_080).unwrap();
    assert_eq!(registry.get_node(&node_id).unwrap().status, NodeLifecycleStatus::Auditing);

    // Phase 4: Challenged
    registry.update_status(&node_id, NodeLifecycleStatus::Challenged, 1_100).unwrap();
    assert_eq!(registry.get_node(&node_id).unwrap().status, NodeLifecycleStatus::Challenged);

    // Phase 5: Slashed
    let (slashed_quanta, remaining_collateral) = registry.slash_node(&node_id, 2_500).unwrap(); // 25%
    assert_eq!(slashed_quanta, q(L5_MIN_NODE_COLLATERAL_QUANTA * 25 / 100));
    assert_eq!(remaining_collateral, q(L5_MIN_NODE_COLLATERAL_QUANTA * 75 / 100));
    assert_eq!(registry.get_node(&node_id).unwrap().status, NodeLifecycleStatus::Slashed);

    // Node 2 tests orderly Unbonding -> Retired lifecycle
    let (sk2, pk2) = sample_keypair(0x20);
    let digest2 = NodeRegistrationRequest::compute_registration_digest(
        &pk2,
        NodeType::DasNode,
        collateral,
        "https://node2.aurion.network",
    );
    let req2 = NodeRegistrationRequest {
        pubkey: pk2,
        node_type: NodeType::DasNode,
        collateral,
        endpoint: "https://node2.aurion.network".to_string(),
        signature: sk2.sign(&digest2).to_bytes(),
    };
    let node_id2 = registry.register_node(req2, 2_000).unwrap();

    // Phase 6: Unbonding
    registry.initiate_unbonding(&node_id2, 2_100).unwrap();
    assert_eq!(registry.get_node(&node_id2).unwrap().status, NodeLifecycleStatus::Unbonding);

    // Phase 7: Retired
    let returned = registry.complete_unbonding(&node_id2, 2_100 + L5_UNBONDING_DELAY_SLOTS).unwrap();
    assert_eq!(returned, collateral);
    assert_eq!(registry.get_node(&node_id2).unwrap().status, NodeLifecycleStatus::Retired);
    assert!(registry.initiate_unbonding(&node_id2, 4_000).is_err());
}

#[test]
fn test_byzantine_fault_injection_and_slashing_adjudication() {
    let mut registry = NodeRegistry::new();
    let mut arb = ArbitrationEngine::new();

    // Register Byzantine worker node 1
    let (worker_sk, worker_pk) = sample_keypair(0x69);
    let collateral = q(L5_MIN_NODE_COLLATERAL_QUANTA);
    let endpoint = "https://byzantine-worker.aurion.network".to_string();

    let digest = NodeRegistrationRequest::compute_registration_digest(
        &worker_pk,
        NodeType::ComputeWorker,
        collateral,
        &endpoint,
    );
    let sig = worker_sk.sign(&digest).to_bytes();

    let req = NodeRegistrationRequest {
        pubkey: worker_pk,
        node_type: NodeType::ComputeWorker,
        collateral,
        endpoint,
        signature: sig,
    };

    let node_id = registry.register_node(req, 10_000).unwrap();
    let whistleblower = Address::from_bytes([0x77; 32]);

    // Attack 1: Compute result fabrication (50% slash)
    let fraud_proof = b"FRAUD_PROOF_COMPUTE_STARK_EVIDENCE";
    let challenge = FraudChallenge::new(
        node_id,
        whistleblower,
        ViolationType::InvalidComputeResult,
        fraud_proof,
        10_050,
    );

    let cid = arb.file_challenge(challenge, &mut registry, 10_050).unwrap();
    let verdict = arb.adjudicate_challenge(&cid, true, &mut registry, 10_100).unwrap();

    assert!(verdict.convicted);
    assert_eq!(verdict.slashed_quanta, q(L5_MIN_NODE_COLLATERAL_QUANTA / 2)); // 50%
    assert_eq!(verdict.whistleblower_bounty, q(L5_MIN_NODE_COLLATERAL_QUANTA / 4));
    assert_eq!(verdict.burned_quanta, q(L5_MIN_NODE_COLLATERAL_QUANTA / 4));
    assert_eq!(arb.total_burned(), verdict.burned_quanta);

    let node = registry.get_node(&node_id).unwrap();
    assert_eq!(node.status, NodeLifecycleStatus::Slashed);
    assert_eq!(node.collateral, q(L5_MIN_NODE_COLLATERAL_QUANTA / 2));

    // Register Node 2 for Attack 2: Critical Data Availability withholding (100% slash)
    let (worker_sk2, worker_pk2) = sample_keypair(0x70);
    let digest2 = NodeRegistrationRequest::compute_registration_digest(
        &worker_pk2,
        NodeType::DasNode,
        collateral,
        "https://da-byzantine.aurion.network",
    );
    let req2 = NodeRegistrationRequest {
        pubkey: worker_pk2,
        node_type: NodeType::DasNode,
        collateral,
        endpoint: "https://da-byzantine.aurion.network".to_string(),
        signature: worker_sk2.sign(&digest2).to_bytes(),
    };
    let node_id2 = registry.register_node(req2, 10_200).unwrap();

    let challenge2 = FraudChallenge::new(
        node_id2,
        whistleblower,
        ViolationType::DataAvailabilityWithholding,
        b"DA_WITHHOLDING_PROOF",
        10_250,
    );
    let cid2 = arb.file_challenge(challenge2, &mut registry, 10_250).unwrap();
    let verdict2 = arb.adjudicate_challenge(&cid2, true, &mut registry, 10_300).unwrap();

    assert!(verdict2.convicted);
    assert_eq!(verdict2.slashed_quanta, collateral); // 100%
    let final_node2 = registry.get_node(&node_id2).unwrap();
    assert_eq!(final_node2.collateral, q(0));

    // Register Node 3 for Exoneration test (False challenge -> Exonerated to ActiveNode)
    let (worker_sk3, worker_pk3) = sample_keypair(0x71);
    let digest3 = NodeRegistrationRequest::compute_registration_digest(
        &worker_pk3,
        NodeType::Indexer,
        collateral,
        "https://honest-indexer.aurion.network",
    );
    let req3 = NodeRegistrationRequest {
        pubkey: worker_pk3,
        node_type: NodeType::Indexer,
        collateral,
        endpoint: "https://honest-indexer.aurion.network".to_string(),
        signature: worker_sk3.sign(&digest3).to_bytes(),
    };
    let node_id3 = registry.register_node(req3, 10_400).unwrap();

    let challenge3 = FraudChallenge::new(
        node_id3,
        whistleblower,
        ViolationType::FabricatedQueryResponse,
        b"BOGUS_CHALLENGE",
        10_450,
    );
    let cid3 = arb.file_challenge(challenge3, &mut registry, 10_450).unwrap();
    let verdict3 = arb.adjudicate_challenge(&cid3, false, &mut registry, 10_500).unwrap();

    assert!(!verdict3.convicted);
    assert_eq!(verdict3.slashed_quanta, q(0));
    let final_node3 = registry.get_node(&node_id3).unwrap();
    assert_eq!(final_node3.status, NodeLifecycleStatus::ActiveNode);
    assert_eq!(final_node3.collateral, collateral);
}

#[test]
fn test_integrated_edge_ecosystem_interaction() {
    // 1. Set up storage grid
    let mut grid = StorageGrid::new();
    let owner = [0x11; 32];
    let host_id = compute_node_id(&[0x22; 32]);

    let storage_data = vec![0xAB; 2_500_000]; // ~2.5 MiB = 3 chunks
    let (manifest, _chunks) = StorageManifest::create(&storage_data, owner, q(100));

    grid.register_manifest(manifest.clone());
    for &cid in &manifest.chunk_ids {
        grid.allocate_replica(cid, host_id);
    }

    let proof_path = manifest.generate_merkle_proof(0).unwrap();
    let por = ProofOfRetrievability {
        chunk_index: 0,
        chunk_id: manifest.chunk_ids[0],
        merkle_path: proof_path,
    };
    assert!(por.verify(&manifest.storage_root));

    // 2. Set up streaming payment channel for reading storage chunks
    let mut payments = StreamingPaymentEngine::new();
    let (reader_sk, reader_pk) = sample_keypair(0x33);
    let reader_addr = Address::from_bytes(reader_pk);
    let host_addr = Address::from_bytes([0x55; 32]);

    let deposit = q(500_000);
    let channel_id = payments.open_channel(reader_addr, reader_pk, host_addr, deposit).unwrap();

    // Stream payment for 3 chunks retrieved @ 25,000 Quanta each = 75,000 Quanta
    let fee = q(75_000);
    let seq = 1;

    let digest = OffChainBalanceProof::compute_digest(&channel_id, fee, seq);
    let sig = reader_sk.sign(&digest).to_bytes();

    let proof = OffChainBalanceProof {
        channel_id,
        cumulative_amount_quanta: fee,
        nonce: seq,
        signature: sig,
    };

    let (payout, returned) = payments.cooperative_close(&proof).unwrap();
    assert_eq!(payout, fee);
    assert_eq!(payout.checked_add(returned).unwrap(), deposit);

    // 3. Autonomous agent manages storage renewal
    let mut exec = AgentExecutive::new();
    let (user_sk, user_pk) = sample_keypair(0x44);
    let user_addr = Address::from_bytes(user_pk);
    let (agent_sk, agent_pk) = sample_keypair(0x66);

    let allowed_ops = vec!["RENEW_STORAGE".to_string()];
    let cap = q(100_000);
    let valid_until = 50_000;

    let digest = AgentMandate::compute_mandate_digest(
        &user_addr,
        &agent_pk,
        &allowed_ops,
        cap,
        valid_until,
    );
    let principal_sig = user_sk.sign(&digest).to_bytes();

    let mid = [0x77; 32];
    let mandate = AgentMandate {
        mandate_id: mid,
        principal_address: user_addr,
        principal_pubkey: user_pk,
        agent_pubkey: agent_pk,
        allowed_operations: allowed_ops,
        spending_cap_quanta: cap,
        cumulative_spent_quanta: q(0),
        valid_until_slot: valid_until,
        principal_signature: principal_sig,
    };
    exec.register_mandate(mandate).unwrap();

    let cost_renew = q(30_000);
    let action_digest = DelegatedAction::compute_action_digest(
        &mid,
        "RENEW_STORAGE",
        &manifest.content_id,
        cost_renew,
        1,
    );
    let renew_action = DelegatedAction {
        mandate_id: mid,
        operation: "RENEW_STORAGE".to_string(),
        action_payload: manifest.content_id.to_vec(),
        cost_quanta: cost_renew,
        action_nonce: 1,
        agent_signature: agent_sk.sign(&action_digest).to_bytes(),
    };
    exec.execute_delegated_action(&renew_action, 40_000).unwrap();

    let m_info = exec.get_mandate(&mid).unwrap();
    assert_eq!(m_info.cumulative_spent_quanta, q(30_000));
}
