#![forbid(unsafe_code)]

use aurion::codec::CanonicalEncode;
use aurion::consensus::bft::{
    BftEngine, BftReactor, BftTransport, Block, BlockProposalEnvelope, CommitCertificate,
    InMemoryNetworkHub, ValidatorSet,
};
use aurion::consensus::header::BlockHeader;
use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::crypto::Keypair;
use aurion::genesis::builder::GENESIS_CHAIN_ID;
use aurion::genesis::ceremony::{CanonicalCeremonyKeypairs, CeremonyTranscript};
use aurion::state::account::Account;
use aurion::state::chain::ChainLedger;
use aurion::state::monetary::calculate_block_subsidy;
use aurion::state::smt::compute_accounts_state_root;
use aurion::state::stf::apply_transaction;
use aurion::storage::{RedbStorageEngine, StateStore};
use aurion::transaction::types::{Transaction, TxType};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::NamedTempFile;

type Ledger = Arc<Mutex<ChainLedger>>;

struct Cluster {
    keys: CanonicalCeremonyKeypairs,
    validators: ValidatorSet,
    ledgers: Vec<Ledger>,
    stores: Vec<Arc<RedbStorageEngine>>,
    paths: Vec<NamedTempFile>,
    reports: tokio::sync::mpsc::UnboundedReceiver<CommitCertificate>,
    handles: Vec<tokio::task::JoinHandle<()>>,
    publisher: aurion::consensus::bft::InMemoryBftTransport,
}

