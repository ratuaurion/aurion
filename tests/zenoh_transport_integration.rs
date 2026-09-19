#![forbid(unsafe_code)]

use aurion::consensus::bft::{
    BftTransport, Block, BlockProposalEnvelope, ConsensusMessage, Vote, ZenohBftTransport,
    MAX_PROPOSAL_WIRE_BYTES, PHASE_PRECOMMIT,
};
use aurion::consensus::header::BlockHeader;
use aurion::core::Hash256;
use aurion::crypto::Keypair;
use aurion::genesis::builder::GENESIS_CHAIN_ID;
use std::time::Duration;

fn proposal() -> BlockProposalEnvelope {
    let key = Keypair::from_seed(&[0x31; 32]);
    let block = Block::new(
        BlockHeader {
            version: 1,
            height: 1,
            round: 0,
            timestamp: 1,
            prev_block_hash: Hash256::ZERO,
            tx_merkle_root: Hash256::ZERO,
            state_root: Hash256::ZERO,
        },
        Vec::new(),
        None,
    );
    BlockProposalEnvelope::new_signed(block, GENESIS_CHAIN_ID, 0, &key)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_zenoh_bft_transport_peer_exchange_and_self_filter() {
    let mut node0 = ZenohBftTransport::open(0, GENESIS_CHAIN_ID, "tcp/127.0.0.1:17447", &[])
        .await
        .unwrap();
    let mut node1 = ZenohBftTransport::open(
        1,
        GENESIS_CHAIN_ID,
        "tcp/127.0.0.1:17448",
        &["tcp/127.0.0.1:17447"],
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(700)).await;

    let envelope = proposal();
    node0.broadcast_proposal(envelope.clone()).await.unwrap();
    let received = tokio::time::timeout(Duration::from_secs(3), node1.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(received, ConsensusMessage::Proposal(envelope));

    let key = Keypair::from_seed(&[0x41; 32]);
    let vote = Vote::new_signed(&key, PHASE_PRECOMMIT, 1, 0, Hash256::ZERO, 1).unwrap();
    node1.broadcast_vote(vote.clone()).await.unwrap();
    let received = tokio::time::timeout(Duration::from_secs(3), node0.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(received, ConsensusMessage::Vote(vote));

    let self_filter = tokio::time::timeout(Duration::from_millis(250), node0.recv()).await;
    assert!(
        self_filter.is_err(),
        "node 0 must not receive its own proposal"
    );
    let self_filter = tokio::time::timeout(Duration::from_millis(250), node1.recv()).await;
    assert!(self_filter.is_err(), "node 1 must not receive its own vote");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_zenoh_bft_transport_wire_bounds_enforcement() {
    let transport = ZenohBftTransport::open(0, GENESIS_CHAIN_ID, "tcp/127.0.0.1:17449", &[])
        .await
        .unwrap();
    let oversized = Block::new(
        BlockHeader {
            version: 1,
            height: 1,
            round: 0,
            timestamp: 1,
            prev_block_hash: Hash256::ZERO,
            tx_merkle_root: Hash256::ZERO,
            state_root: Hash256::ZERO,
        },
        vec![aurion::transaction::types::Transaction {
            version: 1,
            chain_id: GENESIS_CHAIN_ID,
            tx_type: aurion::transaction::types::TxType::Transfer,
            flags: 0,
            sender: aurion::core::Address([1; 32]),
            recipient: aurion::core::Address([2; 32]),
            nonce: 0,
            amount: aurion::core::Quantum::ZERO,
            fee: aurion::core::Quantum::ZERO,
            valid_until: u64::MAX,
            payload: vec![0; MAX_PROPOSAL_WIRE_BYTES],
            signature: aurion::core::Signature::from_bytes([0; 64]),
        }],
        None,
    );
    let envelope = BlockProposalEnvelope {
        block: oversized,
        proposer_index: 0,
        signature: [0; 64],
    };
    let error = transport.broadcast_proposal(envelope).await.unwrap_err();
    assert!(matches!(
        error,
        aurion::consensus::bft::TransportError::ProposalTooLarge { .. }
    ));
}
