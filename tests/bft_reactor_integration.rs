#![forbid(unsafe_code)]

use aurion::codec::CanonicalEncode;
use aurion::consensus::bft::{
    BftEngine, BftReactor, BftTransport, Block, BlockProposalEnvelope, CommitCertificate,
    ConsensusMessage, InMemoryNetworkHub, ValidatorSet, Vote, VoteAccumulator, PHASE_PRECOMMIT,
};
use aurion::consensus::header::BlockHeader;
use aurion::core::Hash256;
use aurion::genesis::builder::GENESIS_CHAIN_ID;
use aurion::genesis::ceremony::{CanonicalCeremonyKeypairs, CeremonyTranscript};
use aurion::state::chain::ChainLedger;
use aurion::state::monetary::calculate_block_subsidy;
use aurion::state::smt::compute_accounts_state_root;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

type SharedLedger = Arc<Mutex<ChainLedger>>;
type CommitReport = (usize, CommitCertificate);

struct Cluster {
    keys: CanonicalCeremonyKeypairs,
    validator_set: ValidatorSet,
    ledgers: Vec<SharedLedger>,
    reports: mpsc::UnboundedReceiver<CommitReport>,
    progress: mpsc::UnboundedReceiver<(usize, u64, u64)>,
    errors: mpsc::UnboundedReceiver<(usize, String)>,
    handles: Vec<tokio::task::JoinHandle<()>>,
    publisher: aurion::consensus::bft::InMemoryBftTransport,
    observer: aurion::consensus::bft::InMemoryBftTransport,
}

fn make_cluster(active_nodes: usize, round_timeout: Duration) -> Cluster {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let genesis = CeremonyTranscript::canonical_mainnet_genesis();
    let validator_set = genesis.validator_set.clone();
    let hub = InMemoryNetworkHub::new(512);
    let (report_tx, reports) = mpsc::unbounded_channel();
    let (progress_tx, progress) = mpsc::unbounded_channel();
    let (error_tx, errors) = mpsc::unbounded_channel();
    let mut ledgers = Vec::with_capacity(active_nodes);
    let mut handles = Vec::with_capacity(active_nodes);

    for index in 0..active_nodes {
        let ledger = Arc::new(Mutex::new(ChainLedger::from_genesis(genesis.clone())));
        let reactor = BftReactor::new_with_ledger(
            index as u32,
            keys.validators[index].clone(),
            validator_set.clone(),
            hub.connect(index as u32),
            ledger.clone(),
            round_timeout,
        );
        let report_tx = report_tx.clone();
        let progress_tx = progress_tx.clone();
        let error_tx = error_tx.clone();
        handles.push(tokio::spawn(async move {
            let mut reactor = reactor;
            loop {
                let result = reactor.step().await;
                let _ = progress_tx.send((
                    reactor.validator_index as usize,
                    reactor.current_height,
                    reactor.current_round,
                ));
                match result {
                    Ok(Some(certificate)) => {
                        if report_tx
                            .send((reactor.validator_index as usize, certificate))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let _ =
                            error_tx.send((reactor.validator_index as usize, error.to_string()));
                        break;
                    }
                }
            }
        }));
        ledgers.push(ledger);
    }
    assert_eq!(hub.subscriber_count(), active_nodes);

    Cluster {
        keys,
        validator_set,
        ledgers,
        reports,
        progress,
        errors,
        handles,
        publisher: hub.connect(VALIDATOR_PUBLISHER_INDEX),
        observer: hub.connect(99),
    }
}

const VALIDATOR_PUBLISHER_INDEX: u32 = 4;

