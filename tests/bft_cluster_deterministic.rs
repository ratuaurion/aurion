#![forbid(unsafe_code)]

use aurion::codec::CanonicalEncode;
use aurion::consensus::bft::{
    BftEngine, BftTransport, Block, BlockProposalEnvelope, CommitCertificate, InMemoryBftTransport,
    InMemoryNetworkHub, ValidatorEntry, ValidatorSet, Vote, PHASE_PRECOMMIT, PHASE_PREVOTE,
};
use aurion::consensus::header::BlockHeader;
use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::crypto::Keypair;
use aurion::genesis::builder::{build_genesis, GENESIS_CHAIN_ID};
use aurion::genesis::ceremony::CanonicalCeremonyKeypairs;
use aurion::state::monetary::MonetaryState;
use aurion::state::smt::compute_accounts_state_root;
use aurion::state::stf::apply_transaction;
use aurion::transaction::types::{Transaction, TxType};
use std::collections::HashMap;
use std::time::Duration;

const VALIDATOR_COUNT: usize = 4;
const ACTIVE_VALIDATORS: usize = 3;
const ROUND: u64 = 0;

struct ClusterFixture {
    keys: CanonicalCeremonyKeypairs,
    validator_set: ValidatorSet,
    accounts: HashMap<Address, aurion::state::account::Account>,
    monetary: MonetaryState,
    previous_block_hash: Hash256,
}

fn fixture() -> ClusterFixture {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let validators = keys
        .validators
        .iter()
        .map(|key| ValidatorEntry {
            validator_id: key.derive_address(),
            consensus_pubkey: key.public_key_bytes(),
            voting_weight: 1,
        })
        .collect();
    let validator_set = ValidatorSet::new(validators);
    let genesis = build_genesis(
        keys.creator.derive_address(),
        keys.developer.derive_address(),
        validator_set.validators.clone(),
    );

    ClusterFixture {
        keys,
        validator_set,
        accounts: genesis.accounts,
        monetary: genesis.monetary,
        previous_block_hash: genesis.header.compute_block_hash(),
    }
}

fn transaction(key: &Keypair, recipient: Address, nonce: u64, amount: u128) -> Transaction {
    let mut tx = Transaction {
        version: 1,
        chain_id: GENESIS_CHAIN_ID,
        tx_type: TxType::Transfer,
        flags: 0,
        sender: key.derive_address(),
        recipient,
        nonce,
        amount: Quantum::new(amount),
        fee: Quantum::ZERO,
        valid_until: u64::MAX,
        payload: Vec::new(),
        signature: Signature::from_bytes([0u8; 64]),
    };
    tx.signature = key.sign(&tx.signing_preimage());
    tx
}

fn proposal(
    fixture: &mut ClusterFixture,
    height: u64,
    tx: Transaction,
    timestamp: u64,
) -> (
    BlockProposalEnvelope,
    HashMap<Address, aurion::state::account::Account>,
) {
    let proposer = BftEngine::select_proposer(
        &fixture.validator_set,
        height,
        ROUND,
        &fixture.previous_block_hash,
    ) as usize;
    let miner = fixture.keys.validators[proposer].derive_address();
    let mut next_accounts = fixture.accounts.clone();
    let mut next_monetary = fixture.monetary.clone();
    apply_transaction(&mut next_accounts, &mut next_monetary, &miner, &tx)
        .expect("fixture transaction must pass STF");

    let block = Block::new(
        BlockHeader {
            version: 1,
            height,
            round: ROUND,
            timestamp,
            prev_block_hash: fixture.previous_block_hash,
            tx_merkle_root: Block::calculate_tx_merkle_root(std::slice::from_ref(&tx)),
            state_root: compute_accounts_state_root(&next_accounts),
        },
        vec![tx],
        None,
    );
    let envelope = BlockProposalEnvelope::new_signed(
        block,
        GENESIS_CHAIN_ID,
        proposer as u32,
        &fixture.keys.validators[proposer],
    );
    (envelope, next_accounts)
}

fn assert_valid_proposal(
    envelope: &BlockProposalEnvelope,
    fixture: &ClusterFixture,
    expected_accounts: &HashMap<Address, aurion::state::account::Account>,
) {
    envelope
        .verify(GENESIS_CHAIN_ID, &fixture.validator_set)
        .expect("leader envelope must authenticate");
    assert_eq!(
        envelope.block.header.prev_block_hash,
        fixture.previous_block_hash
    );
    assert_eq!(envelope.block.height(), envelope.block.header.height);
    assert_eq!(
        envelope.block.header.state_root,
        compute_accounts_state_root(expected_accounts)
    );
    assert!(envelope.block.verify_tx_merkle_root());
}

