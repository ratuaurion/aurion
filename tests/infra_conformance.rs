#![forbid(unsafe_code)]

//! Aurion Layer-5 Global Distributed Infrastructure Conformance Test Harness.
//!
//! 12 Pilar Pengujian Kepatuhan (REQ-L5-01..12) sesuai Dokumen Aturan Aplikasi 20
//! (20-L5-GLOBAL-INFRASTRUCTURE-BLUEPRINT.md).

use aurion::infrastructure::{
    compute_node_id, AgentExecutive, AgentMandate, AntiDdosShield,
    ArbitrationEngine, ComputeEngine, ComputeJob, DasSamplingClient,
    DataAvailabilityMatrix, DdosFilterDecision, DelegatedAction, EdgeRelayMesh, FraudChallenge,
    IndexingMesh, IndexingQuery, M2MClearingHouse, M2MContract, MeteredUsageReceipt,
    NodeLifecycleStatus, NodeRegistrationRequest, NodeRegistry, OffChainBalanceProof,
    ProofOfRetrievability, QueryAttestation, RelayPeer, ReputationEngine, ServiceMetric,
    SovereignDid, StorageGrid, StorageManifest, StreamingPaymentEngine, VerifiableCredential,
    ViolationType, ZkComputeAttestation, L5_CHALLENGE_WINDOW_SLOTS, L5_MIN_NODE_COLLATERAL_QUANTA,
    L5_STORAGE_CHUNK_BYTES, L5_UNBONDING_DELAY_SLOTS,
};
use aurion::infrastructure::types::NodeType;
use aurion::primitives::core::{Address, Quantum};
use blake3::Hasher;
use ed25519_dalek::{Signer, SigningKey};

fn q(v: u128) -> Quantum {
    Quantum::new(v)
}

fn sample_keypair(seed: u8) -> (SigningKey, [u8; 32]) {
    let sk = SigningKey::from_bytes(&[seed; 32]);
    let pk = sk.verifying_key().to_bytes();
    (sk, pk)
}

fn register_test_worker(registry: &mut NodeRegistry, slot: u64, seed: u8) -> [u8; 32] {
    let (sk, pk) = sample_keypair(seed);
    let collateral = q(L5_MIN_NODE_COLLATERAL_QUANTA);
    let endpoint = format!("https://node-{seed}.aurion.network");

    let digest = NodeRegistrationRequest::compute_registration_digest(
        &pk,
        NodeType::ComputeWorker,
        collateral,
        &endpoint,
    );
    let signature = sk.sign(&digest).to_bytes();

    let req = NodeRegistrationRequest {
        pubkey: pk,
        node_type: NodeType::ComputeWorker,
        collateral,
        endpoint,
        signature,
    };

    registry.register_node(req, slot).expect("Registration must succeed")
}

// -----------------------------------------------------------------------------
// REQ-L5-01: Decentralized Node Registry, Collateral Staking & Unbonding Queue
// -----------------------------------------------------------------------------
#[test]
fn req_l5_01_node_registry_staking_and_unbonding() {
    let mut registry = NodeRegistry::new();
    let (sk, pk) = sample_keypair(0x11);
    let collateral = q(L5_MIN_NODE_COLLATERAL_QUANTA);
    let endpoint = "https://validator-node.aurion.network".to_string();

    let digest = NodeRegistrationRequest::compute_registration_digest(
        &pk,
        NodeType::DasNode,
        collateral,
        &endpoint,
    );
    let signature = sk.sign(&digest).to_bytes();

    let req = NodeRegistrationRequest {
        pubkey: pk,
        node_type: NodeType::DasNode,
        collateral,
        endpoint,
        signature,
    };

    let node_id = registry.register_node(req, 1_000).expect("REQ-L5-01: Registration must succeed");
    let node = registry.get_node(&node_id).expect("Node must be indexed");
    assert_eq!(node.status, NodeLifecycleStatus::ActiveNode);
    assert_eq!(node.collateral, collateral);

    // Request unbonding
    registry.initiate_unbonding(&node_id, 1_050).expect("Unbonding initiation must succeed");
    assert_eq!(
        registry.get_node(&node_id).unwrap().status,
        NodeLifecycleStatus::Unbonding
    );

    // Premature withdrawal must fail
    assert!(
        registry.complete_unbonding(&node_id, 1_050 + L5_UNBONDING_DELAY_SLOTS - 1).is_err(),
        "REQ-L5-01: Premature withdrawal must fail"
    );

    // Finalize withdrawal after unbonding delay passes
    let returned_collateral = registry
        .complete_unbonding(&node_id, 1_050 + L5_UNBONDING_DELAY_SLOTS)
        .expect("REQ-L5-01: Finalizing unbonding must succeed after delay");
    assert_eq!(returned_collateral, collateral);
    assert_eq!(
        registry.get_node(&node_id).unwrap().status,
        NodeLifecycleStatus::Retired
    );
}

