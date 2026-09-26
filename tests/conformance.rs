//! Aurion 8-Pillar Protocol Conformance Test Suite.
//! Memverifikasi seluruh subsistem terhadap spesifikasi kanonikal Dokumen 11 & 12.

use aurion::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use aurion::consensus::certificate::{CommitCertificate, ValidatorEntry, ValidatorSet};
use aurion::consensus::header::{BlockHeader, BLOCK_HEADER_BYTES};
use aurion::consensus::vote::{Vote, VOTE_BYTES};
use aurion::core::{
    Address, Hash256, MonetaryError, Quantum, Signature, MASTER_TREASURY_ALLOCATION_QUANTA,
    MAX_SUPPLY_QUANTA,
};
use aurion::crypto::{
    blake3_derive_key, blake3_hash, decode_address_bech32m, derive_address_from_pubkey,
    ed25519_verify_strict, encode_address_bech32m, Keypair, HRP_MAINNET, HRP_TESTNET,
};
use aurion::genesis::build_genesis;
use aurion::state::account::Account;
use aurion::state::monetary::MonetaryState;
use aurion::state::stf::apply_transaction;
use aurion::transaction::types::{Transaction, TxType};
use aurion::transaction::validator::validate_transaction_stateless;
use aurion::wire::frame::{
    parse_network_frame, serialize_network_frame, WireError, WIRE_FRAME_HEADER_BYTES,
};
use aurion::wire::MSG_TX_GOSSIP;
use std::collections::HashMap;

#[test]
fn pillar_1_cryptographic_primitives() {
    // 1.1 Blake3 Hash Standard
    let empty_hash = blake3_hash(b"");
    assert_eq!(
        empty_hash.to_hex(),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );

    let aurion_hash = blake3_hash(b"aurion");
    assert_eq!(
        aurion_hash.to_hex(),
        "c81453c888cdd581753d4b72f466a585e98ffda6f68dbd2fabc5f43d5fee86c8"
    );

    // 1.2 Blake3 KDF
    let derived = blake3_derive_key(
        "AURION-TEST-V1",
        &hex::decode("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f").unwrap(),
    );
    assert_eq!(
        derived.to_hex(),
        "d08662334d813cc3e872910df43129dd5dbf7ba49215352184def9f82ec5e195"
    );

    // 1.3 Ed25519 Keypair & Strict Signature
    let seed =
        hex::decode("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60").unwrap();
    let mut seed_bytes = [0u8; 32];
    seed_bytes.copy_from_slice(&seed);
    let keypair = Keypair::from_seed(&seed_bytes);

    assert_eq!(
        hex::encode(keypair.public_key_bytes()),
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
    );

    let msg = b"AURION-CONSENSUS-TEST-MESSAGE";
    let sig = keypair.sign(msg);
    assert_eq!(
        sig.to_hex(),
        "cb8dba50a61f5269904f7ea575769cfb4d6916af4ef4c984edeca8fbe36194f500b8373535e9806ad93d04921365f9820d8561ef050f2ff6d8e35f3da3452209"
    );
    assert!(ed25519_verify_strict(&keypair.public_key_bytes(), msg, &sig).is_ok());

    // 1.4 Address Derivation & Bech32m Encoding
    let raw_addr = derive_address_from_pubkey(&keypair.public_key_bytes());
    assert_eq!(
        raw_addr.to_hex(),
        "7acb7a9e77ef27c92b8049ec96ea40901febbf3861e97a97f01e7a02f0170253"
    );

    let mainnet_addr = encode_address_bech32m(&raw_addr, HRP_MAINNET).unwrap();
    assert_eq!(
        mainnet_addr,
        "aur10t9h48nhaunuj2uqf8kfd6jqjq07h0ecv85h49lsreaq9uqhqffshf0p6h"
    );
    let decoded = decode_address_bech32m(&mainnet_addr, HRP_MAINNET).unwrap();
    assert_eq!(decoded, raw_addr);

    let testnet_addr = encode_address_bech32m(&raw_addr, HRP_TESTNET).unwrap();
    assert_eq!(
        testnet_addr,
        "aurt10t9h48nhaunuj2uqf8kfd6jqjq07h0ecv85h49lsreaq9uqhqffsjazk8c"
    );
}