fn build_proposal(
    ledger: &SharedLedger,
    validator_set: &ValidatorSet,
    keys: &CanonicalCeremonyKeypairs,
    round: u64,
) -> BlockProposalEnvelope {
    let guard = ledger.lock().expect("ledger lock must not be poisoned");
    let previous = guard.latest_block().clone();
    let height = previous.height() + 1;
    let proposer = BftEngine::select_proposer(validator_set, height, round, &previous.hash());
    let proposer_address = validator_set
        .get_validator(proposer)
        .expect("selected proposer must exist")
        .validator_id;
    let mut accounts = guard.accounts.clone();
    let subsidy = calculate_block_subsidy(height);
    if !subsidy.is_zero() {
        let account = accounts.entry(proposer_address).or_default();
        account.balance = account
            .balance
            .checked_add(subsidy)
            .expect("test subsidy must fit");
    }
    let block = Block::new(
        BlockHeader {
            version: 1,
            height,
            round,
            timestamp: previous.header.timestamp + 1,
            prev_block_hash: previous.hash(),
            tx_merkle_root: Hash256::ZERO,
            state_root: compute_accounts_state_root(&accounts),
        },
        Vec::new(),
        None,
    );
    BlockProposalEnvelope::new_signed(
        block,
        GENESIS_CHAIN_ID,
        proposer,
        &keys.validators[proposer as usize],
    )
}

async fn publish_and_wait_for_height(
    cluster: &mut Cluster,
    round: u64,
    expected_height: u64,
    active_nodes: usize,
) -> Vec<CommitCertificate> {
    let proposal = build_proposal(
        &cluster.ledgers[0],
        &cluster.validator_set,
        &cluster.keys,
        round,
    );
    cluster
        .publisher
        .broadcast_proposal(proposal)
        .await
        .expect("proposal publication must succeed");
    tokio::time::timeout(Duration::from_millis(250), async {
        loop {
            let observed = cluster
                .observer
                .recv()
                .await
                .expect("proposal channel must remain open");
            if matches!(observed, ConsensusMessage::Proposal(_)) {
                break;
            }
        }
    })
    .await
    .expect("observer must receive proposal");

    let mut certificates = Vec::with_capacity(active_nodes);
    let wait_result = tokio::time::timeout(Duration::from_secs(5), async {
        while certificates.len() < active_nodes {
            let (_, certificate) = tokio::select! {
                report = cluster.reports.recv() => report.expect("reactor task must remain alive"),
                error = cluster.errors.recv() => {
                    panic!("reactor task failed: {:?}", error);
                }
            };
            assert_eq!(certificate.height, expected_height);
            certificates.push(certificate);
        }
    })
    .await;
    if wait_result.is_err() {
        let mut progress = Vec::new();
        while let Ok(value) = cluster.progress.try_recv() {
            progress.push(value);
        }
        let mut errors = Vec::new();
        while let Ok(value) = cluster.errors.try_recv() {
            errors.push(value);
        }
        panic!(
            "reactors must commit before integration timeout; progress={progress:?}, errors={errors:?}"
        );
    }
    certificates
}

#[tokio::test]
async fn test_four_reactors_progress_three_blocks_without_state_split() {
    let mut cluster = make_cluster(4, Duration::from_secs(1));
    tokio::time::sleep(Duration::from_millis(20)).await;
    for height in 1..=3 {
        let certificates = publish_and_wait_for_height(&mut cluster, 0, height, 4).await;
        for certificate in certificates {
            certificate
                .verify(&cluster.validator_set)
                .expect("every committed certificate must verify");
        }
    }

    let ledgers = cluster
        .ledgers
        .iter()
        .map(|ledger| ledger.lock().expect("ledger lock").latest_block().clone())
        .collect::<Vec<_>>();
    assert!(ledgers.iter().all(|block| block.height() >= 3));
    assert!(ledgers
        .windows(2)
        .all(|pair| pair[0].hash() == pair[1].hash()));
    assert!(ledgers
        .windows(2)
        .all(|pair| pair[0].header.state_root == pair[1].header.state_root));
    assert_eq!(
        ledgers[0]
            .commit_certificate
            .as_ref()
            .unwrap()
            .to_canonical_bytes()
            .len(),
        52 + 117 * 3
    );

    for handle in cluster.handles {
        handle.abort();
    }
}