// -----------------------------------------------------------------------------
// REQ-L5-02: Verifiable Decentralized Compute Engine (Zk-STARK / Optimistic)
// -----------------------------------------------------------------------------
#[test]
fn req_l5_02_verifiable_compute_engine_lifecycle() {
    let (_worker_sk, worker_pk) = sample_keypair(0x22);
    let worker_node_id = compute_node_id(&worker_pk);

    let program_hash = [0xAA; 32];
    let input_bytes = b"BATCH_QUERY_INPUT".to_vec();

    let job = ComputeJob::new(
        program_hash,
        input_bytes,
        100_000,
        q(50_000),
        [0x01; 32],
        10_000,
    );

    // Worker executes job and provides cryptographic attestation
    let receipt = ComputeEngine::execute_job(&job, worker_node_id, |input| {
        let mut out = input.to_vec();
        out.reverse();
        (out, 45_000)
    })
    .expect("REQ-L5-02: Job execution must succeed");

    assert_eq!(receipt.instructions_executed, 45_000);
    assert!(ComputeEngine::verify_receipt(&receipt));
    assert!(ZkComputeAttestation::verify_attestation(
        &receipt.task_id,
        &receipt.output_hash,
        receipt.instructions_executed,
        &receipt.attestation_proof,
    ));
}

// -----------------------------------------------------------------------------
// REQ-L5-03: Distributed Content-Addressed Storage Grid (Blake3 & PoR)
// -----------------------------------------------------------------------------
#[test]
fn req_l5_03_content_addressed_storage_grid_and_por() {
    let mut grid = StorageGrid::new();
    let owner = [0xBB; 32];
    let keeper_id = compute_node_id(&[0x33; 32]);

    let raw_payload = vec![0xEE; L5_STORAGE_CHUNK_BYTES * 2 + 500]; // 3 chunks
    let rent = q(100);
    let (manifest, chunks) = StorageManifest::create(&raw_payload, owner, rent);

    assert_eq!(manifest.chunk_count, 3);
    assert_eq!(chunks.len(), 3);
    assert_eq!(manifest.total_bytes, raw_payload.len() as u64);

    grid.register_manifest(manifest.clone());
    for &cid in &manifest.chunk_ids {
        grid.allocate_replica(cid, keeper_id);
    }

    assert_eq!(grid.get_replicas(&manifest.chunk_ids[0]), &[keeper_id]);

    // Host node generates and verifies Proof of Retrievability (PoR)
    let proof_path = manifest.generate_merkle_proof(0).expect("Proof generation must succeed");
    let por = ProofOfRetrievability {
        chunk_index: 0,
        chunk_id: manifest.chunk_ids[0],
        merkle_path: proof_path,
    };

    assert!(
        por.verify(&manifest.storage_root),
        "REQ-L5-03: Proof of Retrievability must verify against storage root"
    );
}

// -----------------------------------------------------------------------------
// REQ-L5-04: Data Availability Sampling (DAS) & 2D Erasure Coding
// -----------------------------------------------------------------------------
#[test]
fn req_l5_04_data_availability_sampling_and_erasure_coding() {
    // 2x2 original cells (k=2) -> expanded to 4x4 (size=4)
    let original = vec![
        vec![[0x01; 32], [0x02; 32]],
        vec![[0x03; 32], [0x04; 32]],
    ];
    let matrix = DataAvailabilityMatrix::build(&original)
        .expect("REQ-L5-04: 2D DA matrix encoding must succeed");

    assert_eq!(matrix.k, 2);
    assert_eq!(matrix.size, 4);

    let da_root = matrix.da_root;
    assert_ne!(da_root, [0u8; 32]);

    // Sample coordinates
    let sample = matrix.get_sample(1, 3).expect("Valid coordinate must sample");
    assert_eq!(sample.row, 1);
    assert_eq!(sample.col, 3);

    assert!(
        DasSamplingClient::verify_sampling_session(&[sample], &da_root),
        "REQ-L5-04: DAS sample must verify against Merkle-Blake3 DA root"
    );
}