#[test]
fn pillar_2_monetary_policy() {
    let one_aur = Quantum::ONE_AUR;
    assert_eq!(one_aur.as_u128(), 1_000_000_000);

    let ten_aur = one_aur.checked_mul(10).unwrap();
    assert_eq!(ten_aur.as_u128(), 10_000_000_000);
    assert_eq!(ten_aur.to_aur_string(), "10.000000000 AUR");

    // Hard Cap Invariant (66M AUR)
    let max = Quantum::MAX_SUPPLY;
    assert_eq!(max.as_u128(), MAX_SUPPLY_QUANTA);
    assert_eq!(
        max.checked_add_bounded(Quantum::ONE),
        Err(MonetaryError::SupplyCapExceeded(MAX_SUPPLY_QUANTA + 1))
    );

    // Fee Allocation (0% burn, 100% validator)
    let fee = Quantum::new(100_000_000);
    let (burned, validator) = MonetaryState::split_fee(fee).unwrap();
    assert_eq!(burned.as_u128(), 0);
    assert_eq!(validator.as_u128(), 100_000_000);
}

#[test]
fn pillar_3_canonical_codec() {
    let original: u64 = 0x0102030405060708;
    let encoded = original.to_canonical_bytes();
    assert_eq!(encoded, vec![1, 2, 3, 4, 5, 6, 7, 8]);

    let decoded = u64::decode_canonical_exact(&encoded).unwrap();
    assert_eq!(decoded, original);

    // Strict zero-trailing rejection
    let mut trailing = encoded.clone();
    trailing.push(0x00);
    assert_eq!(
        u64::decode_canonical_exact(&trailing),
        Err(CodecError::TrailingBytes(1))
    );
}

#[test]
fn pillar_4_transaction_pipeline() {
    let seed = [1u8; 32];
    let keypair = Keypair::from_seed(&seed);
    let sender = keypair.derive_address();
    let recipient = Address::from_bytes([2u8; 32]);

    let mut tx = Transaction {
        version: 1,
        chain_id: 1001,
        tx_type: TxType::Transfer,
        flags: 0,
        sender,
        recipient,
        nonce: 0,
        amount: Quantum::new(500_000_000), // 5 AUR
        fee: Quantum::new(10_000_000),     // 0.1 AUR
        valid_until: 1000,
        payload: vec![],
        signature: Signature::from_bytes([0u8; 64]),
    };

    let preimage = tx.signing_preimage();
    tx.signature = keypair.sign(&preimage);

    assert!(validate_transaction_stateless(&tx, &keypair.public_key_bytes()).is_ok());

    let tx_id = tx.compute_tx_id();
    assert_ne!(tx_id, Hash256::ZERO);
}

#[test]
fn pillar_5_state_transition_execution() {
    let sender = Address::from_bytes([1u8; 32]);
    let recipient = Address::from_bytes([2u8; 32]);
    let proposer = Address::from_bytes([3u8; 32]);

    let mut accounts = HashMap::new();
    accounts.insert(sender, Account::new(Quantum::new(1_000_000_000), 0));

    let mut monetary = MonetaryState::new(Quantum::new(1_000_000_000), Quantum::ZERO);

    let tx = Transaction {
        version: 1,
        chain_id: 1001,
        tx_type: TxType::Transfer,
        flags: 0,
        sender,
        recipient,
        nonce: 0,
        amount: Quantum::new(300_000_000),
        fee: Quantum::new(50_000_000),
        valid_until: 100,
        payload: vec![],
        signature: Signature::from_bytes([0u8; 64]),
    };

    let receipt = apply_transaction(&mut accounts, &mut monetary, &proposer, &tx).unwrap();

    assert_eq!(receipt.burned_fee.as_u128(), 0);
    assert_eq!(receipt.validator_fee.as_u128(), 50_000_000);

    assert_eq!(
        accounts.get(&sender).unwrap().balance.as_u128(),
        650_000_000
    );
    assert_eq!(accounts.get(&sender).unwrap().nonce, 1);
    assert_eq!(
        accounts.get(&recipient).unwrap().balance.as_u128(),
        300_000_000
    );
    assert_eq!(
        accounts.get(&proposer).unwrap().balance.as_u128(),
        50_000_000
    );
    assert_eq!(monetary.total_burned.as_u128(), 0);
}