#[tokio::test]
async fn test_three_reactors_progress_with_one_silent_validator() {
    let mut cluster = make_cluster(3, Duration::from_secs(1));
    tokio::time::sleep(Duration::from_millis(20)).await;
    let certificates = publish_and_wait_for_height(&mut cluster, 0, 1, 3).await;
    assert_eq!(certificates.len(), 3);
    for certificate in certificates {
        certificate
            .verify(&cluster.validator_set)
            .expect("3/4 certificate must verify");
        assert_eq!(certificate.precommits.len(), 3);
    }
    assert!(cluster.ledgers.iter().all(|ledger| ledger
        .lock()
        .expect("ledger lock")
        .latest_height()
        == 1));

    for handle in cluster.handles {
        handle.abort();
    }
}

#[tokio::test]
async fn test_silent_round_zero_leader_advances_to_round_one() {
    let mut cluster = make_cluster(4, Duration::from_millis(25));
    tokio::time::sleep(Duration::from_millis(20)).await;
    let mut observed_round_one = [false; 4];
    tokio::time::timeout(Duration::from_secs(2), async {
        while !observed_round_one.iter().all(|observed| *observed) {
            let (index, _height, round) = cluster
                .progress
                .recv()
                .await
                .expect("reactor task must remain alive");
            if round >= 1 {
                observed_round_one[index] = true;
            }
        }
    })
    .await
    .expect("all reactors must perform deterministic view change");

    let certificates = publish_and_wait_for_height(&mut cluster, 1, 1, 4).await;
    assert!(certificates
        .iter()
        .all(|certificate| certificate.round == 1));
    assert!(cluster.ledgers.iter().all(|ledger| ledger
        .lock()
        .expect("ledger lock")
        .latest_height()
        == 1));

    for handle in cluster.handles {
        handle.abort();
    }
}

#[tokio::test]
async fn test_vote_accumulator_rejects_replay_and_equivocation() {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let mut accumulator = VoteAccumulator::new();
    let block_hash_a = Hash256::from_bytes([0x11; 32]);
    let block_hash_b = Hash256::from_bytes([0x22; 32]);

    let vote_a = Vote::new_signed(&keys.validators[0], PHASE_PRECOMMIT, 1, 0, block_hash_a, 0)
        .expect("valid precommit must sign");
    let vote_a_dup = vote_a.clone();
    let vote_b = Vote::new_signed(&keys.validators[0], PHASE_PRECOMMIT, 1, 0, block_hash_b, 0)
        .expect("valid alternate precommit must sign");

    assert_eq!(accumulator.add_vote_checked(vote_a.clone()).unwrap(), 1);
    assert_eq!(accumulator.add_vote_checked(vote_a_dup).unwrap(), 1);
    assert!(accumulator.add_vote_checked(vote_b).is_err());
}

#[tokio::test]
async fn test_silent_round_zero_and_one_leaders_advance_to_round_two() {
    let mut cluster = make_cluster(4, Duration::from_millis(100));
    tokio::time::sleep(Duration::from_millis(20)).await;

    let proposal = build_proposal(
        &cluster.ledgers[0],
        &cluster.validator_set,
        &cluster.keys,
        2,
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let (_index, _height, round) = cluster
                .progress
                .recv()
                .await
                .expect("reactor task must remain alive");
            if round >= 2 {
                break;
            }
        }
    })
    .await
    .expect("at least one reactor must reach round two");

    cluster
        .publisher
        .broadcast_proposal(proposal)
        .await
        .expect("round-two proposal publication must succeed");

    let mut certificates = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), async {
        while certificates.len() < 4 {
            let (_, certificate) = cluster
                .reports
                .recv()
                .await
                .expect("reactor task must remain alive");
            certificates.push(certificate);
        }
    })
    .await
    .expect("all reactors must commit the round-two proposal");

    assert!(certificates
        .iter()
        .all(|certificate| certificate.round == 2));
    assert!(cluster.ledgers.iter().all(|ledger| ledger
        .lock()
        .expect("ledger lock")
        .latest_height()
        == 1));

    for handle in cluster.handles {
        handle.abort();
    }
}