// -----------------------------------------------------------------------------
// REQ-L5-05: Decentralized Indexing Mesh with Zero-Fabrication Provenance
// -----------------------------------------------------------------------------
#[test]
fn req_l5_05_indexing_mesh_zero_fabrication_provenance() {
    let mut mesh = IndexingMesh::new();
    let (indexer_sk, indexer_pk) = sample_keypair(0x55);
    let indexer_id = compute_node_id(&indexer_pk);

    mesh.register_indexer(indexer_id, indexer_pk);
    assert!(mesh.is_indexer_registered(&indexer_id));

    let query = IndexingQuery::new(
        "AccountBalance".to_string(),
        [0x01; 32],
        120_000,
        [0x02; 32],
    );

    let result_payload = b"Account: aur1xyz... Balance: 500000000".to_vec();
    let leaf = QueryAttestation::compute_leaf_hash(&result_payload);
    let sibling = [0x99; 32];

    let mut hasher = Hasher::new();
    hasher.update(b"AURION-L5-INDEX-MERKLE-NODE-V1");
    hasher.update(&leaf);
    hasher.update(&sibling);
    let canonical_state_root = *hasher.finalize().as_bytes();

    let digest = QueryAttestation::compute_attestation_digest(
        &query.query_id,
        &result_payload,
        &canonical_state_root,
    );
    let signature = indexer_sk.sign(&digest).to_bytes();

    let attestation = QueryAttestation {
        query_id: query.query_id,
        indexer_id,
        indexer_pubkey: indexer_pk,
        result_payload,
        l1_state_root: canonical_state_root,
        inclusion_proof: vec![sibling],
        signature,
    };

    assert!(
        attestation.verify_zero_fabrication(&canonical_state_root),
        "REQ-L5-05: Attestation must verify against canonical state root"
    );

    // Mismatched state root must be rejected (Zero-Fabrication Mandate: AUR-L5-DATA-002)
    let fabricated_state_root = [0xEE; 32];
    assert!(
        !attestation.verify_zero_fabrication(&fabricated_state_root),
        "REQ-L5-05: Fabricated state root must fail verification"
    );
}

// -----------------------------------------------------------------------------
// REQ-L5-06: Sovereign Identity (DID) & Cryptographic Reputation Engine
// -----------------------------------------------------------------------------
#[test]
fn req_l5_06_sovereign_did_and_reputation_dynamics() {
    let (issuer_sk, issuer_pk) = sample_keypair(0x66);
    let issuer_did = SovereignDid::from_pubkey(&issuer_pk);
    let subject_address = Address::from_bytes([0x12; 32]);
    let subject_did = SovereignDid::from_address(&subject_address);

    let claim_type = "CertifiedComputeProvider";
    let claim_value = b"TEE_HARDWARE=TRUE, RAM_GB=128";
    let issued_at = 1_000;
    let expires_at = 200_000;

    let digest = VerifiableCredential::compute_digest(
        &issuer_did,
        &subject_did,
        claim_type,
        claim_value,
        issued_at,
        expires_at,
    );
    let signature = issuer_sk.sign(&digest).to_bytes();

    let vc = VerifiableCredential {
        credential_id: [0x88; 32],
        issuer_did,
        subject_did,
        claim_type: claim_type.to_string(),
        claim_value: claim_value.to_vec(),
        issued_at_timestamp: issued_at,
        expires_at_timestamp: expires_at,
        signature,
    };

    assert!(
        vc.verify(&issuer_pk, 5_000),
        "REQ-L5-06: Verifiable credential signature must verify"
    );
    assert!(
        !vc.verify(&issuer_pk, 250_000),
        "Expired credential must be rejected"
    );

    // Reputation dynamics
    let mut rep = ReputationEngine::new();
    let node_id = compute_node_id(&issuer_pk);

    assert_eq!(rep.get_score(&node_id), 10_000); // 100.00%
    rep.record_infraction(&node_id, 2_000);
    assert_eq!(rep.get_score(&node_id), 8_000); // 80.00%

    rep.record_successful_service(&node_id, 500);
    assert_eq!(rep.get_score(&node_id), 8_500); // 85.00%
}

