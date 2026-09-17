#![forbid(unsafe_code)]

//! Test Integrasi Peluncuran Produksi Aurion Mainnet (PRD-016).
//! Menguji transisi konsensus BFT Slot 0 -> Blok 1, siklus transaksi perdana,
//! konservasi moneter, persistensi redb, dan CLI control plane.

use aurion::cli::{dispatch, CliCommand, OutputFormat};
use aurion::consensus::bft::certificate::CommitCertificate;
use aurion::consensus::bft::vote::{Vote, PHASE_PRECOMMIT};
use aurion::core::{Address, Quantum, Signature};
use aurion::genesis::builder::{GENESIS_CHAIN_ID, GENESIS_TIMESTAMP};
use aurion::genesis::ceremony::{
    CanonicalCeremonyKeypairs, CeremonyTranscript, CEREMONY_QUORUM_THRESHOLD,
    CEREMONY_TOTAL_VOTING_POWER,
};
use aurion::runtime::config::NodeConfig;
use aurion::runtime::AurionNode;
use aurion::storage::RedbStorageEngine;
use aurion::transaction::types::{Transaction, TxType};
use std::sync::Arc;
use tempfile::tempdir;

const EXPECTED_GENESIS_BLOCK_HASH: &str =
    "d82f72ac1be185911bd803987660e624c0ed1c12d4a189b147de9c5b7f5635f9";
const EXPECTED_STATE_ROOT: &str =
    "61e647706990a010ba95f781d506620cf69b2dc57a7b1b53ca96f0f1d07bb850";
const EXPECTED_CEREMONY_HASH: &str =
    "f88d06b37766746bcb9c9985c35ebf6448c441a578a576ee51bd7efbc64e3380";

#[test]
fn test_mainnet_genesis_initialization_from_sealed_ceremony() {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let transcript = CeremonyTranscript::build_and_seal(&keys)
        .expect("Canonical ceremony build must succeed");

    assert_eq!(transcript.ceremony_hash, EXPECTED_CEREMONY_HASH);
    assert_eq!(transcript.genesis_block_hash, EXPECTED_GENESIS_BLOCK_HASH);
    assert_eq!(transcript.state_root, EXPECTED_STATE_ROOT);
    assert_eq!(transcript.chain_id, GENESIS_CHAIN_ID);
    assert_eq!(transcript.timestamp, GENESIS_TIMESTAMP);
    assert_eq!(transcript.hard_cap_aur, 66_000_000);
    assert_eq!(transcript.initial_supply_aur, 23_100_000);
    assert_eq!(transcript.creator_allocation_aur, 19_800_000);
    assert_eq!(transcript.developer_allocation_aur, 3_300_000);

    let genesis = transcript
        .build_genesis_initialization()
        .expect("Genesis initialization reconstruction must succeed");

    assert_eq!(
        genesis.header.compute_block_hash().to_hex(),
        EXPECTED_GENESIS_BLOCK_HASH
    );
    assert_eq!(genesis.header.state_root.to_hex(), EXPECTED_STATE_ROOT);
    assert_eq!(genesis.validator_set.validators.len(), 4);
    assert_eq!(
        genesis.validator_set.total_voting_power(),
        CEREMONY_TOTAL_VOTING_POWER
    );

    // Creator: 19.8M AUR = 1,980,000,000,000,000 Quanta
    let creator_addr = keys.creator.derive_address();
    let creator_acc = genesis.accounts.get(&creator_addr).expect("Creator account exists");
    assert_eq!(creator_acc.balance.as_u128(), 1_980_000_000_000_000);

    // Developer: 3.3M AUR = 330,000,000,000,000 Quanta
    let dev_addr = keys.developer.derive_address();
    let dev_acc = genesis.accounts.get(&dev_addr).expect("Developer account exists");
    assert_eq!(dev_acc.balance.as_u128(), 330_000_000_000_000);

    // Total initial supply: exactly 23.1M AUR
    let total_initial = creator_acc.balance.checked_add(dev_acc.balance).unwrap();
    assert_eq!(total_initial.as_u128(), 2_310_000_000_000_000);
}

