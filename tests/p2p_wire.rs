//! Pengujian Integrasi P2P Wire Framing, Mutual Handshake, dan Reputasi Peer.
//! Memvalidasi Kepatuhan P2P Aurion (Dokumen 06).

use aurion::core::Signature;
use aurion::crypto::{blake3_hash, Keypair};
use aurion::wire::handshake::{
    validate_handshake_ack, validate_handshake_hello, HandshakeAck, HandshakeError, HandshakeHello,
};
use aurion::wire::messages::*;
use aurion::wire::peer::{PeerRecord, PeerState, PeerViolation};
use aurion::wire::zenoh_transport::AurionKeyExpressions;
use std::net::SocketAddr;

#[test]
fn test_canonical_handshake_mutual_success() {
    let node_a_key = Keypair::from_seed(&[10u8; 32]);
    let node_b_key = Keypair::from_seed(&[20u8; 32]);

    let chain_id = 1u64;
    let genesis_hash = blake3_hash(b"AURION-GENESIS-CANONICAL-STATE");
    let current_time = 1_773_570_000u64;

    // Node A membuat HandshakeHello
    let hello = HandshakeHello::new(chain_id, genesis_hash, 100, current_time, &node_a_key);

    // Node B memvalidasi HandshakeHello
    let validation_result = validate_handshake_hello(&hello, chain_id, &genesis_hash, current_time);
    assert!(validation_result.is_ok(), "HandshakeHello valid wajib diterima");

    // Node B membalas dengan HandshakeAck
    let ack = HandshakeAck::new(105, current_time, &node_b_key);

    // Node A memvalidasi HandshakeAck
    let ack_result = validate_handshake_ack(&ack, current_time);
    assert!(ack_result.is_ok(), "HandshakeAck valid wajib diterima");
}

#[test]
fn test_handshake_rejection_on_mismatched_genesis() {
    let node_key = Keypair::from_seed(&[11u8; 32]);
    let correct_genesis = blake3_hash(b"AURION-CANONICAL-GENESIS");
    let alien_genesis = blake3_hash(b"AURION-TESTNET-GENESIS");
    let current_time = 1_773_570_000u64;

    // Peer dari jaringan lain mengirim genesis yang salah
    let hello = HandshakeHello::new(1, alien_genesis, 50, current_time, &node_key);

    let res = validate_handshake_hello(&hello, 1, &correct_genesis, current_time);
    assert_eq!(res, Err(HandshakeError::GenesisMismatch));
}

#[test]
fn test_handshake_rejection_on_clock_drift_exceeded() {
    let node_key = Keypair::from_seed(&[12u8; 32]);
    let genesis_hash = blake3_hash(b"GENESIS");
    let local_time = 1_773_570_000u64;
    let drift_time = local_time + 16; // Lebih dari 15 detik batas toleransi

    let hello = HandshakeHello::new(1, genesis_hash, 10, drift_time, &node_key);

    let res = validate_handshake_hello(&hello, 1, &genesis_hash, local_time);
    assert!(matches!(res, Err(HandshakeError::ClockDriftExceeded { .. })));
}

#[test]
fn test_handshake_rejection_on_forged_signature() {
    let node_key = Keypair::from_seed(&[13u8; 32]);
    let genesis_hash = blake3_hash(b"GENESIS");
    let current_time = 1_773_570_000u64;

    let mut hello = HandshakeHello::new(1, genesis_hash, 10, current_time, &node_key);
    // Palsukan tanda tangan
    hello.signature = Signature([0xFFu8; 64]);

    let res = validate_handshake_hello(&hello, 1, &genesis_hash, current_time);
    assert_eq!(res, Err(HandshakeError::InvalidSignature));
}

#[test]
fn test_peer_reputation_scoring_and_penalties() {
    let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
    let current_time = 1_000u64;
    let mut peer = PeerRecord::new(addr, current_time);

    assert_eq!(peer.score, 100);
    assert_eq!(peer.state, PeerState::Connecting);

    // Penalti 1: Checksum Blake3 tidak cocok (-50 poin)
    peer.apply_penalty(PeerViolation::ChecksumMismatch, current_time);
    assert_eq!(peer.score, 50);
    assert_eq!(peer.state, PeerState::Throttled, "Skor <= 50 wajib masuk Throttled");

    // Penalti 2: Magic bytes salah (-100 poin)
    peer.apply_penalty(PeerViolation::InvalidMagic, current_time);
    assert_eq!(peer.score, -50);
    assert!(peer.score <= 0);
    assert_eq!(peer.state, PeerState::Banned, "Skor <= 0 wajib di-Ban 24 jam");
    assert!(peer.is_banned(current_time + 3600));
}

#[test]
fn test_zenoh_key_expressions_routing() {
    let keys = AurionKeyExpressions::new(1);

    assert_eq!(keys.key_for_message_type(MSG_TX_GOSSIP), "aurion/net/1/mempool/tx");
    assert_eq!(keys.key_for_message_type(MSG_BFT_PROPOSAL), "aurion/net/1/bft/proposal");
    assert_eq!(keys.key_for_message_type(MSG_BFT_PREVOTE), "aurion/net/1/bft/votes");
    assert_eq!(keys.key_for_message_type(MSG_BFT_PRECOMMIT), "aurion/net/1/bft/votes");
    assert_eq!(keys.key_for_message_type(MSG_BFT_COMMIT_CERT), "aurion/net/1/bft/cert");
    assert_eq!(keys.key_for_message_type(MSG_HANDSHAKE_HELLO), "aurion/net/1/handshake/hello");
    assert_eq!(keys.peer_announce, "aurion/net/v1/peers/announce");
}