// -----------------------------------------------------------------------------
// REQ-L5-07: Streaming Micropayments & Off-Chain State Channels
// -----------------------------------------------------------------------------
#[test]
fn req_l5_07_streaming_payment_exact_balance_conservation() {
    let mut engine = StreamingPaymentEngine::new();
    let (sender_sk, sender_pk) = sample_keypair(0x77);
    let sender_addr = Address::from_bytes(sender_pk);
    let receiver_addr = Address::from_bytes([0x88; 32]);

    let deposit = q(1_000_000);
    let channel_id = engine
        .open_channel(sender_addr, sender_pk, receiver_addr, deposit)
        .expect("REQ-L5-07: Channel open must succeed");

    assert!(engine.audit_balance_conservation());

    // Stream micropayment: cumulative amount = 350,000 Quanta
    let transferred = q(350_000);
    let sequence = 1;

    let digest = OffChainBalanceProof::compute_digest(&channel_id, transferred, sequence);
    let sig = sender_sk.sign(&digest).to_bytes();

    let proof = OffChainBalanceProof {
        channel_id,
        cumulative_amount_quanta: transferred,
        nonce: sequence,
        signature: sig,
    };

    assert!(
        proof.verify_signature(&sender_pk),
        "REQ-L5-07: Off-chain balance proof signature must verify"
    );

    let (payout_recip, refund_send) = engine
        .cooperative_close(&proof)
        .expect("REQ-L5-07: Channel settlement must succeed");

    assert_eq!(payout_recip, transferred);
    assert_eq!(
        payout_recip.checked_add(refund_send).unwrap(),
        deposit,
        "REQ-L5-07: Balance Conservation Invariant (AUR-L5-PREC-002)"
    );
    assert!(engine.audit_balance_conservation());
}

// -----------------------------------------------------------------------------
// REQ-L5-08: M2M Autonomous Resource Metering & Settlement
// -----------------------------------------------------------------------------
#[test]
fn req_l5_08_m2m_autonomous_metering_and_clearing() {
    let mut clearing = M2MClearingHouse::new();
    let provider = [0x44; 32];
    let consumer = [0x55; 32];
    let (consumer_sk, consumer_pk) = sample_keypair(0x88);

    let contract = M2MContract::new(
        provider,
        consumer,
        consumer_pk,
        ServiceMetric::PerComputeUnit,
        q(2), // 2 Quanta per cycle
        q(500_000),
    );
    let contract_id = contract.contract_id;

    clearing.register_contract(contract);

    // Metered usage: 15,000 cycles * 2 = 30,000 Quanta fee
    let usage_units = 15_000;
    let total_cost = q(30_000);
    let seq = 1;

    let digest = MeteredUsageReceipt::compute_digest(&contract_id, usage_units, total_cost, seq);
    let sig = consumer_sk.sign(&digest).to_bytes();

    let receipt = MeteredUsageReceipt {
        contract_id,
        units_consumed: usage_units,
        total_owed_quanta: total_cost,
        nonce: seq,
        consumer_signature: sig,
    };

    assert!(receipt.verify_signature(&consumer_pk));

    let settled = clearing
        .clear_metered_usage(&receipt)
        .expect("REQ-L5-08: M2M usage clearing must succeed");

    assert_eq!(settled, total_cost);
    let updated = clearing.get_contract(&contract_id).unwrap();
    assert_eq!(updated.settled_quanta, total_cost);
}