#[test]
fn test_mainnet_slot_0_to_block_1_bft_transition() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("mainnet.redb");
    let store = Arc::new(RedbStorageEngine::open_or_create(&db_path).unwrap());

    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let genesis = CeremonyTranscript::canonical_mainnet_genesis();
    let val0_key = keys.validators[0].clone();
    let val0_addr = val0_key.derive_address();

    let config = NodeConfig {
        chain_id: GENESIS_CHAIN_ID as u64,
        ..NodeConfig::new_validator(Vec::new())
    };

    let node = AurionNode::new_with_store(
        config,
        genesis,
        Some(val0_key.clone()),
        Some(0),
        store.clone(),
    );

    // Initial state: Height 0 (Genesis)
    assert_eq!(node.ledger.lock().unwrap().latest_height(), 0);
    assert_eq!(
        node.ledger.lock().unwrap().latest_block().hash().to_hex(),
        EXPECTED_GENESIS_BLOCK_HASH
    );

    // Rakit proposal Blok 1
    let timestamp_1 = GENESIS_TIMESTAMP + 1;
    let (block_1, block_hash_1) = {
        let ledger = node.ledger.lock().unwrap();
        let mempool = node.mempool.lock().unwrap();
        let bft = node.bft_engine.lock().unwrap();
        let proposal = bft.assemble_block_proposal(
            &ledger,
            &mempool,
            0,
            timestamp_1,
            &val0_addr,
            1024 * 1024,
        );
        let hash = proposal.hash();
        (proposal, hash)
    };

    assert_eq!(block_1.height(), 1);
    assert_eq!(block_1.header.prev_block_hash.to_hex(), EXPECTED_GENESIS_BLOCK_HASH);

    // Bentuk CommitCertificate valid dengan suara 3 dari 4 validator (750,000 / 1,000,000 >= 666,667 quorum)
    let mut precommits = Vec::new();
    let mut total_weight = 0;
    for (i, val_key) in keys.validators[0..3].iter().enumerate() {
        let vote = Vote::new_signed(val_key, PHASE_PRECOMMIT, 1, 0, block_hash_1, i as u32)
            .expect("Vote signing must succeed");
        precommits.push(vote);
        total_weight += 250_000;
    }

    assert!(total_weight >= CEREMONY_QUORUM_THRESHOLD);

    let cert = CommitCertificate {
        block_hash: block_hash_1,
        height: 1,
        round: 0,
        precommits,
    };

    // Verifikasi kuorum sertifikat terhadap validator set
    cert.verify(&node.ledger.lock().unwrap().validator_set)
        .expect("Certificate verification must pass");

    // Komit Blok 1 ke dalam simpul
    let committed_block = node
        .produce_and_commit_block(cert, &val0_addr, timestamp_1)
        .expect("Commit block 1 must succeed");

    assert_eq!(committed_block.height(), 1);
    assert_eq!(node.ledger.lock().unwrap().latest_height(), 1);
    assert_eq!(node.ledger.lock().unwrap().finalized_height(), 1);

    // Miner/Proposer validator 0 mendapatkan subsidi blok perdana
    let val0_balance = node
        .ledger
        .lock()
        .unwrap()
        .get_account(&val0_addr)
        .map(|a| a.balance)
        .unwrap_or(Quantum::ZERO);

    assert!(val0_balance > Quantum::ZERO, "Miner must receive block subsidy");
}