fn make_cluster(active: usize) -> Cluster {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let genesis = CeremonyTranscript::canonical_mainnet_genesis();
    let validators = genesis.validator_set.clone();
    let hub = InMemoryNetworkHub::new(1024);
    let (report_tx, reports) = tokio::sync::mpsc::unbounded_channel();
    let mut paths = Vec::new();
    let mut stores = Vec::new();
    let mut ledgers = Vec::new();
    let mut handles = Vec::new();

    for index in 0..4 {
        let path = NamedTempFile::new().unwrap();
        let store = Arc::new(RedbStorageEngine::open_or_create(path.path()).unwrap());
        let ledger = Arc::new(Mutex::new(
            ChainLedger::from_genesis_with_store(genesis.clone(), store.clone()).unwrap(),
        ));
        if index < active {
            let reactor = BftReactor::new_with_ledger(
                index as u32,
                keys.validators[index].clone(),
                validators.clone(),
                hub.connect(index as u32),
                ledger.clone(),
                Duration::from_secs(1),
            );
            let tx = report_tx.clone();
            handles.push(tokio::spawn(async move {
                let mut reactor = reactor;
                loop {
                    match reactor.step().await {
                        Ok(Some(cert)) => {
                            if tx.send(cert).is_err() {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(_) => break,
                    }
                }
            }));
        }
        paths.push(path);
        stores.push(store);
        ledgers.push(ledger);
    }

    Cluster {
        keys,
        validators,
        ledgers,
        stores,
        paths,
        reports,
        handles,
        publisher: hub.connect(99),
    }
}

fn transfer(key: &Keypair, recipient: Address, nonce: u64) -> Transaction {
    let mut tx = Transaction {
        version: 1,
        chain_id: GENESIS_CHAIN_ID,
        tx_type: TxType::Transfer,
        flags: 0,
        sender: key.derive_address(),
        recipient,
        nonce,
        amount: Quantum::new(1),
        fee: Quantum::ZERO,
        valid_until: u64::MAX,
        payload: Vec::new(),
        signature: Signature::from_bytes([0; 64]),
    };
    tx.signature = key.sign(&tx.signing_preimage());
    tx
}

fn proposal(cluster: &Cluster, height: u64) -> BlockProposalEnvelope {
    let ledger = cluster.ledgers[0].lock().unwrap();
    let previous = ledger.latest_block().clone();
    let proposer = BftEngine::select_proposer(&cluster.validators, height, 0, &previous.hash());
    let miner = cluster
        .validators
        .get_validator(proposer)
        .unwrap()
        .validator_id;
    let recipient = Address([0xA5; 32]);
    let tx = transfer(&cluster.keys.creator, recipient, height - 1);
    let mut accounts = ledger.accounts.clone();
    let mut monetary = ledger.monetary.clone();
    apply_transaction(&mut accounts, &mut monetary, &miner, &tx).unwrap();
    let subsidy = calculate_block_subsidy(height);
    if !subsidy.is_zero() {
        let account = accounts.entry(miner).or_insert_with(Account::default);
        account.balance = account.balance.checked_add(subsidy).unwrap();
    }
    let block = Block::new(
        BlockHeader {
            version: 1,
            height,
            round: 0,
            timestamp: previous.header.timestamp + 1,
            prev_block_hash: previous.hash(),
            tx_merkle_root: Block::calculate_tx_merkle_root(std::slice::from_ref(&tx)),
            state_root: compute_accounts_state_root(&accounts),
        },
        vec![tx],
        None,
    );
    BlockProposalEnvelope::new_signed(
        block,
        GENESIS_CHAIN_ID,
        proposer,
        &cluster.keys.validators[proposer as usize],
    )
}

async fn publish_and_wait(cluster: &mut Cluster, height: u64, expected_reports: usize) {
    cluster
        .publisher
        .broadcast_proposal(proposal(cluster, height))
        .await
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(8), async {
        let mut count = 0;
        while count < expected_reports {
            let cert = cluster.reports.recv().await.unwrap();
            cert.verify(&cluster.validators).unwrap();
            assert_eq!(cert.to_canonical_bytes().len(), 403);
            count += 1;
        }
    })
    .await;
    assert!(result.is_ok(), "cluster must reach quorum without stalling");
}

#[tokio::test]
async fn test_consensus_gate_multiblock_redb_and_recovery() {
    let mut cluster = make_cluster(4);
    tokio::time::sleep(Duration::from_millis(30)).await;
    for height in 1..=3 {
        publish_and_wait(&mut cluster, height, 4).await;
        let blocks = cluster
            .ledgers
            .iter()
            .map(|ledger| ledger.lock().unwrap().latest_block().clone())
            .collect::<Vec<_>>();
        assert!(blocks.iter().all(|block| block.height() == height));
        assert!(blocks
            .windows(2)
            .all(|pair| pair[0].hash() == pair[1].hash()));
        assert!(blocks
            .windows(2)
            .all(|pair| pair[0].header.state_root == pair[1].header.state_root));
        for store in &cluster.stores {
            assert_eq!(
                store
                    .get_certificate(height)
                    .unwrap()
                    .unwrap()
                    .to_canonical_bytes()
                    .len(),
                403
            );
        }
    }
    let handles = cluster.handles.drain(..).collect::<Vec<_>>();
    for handle in &handles {
        handle.abort();
    }
    for handle in handles {
        let _ = handle.await;
    }
    cluster.ledgers.clear();
    cluster.stores.clear();
    let genesis = CeremonyTranscript::canonical_mainnet_genesis();
    for path in &cluster.paths {
        let store = Arc::new(RedbStorageEngine::open_or_create(path.path()).unwrap());
        let recovered =
            ChainLedger::from_genesis_with_store(genesis.clone(), store.clone()).unwrap();
        assert_eq!(recovered.latest_height(), 3);
        let block = recovered.latest_block();
        assert_eq!(
            store.get_metadata("latest_block_hash").unwrap().unwrap(),
            block.hash().as_bytes()
        );
        assert_eq!(
            store.get_metadata("latest_state_root").unwrap().unwrap(),
            block.header.state_root.as_bytes()
        );
        store
            .get_certificate(3)
            .unwrap()
            .unwrap()
            .verify(&recovered.validator_set)
            .unwrap();
    }
}

#[tokio::test]
async fn test_consensus_gate_one_validator_offline() {
    let mut cluster = make_cluster(3);
    tokio::time::sleep(Duration::from_millis(30)).await;
    publish_and_wait(&mut cluster, 1, 3).await;
    assert!(cluster.ledgers[..3]
        .iter()
        .all(|ledger| ledger.lock().unwrap().latest_height() == 1));
    assert_eq!(cluster.stores[3].get_latest_height().unwrap(), Some(0));
    for handle in cluster.handles {
        handle.abort();
    }
}

#[tokio::test]
async fn test_consensus_gate_rejection_invariants() {
    let cluster = make_cluster(4);
    let valid = proposal(&cluster, 1);
    let mut forged = valid.clone();
    forged.proposer_index = (valid.proposer_index + 1) % 4;
    assert!(forged
        .verify(GENESIS_CHAIN_ID, &cluster.validators)
        .is_err());
    let mut corrupted = valid.block.clone();
    corrupted.header.state_root = Hash256::ZERO;
    let corrupted = BlockProposalEnvelope::new_signed(
        corrupted,
        GENESIS_CHAIN_ID,
        valid.proposer_index,
        &cluster.keys.validators[valid.proposer_index as usize],
    );
    assert!(corrupted.block.header.state_root != valid.block.header.state_root);
    assert!(cluster.ledgers[0]
        .lock()
        .unwrap()
        .validate_block_proposal(
            &corrupted.block,
            &cluster
                .validators
                .get_validator(valid.proposer_index)
                .unwrap()
                .validator_id,
        )
        .is_err());
}