// -----------------------------------------------------------------------------
// REQ-L5-09: Autonomous Agent Executive Runtime & Cryptographic Mandates
// -----------------------------------------------------------------------------
#[test]
fn req_l5_09_agent_executive_mandate_and_spending_caps() {
    let mut exec = AgentExecutive::new();
    let (user_sk, user_pk) = sample_keypair(0x99);
    let user_addr = Address::from_bytes(user_pk);
    let (agent_sk, agent_pk) = sample_keypair(0xAA);

    let allowed_ops = vec!["SWAP_TOKENS".to_string(), "PROVISION_STORAGE".to_string()];
    let spending_cap = q(200_000);
    let valid_until = 700_000;

    let digest = AgentMandate::compute_mandate_digest(
        &user_addr,
        &agent_pk,
        &allowed_ops,
        spending_cap,
        valid_until,
    );
    let principal_sig = user_sk.sign(&digest).to_bytes();

    let mandate_id = [0x55; 32];
    let mandate = AgentMandate {
        mandate_id,
        principal_address: user_addr,
        principal_pubkey: user_pk,
        agent_pubkey: agent_pk,
        allowed_operations: allowed_ops,
        spending_cap_quanta: spending_cap,
        cumulative_spent_quanta: q(0),
        valid_until_slot: valid_until,
        principal_signature: principal_sig,
    };

    let mid = exec.register_mandate(mandate).expect("Mandate registration must succeed");

    // Execute permitted action within budget
    let cost1 = q(50_000);
    let action1_digest = DelegatedAction::compute_action_digest(
        &mid,
        "SWAP_TOKENS",
        b"SWAP 100 TOKEN_A FOR TOKEN_B",
        cost1,
        1,
    );
    let action1 = DelegatedAction {
        mandate_id: mid,
        operation: "SWAP_TOKENS".to_string(),
        action_payload: b"SWAP 100 TOKEN_A FOR TOKEN_B".to_vec(),
        cost_quanta: cost1,
        action_nonce: 1,
        agent_signature: agent_sk.sign(&action1_digest).to_bytes(),
    };

    exec.execute_delegated_action(&action1, 650_000)
        .expect("REQ-L5-09: Permitted action within budget must succeed");

    // Action exceeding spending cap must fail
    let cost_excess = q(160_000); // 50,000 + 160,000 = 210,000 > 200,000
    let action_excess_digest = DelegatedAction::compute_action_digest(
        &mid,
        "PROVISION_STORAGE",
        b"STORAGE_EXTEND",
        cost_excess,
        2,
    );
    let action_excess = DelegatedAction {
        mandate_id: mid,
        operation: "PROVISION_STORAGE".to_string(),
        action_payload: b"STORAGE_EXTEND".to_vec(),
        cost_quanta: cost_excess,
        action_nonce: 2,
        agent_signature: agent_sk.sign(&action_excess_digest).to_bytes(),
    };

    assert!(
        exec.execute_delegated_action(&action_excess, 660_000).is_err(),
        "REQ-L5-09: Spending cap violation must be rejected"
    );

    // Unauthorized action must fail
    let cost_unauth = q(1_000);
    let action_unauth_digest = DelegatedAction::compute_action_digest(
        &mid,
        "DRAIN_FUNDS",
        b"",
        cost_unauth,
        3,
    );
    let action_unauth = DelegatedAction {
        mandate_id: mid,
        operation: "DRAIN_FUNDS".to_string(),
        action_payload: vec![],
        cost_quanta: cost_unauth,
        action_nonce: 3,
        agent_signature: agent_sk.sign(&action_unauth_digest).to_bytes(),
    };

    assert!(
        exec.execute_delegated_action(&action_unauth, 670_000).is_err(),
        "REQ-L5-09: Unauthorized action must be rejected"
    );
}