#[test]
fn pillar_6_consensus_bft() {
    let header = BlockHeader {
        version: 1,
        height: 1,
        round: 0,
        timestamp: 1773532860,
        prev_block_hash: Hash256::ZERO,
        tx_merkle_root: Hash256::from_bytes([2u8; 32]),
        state_root: Hash256::from_bytes([1u8; 32]),
    };

    let encoded_header = header.to_canonical_bytes();
    assert_eq!(encoded_header.len(), BLOCK_HEADER_BYTES);

    let block_hash = header.compute_block_hash();
    assert_ne!(block_hash, Hash256::ZERO);

    // Setup 4 validators with voting weight 10 each (total 40, quorum > 2/3 = 27)
    let seeds = [[1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]];
    let keypairs: Vec<Keypair> = seeds.iter().map(Keypair::from_seed).collect();
    let val_entries: Vec<ValidatorEntry> = keypairs
        .iter()
        .enumerate()
        .map(|(idx, kp)| ValidatorEntry {
            validator_id: Address::from_bytes([(idx + 1) as u8; 32]),
            consensus_pubkey: kp.public_key_bytes(),
            voting_weight: 10,
        })
        .collect();

    let val_set = ValidatorSet::new(val_entries);
    assert_eq!(val_set.quorum_threshold(), 27);

    // Construct 3 precommits (30 voting weight > 27)
    let mut precommits = Vec::new();
    for (idx, kp) in keypairs.iter().enumerate().take(3) {
        let vote = Vote::new_signed(
            kp,
            aurion::consensus::PHASE_PRECOMMIT,
            1,
            0,
            block_hash,
            idx as u32,
        )
        .unwrap();
        assert_eq!(vote.to_canonical_bytes().len(), VOTE_BYTES);
        precommits.push(vote);
    }

    let cert = CommitCertificate {
        block_hash,
        height: 1,
        round: 0,
        precommits,
    };

    assert!(cert.verify(&val_set).is_ok());
}

#[test]
fn pillar_7_wire_framing_and_security() {
    let payload = b"AURION-P2P-PAYLOAD";
    let frame = serialize_network_frame(MSG_TX_GOSSIP, payload).unwrap();
    assert_eq!(frame.len(), WIRE_FRAME_HEADER_BYTES + payload.len());

    let (header, parsed_payload) = parse_network_frame(&frame).unwrap();
    assert_eq!(header.message_type, MSG_TX_GOSSIP);
    assert_eq!(parsed_payload, payload);

    // Tampered payload must fail checksum
    let mut tampered = frame.clone();
    tampered[WIRE_FRAME_HEADER_BYTES] ^= 0xFF;
    assert!(matches!(
        parse_network_frame(&tampered),
        Err(WireError::ChecksumMismatch { .. })
    ));
}

#[test]
fn pillar_8_genesis_block_and_state() {
    let treasury = Address::from_bytes([0xAA; 32]);
    let val_entry = ValidatorEntry {
        validator_id: Address::from_bytes([1u8; 32]),
        consensus_pubkey: [1u8; 32],
        voting_weight: 100,
    };

    let genesis = build_genesis(treasury, vec![val_entry]);

    assert_eq!(genesis.header.height, 0);
    assert_eq!(
        genesis.accounts.get(&treasury).unwrap().balance.as_u128(),
        MASTER_TREASURY_ALLOCATION_QUANTA
    );

    assert_eq!(
        genesis.monetary.total_issued.as_u128(),
        MASTER_TREASURY_ALLOCATION_QUANTA
    );
    assert_eq!(genesis.monetary.total_burned.as_u128(), 0);
}