#[test]
fn test_mainnet_first_transaction_lifecycle_and_monetary_conservation() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("mainnet_tx.redb");
    let store = Arc::new(RedbStorageEngine::open_or_create(&db_path).unwrap());

    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let genesis = CeremonyTranscript::canonical_mainnet_genesis();
    let val0_key = keys.validators[0].clone();
    let val0_addr = val0_key.derive_address();

    let config = NodeConfig {
        chain_id: GENESIS_CHAIN_ID as u64,
        ..NodeConfig::new_validator(Vec::new())
    };

    let node = AurionNode::new_with_store(
        config,
        genesis,
        Some(val0_key.clone()),
        Some(0),
        store.clone(),
    );

    let creator_addr = keys.creator.derive_address();
    let initial_creator_balance = node
        .ledger
        .lock()
        .unwrap()
        .get_account(&creator_addr)
        .unwrap()
        .balance;

    // Penerima transaksi perdana di mainnet
    let recipient_addr = Address::from_bytes([0x77; 32]);
    let transfer_amount = Quantum::new(1_000 * 100_000_000); // 1,000 AUR
    let fee = Quantum::new(100_000_000); // 1 AUR

    // Buat transaksi transfer perdana
    let mut tx = Transaction {
        version: 1,
        chain_id: GENESIS_CHAIN_ID,
        tx_type: TxType::Transfer,
        flags: 0,
        sender: creator_addr,
        recipient: recipient_addr,
        nonce: 0,
        amount: transfer_amount,
        fee,
        valid_until: GENESIS_TIMESTAMP + 3600,
        payload: Vec::new(),
        signature: Signature::from_bytes([0u8; 64]),
    };

    // Tanda tangani dengan kunci Creator
    let preimage = tx.signing_preimage();
    tx.signature = keys.creator.sign(&preimage);

    // Masukkan ke dalam mempool melalui submit_transaction
    let creator_acc = node
        .ledger
        .lock()
        .unwrap()
        .get_account(&creator_addr)
        .unwrap()
        .clone();
    node.mempool
        .lock()
        .unwrap()
        .submit_transaction(
            tx,
            &keys.creator.public_key_bytes(),
            GENESIS_TIMESTAMP + 1,
            &creator_acc,
        )
        .expect("Mempool submission must succeed");

    assert_eq!(node.mempool.lock().unwrap().entries.len(), 1);

    // Komit Blok 1 dengan transaksi tersebut
    let timestamp_1 = GENESIS_TIMESTAMP + 1;
    let (_block_1, block_hash_1) = {
        let ledger = node.ledger.lock().unwrap();
        let mempool = node.mempool.lock().unwrap();
        let bft = node.bft_engine.lock().unwrap();
        let proposal = bft.assemble_block_proposal(
            &ledger,
            &mempool,
            0,
            timestamp_1,
            &val0_addr,
            1024 * 1024,
        );
        let hash = proposal.hash();
        (proposal, hash)
    };

    let mut precommits = Vec::new();
    for (i, val_key) in keys.validators[0..3].iter().enumerate() {
        let vote = Vote::new_signed(val_key, PHASE_PRECOMMIT, 1, 0, block_hash_1, i as u32)
            .expect("Vote signing must succeed");
        precommits.push(vote);
    }

    let cert = CommitCertificate {
        block_hash: block_hash_1,
        height: 1,
        round: 0,
        precommits,
    };

    let committed = node
        .produce_and_commit_block(cert, &val0_addr, timestamp_1)
        .expect("Commit block 1 with tx must succeed");

    assert_eq!(committed.transactions.len(), 1);
    assert_eq!(node.mempool.lock().unwrap().entries.len(), 0);

    // Verifikasi saldo penerima
    let recipient_balance = node
        .ledger
        .lock()
        .unwrap()
        .get_account(&recipient_addr)
        .unwrap()
        .balance;
    assert_eq!(recipient_balance, transfer_amount);

    // Verifikasi saldo Creator berkurang transfer_amount + fee
    let final_creator_balance = node
        .ledger
        .lock()
        .unwrap()
        .get_account(&creator_addr)
        .unwrap()
        .balance;
    let expected_creator = initial_creator_balance
        .checked_sub(transfer_amount)
        .unwrap()
        .checked_sub(fee)
        .unwrap();
    assert_eq!(final_creator_balance, expected_creator);

    // Verifikasi fee split: 20% burn, 80% miner share
    let burned_amount = node.ledger.lock().unwrap().monetary.total_burned;
    let expected_burned = Quantum::new(20_000_000); // 0.2 AUR = 20,000,000 Quanta
    assert_eq!(burned_amount, expected_burned);

    // Verifikasi SMT State Root berubah secara deterministik
    let state_root_1 = node.ledger.lock().unwrap().compute_current_state_root();
    assert_ne!(state_root_1.to_hex(), EXPECTED_STATE_ROOT);
}