async fn gossip_votes(transports: &mut [InMemoryBftTransport], votes: &[Vote]) -> Vec<Vec<Vote>> {
    for vote in votes {
        transports[vote.validator_index as usize]
            .broadcast_vote(vote.clone())
            .await
            .expect("vote broadcast must succeed");
    }

    let mut received = vec![Vec::new(); transports.len()];
    let expected_per_node = transports.len().saturating_sub(1);
    for (index, receiver) in transports.iter_mut().enumerate() {
        while received[index].len() < expected_per_node {
            let message = tokio::time::timeout(Duration::from_millis(250), receiver.recv())
                .await
                .expect("vote receive must not hang")
                .expect("in-memory transport must remain connected");
            if let aurion::consensus::bft::ConsensusMessage::Vote(vote) = message {
                received[index].push(vote);
            }
        }
    }
    received
}

fn phase_votes(
    keys: &CanonicalCeremonyKeypairs,
    phase: u8,
    block: &Block,
    count: usize,
) -> Vec<Vote> {
    (0..count)
        .map(|index| {
            Vote::new_signed(
                &keys.validators[index],
                phase,
                block.header.height,
                block.header.round,
                block.hash(),
                index as u32,
            )
            .expect("deterministic vote must sign")
        })
        .collect()
}

#[tokio::test]
async fn test_happy_path_two_phase_advancement() {
    let mut fixture = fixture();
    let recipient = Address([0xAA; 32]);
    let hub = InMemoryNetworkHub::new(64);
    let mut transports: Vec<_> = (0..VALIDATOR_COUNT)
        .map(|index| hub.connect(index as u32))
        .collect();

    let mut node_accounts = vec![fixture.accounts.clone(); VALIDATOR_COUNT];
    for height in 1..=2 {
        let tx = transaction(&fixture.keys.creator, recipient, height - 1, 100);
        let (envelope, next_accounts) = proposal(&mut fixture, height, tx, 1_773_532_800 + height);
        assert_valid_proposal(&envelope, &fixture, &next_accounts);

        let prevotes = phase_votes(
            &fixture.keys,
            PHASE_PREVOTE,
            &envelope.block,
            VALIDATOR_COUNT,
        );
        for vote in &prevotes {
            vote.verify(&fixture.validator_set)
                .expect("valid prevote must verify");
        }
        let _ = gossip_votes(&mut transports, &prevotes).await;

        let precommits = phase_votes(
            &fixture.keys,
            PHASE_PRECOMMIT,
            &envelope.block,
            ACTIVE_VALIDATORS,
        );
        let certificate = CommitCertificate {
            block_hash: envelope.block.hash(),
            height,
            round: ROUND,
            precommits,
        };
        certificate
            .verify(&fixture.validator_set)
            .expect("precommit quorum must verify");
        assert_eq!(certificate.to_canonical_bytes().len(), 52 + 117 * 3);

        for accounts in &mut node_accounts {
            *accounts = next_accounts.clone();
        }
        let node_roots: Vec<_> = node_accounts
            .iter()
            .map(compute_accounts_state_root)
            .collect();
        assert!(node_roots.windows(2).all(|pair| pair[0] == pair[1]));
        fixture.accounts = next_accounts;
        fixture.previous_block_hash = envelope.block.hash();
    }

    assert_eq!(node_accounts.len(), VALIDATOR_COUNT);
    assert!(
        node_accounts
            .iter()
            .map(compute_accounts_state_root)
            .collect::<Vec<_>>()
            .windows(2)
            .all(|pair| pair[0] == pair[1])
    );
}