// -----------------------------------------------------------------------------
// REQ-L5-10: P2P Edge Relay & Anti-DDoS Isolation
// -----------------------------------------------------------------------------
#[test]
fn req_l5_10_anti_ddos_shield_and_edge_relay_mesh() {
    let mut mesh = EdgeRelayMesh::new();
    let peer = RelayPeer {
        peer_id: [0x12; 32],
        endpoint: "tcp://192.168.1.100:9000".to_string(),
        capacity_pps: 10_000,
        is_active: true,
    };
    mesh.register_peer(peer);
    assert_eq!(mesh.active_peer_count(), 1);

    let routed = mesh.route_packet(b"hello_edge_relay").unwrap();
    assert_eq!(routed, [0x12; 32]);

    let mut shield = AntiDdosShield::new(5, 0, 5); // 5 max requests per burst, 0 refill, 5 violations before ban
    let client_ip = [0x55; 32];

    // 5 requests allowed
    for _ in 0..5 {
        assert_eq!(shield.inspect_traffic(&client_ip, 100), DdosFilterDecision::Allow);
    }

    // 6th request triggers rate limit violation
    assert_eq!(shield.inspect_traffic(&client_ip, 100), DdosFilterDecision::RateLimited);

    // Repeated violations trigger temporary ban
    for _ in 0..5 {
        let _ = shield.inspect_traffic(&client_ip, 100);
    }
    assert_eq!(shield.inspect_traffic(&client_ip, 100), DdosFilterDecision::Blacklisted);
}

// -----------------------------------------------------------------------------
// REQ-L5-11: Fraud Challenge Arbitration & Automated Slashing
// -----------------------------------------------------------------------------
#[test]
fn req_l5_11_fraud_challenge_arbitration_and_slashing() {
    let mut registry = NodeRegistry::new();
    let worker_node_id = register_test_worker(&mut registry, 800_000, 0xFE);
    let mut arb = ArbitrationEngine::new();

    let whistleblower = Address::from_bytes([0x77; 32]);
    let evidence = b"PROOF_OF_COMPUTE_STARK_VIOLATION";

    let challenge = FraudChallenge::new(
        worker_node_id,
        whistleblower,
        ViolationType::InvalidComputeResult, // 50% penalty
        evidence,
        800_100,
    );

    let cid = arb.file_challenge(challenge, &mut registry, 800_100)
        .expect("REQ-L5-11: Filing fraud challenge must succeed");

    assert_eq!(
        registry.get_node(&worker_node_id).unwrap().status,
        NodeLifecycleStatus::Challenged
    );

    let verdict = arb.adjudicate_challenge(&cid, true, &mut registry, 800_200)
        .expect("Arbitration adjudication must succeed");

    assert!(verdict.convicted);
    assert_eq!(verdict.slashed_quanta, q(L5_MIN_NODE_COLLATERAL_QUANTA / 2));
    assert_eq!(verdict.whistleblower_bounty.as_u128(), verdict.slashed_quanta.as_u128() / 2);
    assert_eq!(verdict.burned_quanta.as_u128(), verdict.slashed_quanta.as_u128() / 2);
    assert_eq!(
        registry.get_node(&worker_node_id).unwrap().status,
        NodeLifecycleStatus::Slashed
    );
}

// -----------------------------------------------------------------------------
// REQ-L5-12: Comprehensive Architectural Invariants Enforcement
// -----------------------------------------------------------------------------
#[test]
fn req_l5_12_architectural_invariants_enforcement() {
    // 1. Economic Security Collateral Constraint
    let mut registry = NodeRegistry::new();
    let (_sk, pk) = sample_keypair(0x12);
    let sub_collateral = q(L5_MIN_NODE_COLLATERAL_QUANTA - 1);
    let req = NodeRegistrationRequest {
        pubkey: pk,
        node_type: NodeType::ComputeWorker,
        collateral: sub_collateral,
        endpoint: "https://invalid.collateral".to_string(),
        signature: [0u8; 64],
    };
    assert!(
        registry.register_node(req, 900_000).is_err(),
        "REQ-L5-12: Sub-collateral registration must be strictly forbidden"
    );

    // 2. Zero-Float Arithmetic Check: Quantum operations must remain exact
    let val_a = q(999_999_999_999);
    let val_b = q(1);
    let sum = val_a.checked_add(val_b).expect("Add must succeed");
    assert_eq!(sum.as_u128(), 1_000_000_000_000);

    // 3. Challenge Window Slot Boundary Check
    let challenge = FraudChallenge::new(
        [0u8; 32],
        Address::from_bytes([0x01; 32]),
        ViolationType::MissingStorageChunk,
        b"data",
        10_000,
    );
    assert_eq!(challenge.challenge_deadline_slot, 10_000 + L5_CHALLENGE_WINDOW_SLOTS);
}