#[test]
fn test_mainnet_redb_persistence_crash_recovery() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("recovery_mainnet.redb");

    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let val0_key = keys.validators[0].clone();
    let val0_addr = val0_key.derive_address();

    let pre_crash_hash;
    let pre_crash_height;
    let pre_crash_state_root;

    // Phase 1: Jalankan simpul dan komit Blok 1
    {
        let store = Arc::new(RedbStorageEngine::open_or_create(&db_path).unwrap());
        let genesis = CeremonyTranscript::canonical_mainnet_genesis();
        let config = NodeConfig {
            chain_id: GENESIS_CHAIN_ID as u64,
            ..NodeConfig::new_validator(Vec::new())
        };

        let node = AurionNode::new_with_store(
            config,
            genesis,
            Some(val0_key.clone()),
            Some(0),
            store,
        );

        let timestamp_1 = GENESIS_TIMESTAMP + 1;
        let (_block_1, block_hash_1) = {
            let ledger = node.ledger.lock().unwrap();
            let mempool = node.mempool.lock().unwrap();
            let bft = node.bft_engine.lock().unwrap();
            let proposal = bft.assemble_block_proposal(
                &ledger,
                &mempool,
                0,
                timestamp_1,
                &val0_addr,
                1024 * 1024,
            );
            let hash = proposal.hash();
            (proposal, hash)
        };

        let mut precommits = Vec::new();
        for (i, val_key) in keys.validators[0..3].iter().enumerate() {
            let vote = Vote::new_signed(val_key, PHASE_PRECOMMIT, 1, 0, block_hash_1, i as u32)
                .expect("Vote signing must succeed");
            precommits.push(vote);
        }

        let cert = CommitCertificate {
            block_hash: block_hash_1,
            height: 1,
            round: 0,
            precommits,
        };

        let committed = node
            .produce_and_commit_block(cert, &val0_addr, timestamp_1)
            .expect("Commit block 1 must succeed");

        pre_crash_hash = committed.hash();
        pre_crash_height = committed.height();
        pre_crash_state_root = committed.header.state_root;
        // Node dan RedbStorageEngine di-drop di sini (simulasi crash/shutdown)
    }

    // Phase 2: Pemulihan dari crash menggunakan file redb yang sama
    {
        let store = Arc::new(RedbStorageEngine::open_or_create(&db_path).unwrap());
        let genesis = CeremonyTranscript::canonical_mainnet_genesis();
        let config = NodeConfig {
            chain_id: GENESIS_CHAIN_ID as u64,
            ..NodeConfig::new_validator(Vec::new())
        };

        let recovered_node = AurionNode::new_with_store(
            config,
            genesis,
            Some(val0_key),
            Some(0),
            store,
        );

        let recovered_ledger = recovered_node.ledger.lock().unwrap();
        assert_eq!(recovered_ledger.latest_height(), pre_crash_height);
        assert_eq!(
            recovered_ledger.latest_block().hash(),
            pre_crash_hash
        );
        assert_eq!(
            recovered_ledger.compute_current_state_root(),
            pre_crash_state_root
        );

        let val0_balance = recovered_ledger
            .get_account(&val0_addr)
            .map(|a| a.balance)
            .unwrap_or(Quantum::ZERO);
        assert!(val0_balance > Quantum::ZERO);
    }
}

#[tokio::test]
async fn test_mainnet_cli_dispatchers() {
    // 1. aurion network status
    let cmd = CliCommand::Network(vec!["status".to_string()]);
    assert!(dispatch(cmd, OutputFormat::Json).await.is_ok());

    // 2. aurion network peers
    let cmd = CliCommand::Network(vec!["peers".to_string()]);
    assert!(dispatch(cmd, OutputFormat::Json).await.is_ok());

    // 3. aurion node status --dry-run
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("cli_node.redb").to_string_lossy().to_string();
    let cmd = CliCommand::Node(vec![
        "status".to_string(),
        "--data-dir".to_string(),
        db_path.clone(),
        "--dry-run".to_string(),
    ]);
    assert!(dispatch(cmd, OutputFormat::Json).await.is_ok());

    // 4. aurion validator status --dry-run
    let cmd = CliCommand::Validator(vec![
        "status".to_string(),
        "--data-dir".to_string(),
        db_path,
        "--index".to_string(),
        "0".to_string(),
        "--dry-run".to_string(),
    ]);
    assert!(dispatch(cmd, OutputFormat::Json).await.is_ok());
}