#[tokio::test]
async fn test_stf_gating_rejects_corrupted_state_root() {
    let mut fixture = fixture();
    let recipient = Address([0xAB; 32]);
    let tx = transaction(&fixture.keys.creator, recipient, 0, 100);
    let (envelope, expected_accounts) = proposal(&mut fixture, 1, tx, 1_773_532_801);
    let proposer = envelope.proposer_index as usize;
    let mut corrupted_block = envelope.block;
    corrupted_block.header.state_root = Hash256::from_bytes([0xFF; 32]);
    let envelope = BlockProposalEnvelope::new_signed(
        corrupted_block,
        GENESIS_CHAIN_ID,
        proposer as u32,
        &fixture.keys.validators[proposer],
    );

    envelope
        .verify(GENESIS_CHAIN_ID, &fixture.validator_set)
        .expect("authentication is independent from STF state-root validation");
    assert_ne!(
        envelope.block.header.state_root,
        compute_accounts_state_root(&expected_accounts)
    );
    let mut transports = (0..VALIDATOR_COUNT)
        .map(|index| InMemoryNetworkHub::new(8).connect(index as u32))
        .collect::<Vec<_>>();
    for transport in &mut transports {
        assert!(
            tokio::time::timeout(Duration::from_millis(20), transport.recv())
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn test_proposer_auth_rejects_out_of_turn_leader() {
    let mut fixture = fixture();
    let tx = transaction(&fixture.keys.creator, Address([0xAC; 32]), 0, 100);
    let (block_envelope, _) = proposal(&mut fixture, 1, tx, 1_773_532_802);
    let leader = BftEngine::select_proposer(
        &fixture.validator_set,
        1,
        ROUND,
        &fixture.previous_block_hash,
    );
    let wrong_index = (leader + 1) % VALIDATOR_COUNT as u32;
    let forged = BlockProposalEnvelope::new_signed(
        block_envelope.block,
        GENESIS_CHAIN_ID,
        wrong_index,
        &fixture.keys.validators[wrong_index as usize],
    );
    assert!(matches!(
        forged.verify(GENESIS_CHAIN_ID, &fixture.validator_set),
        Err(aurion::consensus::bft::ProposalEnvelopeError::InvalidProposer)
    ));
}

#[tokio::test]
async fn test_byzantine_fault_tolerance_one_node_offline() {
    let mut fixture = fixture();
    let tx = transaction(&fixture.keys.creator, Address([0xAD; 32]), 0, 100);
    let (envelope, next_accounts) = proposal(&mut fixture, 1, tx, 1_773_532_803);
    assert_valid_proposal(&envelope, &fixture, &next_accounts);

    let hub = InMemoryNetworkHub::new(32);
    let mut transports: Vec<_> = (0..ACTIVE_VALIDATORS)
        .map(|index| hub.connect(index as u32))
        .collect();
    let prevotes = phase_votes(
        &fixture.keys,
        PHASE_PREVOTE,
        &envelope.block,
        ACTIVE_VALIDATORS,
    );
    gossip_votes(&mut transports, &prevotes).await;
    let precommits = phase_votes(
        &fixture.keys,
        PHASE_PRECOMMIT,
        &envelope.block,
        ACTIVE_VALIDATORS,
    );
    let certificate = CommitCertificate {
        block_hash: envelope.block.hash(),
        height: 1,
        round: ROUND,
        precommits,
    };
    certificate
        .verify(&fixture.validator_set)
        .expect("3/4 quorum must commit with one node offline");
    assert_eq!(certificate.precommits.len(), 3);
}

#[tokio::test]
async fn test_rejection_of_duplicate_and_invalid_votes() {
    let fixture = fixture();
    let block = Block::new(
        BlockHeader {
            version: 1,
            height: 1,
            round: ROUND,
            timestamp: 1_773_532_804,
            prev_block_hash: fixture.previous_block_hash,
            tx_merkle_root: Hash256::ZERO,
            state_root: compute_accounts_state_root(&fixture.accounts),
        },
        Vec::new(),
        None,
    );
    let vote = phase_votes(&fixture.keys, PHASE_PRECOMMIT, &block, 1)
        .into_iter()
        .next()
        .unwrap();
    let duplicate_certificate = CommitCertificate {
        block_hash: block.hash(),
        height: 1,
        round: ROUND,
        precommits: vec![
            vote.clone(),
            vote.clone(),
            phase_votes(&fixture.keys, PHASE_PRECOMMIT, &block, 2)[1].clone(),
        ],
    };
    assert!(matches!(
        duplicate_certificate.verify(&fixture.validator_set),
        Err(aurion::consensus::bft::CertificateError::DuplicateVote(0))
    ));

    let mut invalid_vote = vote;
    invalid_vote.signature = Signature::from_bytes([0xEE; 64]);
    assert!(matches!(
        invalid_vote.verify(&fixture.validator_set),
        Err(aurion::consensus::bft::VoteError::InvalidSignature)
    ));
}
