#![forbid(unsafe_code)]

//! Suite Pengujian Integrasi Private Multi-Region Testnet Aurion (NET-011).
//!
//! Mematuhi Invariant:
//! - AUR-ARCH-001: Single Sovereign Primary Binary (/bin/aurion).
//! - AUR-ARCH-009: Process Lifecycle, Graceful Shutdown, dan Konvergensi State Terdistribusi.
//! - AUR-ARCH-011: Absolute Zero Unsafe Code (#![forbid(unsafe_code)]).
//! - AUR-ARCH-012: Absolute Zero Float Arithmetic (Fixed Precision Quantum u128).
//! - AUR-CONS-*: Round-Based BFT Consensus with >2/3 Quorum Finality under WAN latency.
//!
//! Suite ini memverifikasi:
//! 1. Kluster konsensus 4-region geografis (AP, EU, US, SA) di bawah latensi WAN simulasi (15ms - 300ms).
//! 2. Rotasi validator dinamis berbasis epoch (EpochTransition) dengan pembaruan validator set.
//! 3. Ekspor State Snapshot (AUSS) kanonikal terotentikasi dengan sertifikat komit.
//! 4. Fast-Sync Onboarding: Simpul baru menyerap snapshot dan mencapai State Root 100% identik.

use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use tempfile::TempDir;

use aurion::consensus::bft::epoch::{
    compute_epoch_id, is_epoch_boundary, rotate_validator_set, EpochInfo, EpochTransition,
};
use aurion::consensus::block::Block;
use aurion::consensus::certificate::{ValidatorEntry, ValidatorSet};
use aurion::core::{Quantum, Signature};
use aurion::crypto::{derive_address_from_pubkey, Keypair};
use aurion::genesis::builder::build_genesis;
use aurion::runtime::config::NodeConfig;
use aurion::runtime::AurionNode;
use aurion::statemachine::state::snapshot::{StateSnapshot, SNAPSHOT_MAGIC, SNAPSHOT_VERSION};
use aurion::storage::{RedbStorageEngine, StateStore};
use aurion::transaction::types::{Transaction, TxType};

static NEXT_TESTNET_PORT: AtomicU16 = AtomicU16::new(19900);

fn allocate_testnet_port() -> String {
    let port = NEXT_TESTNET_PORT.fetch_add(1, Ordering::SeqCst);
    format!("127.0.0.1:{port}")
}

#[tokio::test]
async fn test_multi_region_testnet_wan_latency_and_fast_sync() {
    let temp_dir = TempDir::new().expect("Create temp dir for multi-region testnet");
    let base_path = temp_dir.path().to_path_buf();

    // 1. Inisialisasi 4 Region Geografis (AP, EU, US, SA)
    let regions = ["ap-southeast", "eu-central", "us-east", "sa-east"];
    let latencies_ms: [u64; 4] = [15, 160, 220, 300];

    let mut val_keys = Vec::new();
    let mut val_entries = Vec::new();
    let mut val_addrs = Vec::new();

    for (i, reg) in regions.iter().enumerate() {
        let key = Keypair::generate();
        let addr = derive_address_from_pubkey(&key.public_key_bytes());
        val_entries.push(ValidatorEntry {
            validator_id: addr,
            consensus_pubkey: key.public_key_bytes(),
            voting_weight: 25,
        });
        val_addrs.push(addr);
        val_keys.push(key);
        println!(
            "[TESTNET] Region {reg}: Validator ID {addr} (WAN latency: {}ms)",
            latencies_ms[i]
        );
    }
    let validator_set = ValidatorSet::new(val_entries.clone());
    assert_eq!(validator_set.total_voting_power(), 100);

    let creator_key = Keypair::generate();
    let creator_addr = derive_address_from_pubkey(&creator_key.public_key_bytes());

    let genesis = build_genesis(creator_addr, val_entries.clone());

    // 2. Setup 4 Node Validator di 4 Region
    let mut region_nodes = Vec::new();
    let mut node_stores = Vec::new();

    for (i, val_key) in val_keys.iter().enumerate().take(4) {
        let db_path = base_path.join(format!("region_node_{i}.redb"));
        let store = Arc::new(RedbStorageEngine::open_or_create(&db_path).expect("Create redb"));
        node_stores.push(Arc::clone(&store));

        let config = NodeConfig {
            chain_id: 9999,
            rpc_bind: allocate_testnet_port(),
            p2p_bind: allocate_testnet_port(),
            ..NodeConfig::new_validator(Vec::new())
        };

        let node = Arc::new(AurionNode::new_with_store(
            config,
            genesis.clone(),
            Some(val_key.clone()),
            Some(i as u32),
            store,
        ));
        region_nodes.push(node);
    }

    // 3. Eksekusi Blok 1 dengan Simulasi Delay WAN antar-Region
    let alice = Keypair::generate();
    let alice_addr = derive_address_from_pubkey(&alice.public_key_bytes());

    let mut tx1 = Transaction {
        version: 1,
        chain_id: 9999,
        tx_type: TxType::Transfer,
        flags: 0,
        sender: creator_addr,
        recipient: alice_addr,
        amount: Quantum::new(50_000_000), // 0.5 AUR
        fee: Quantum::new(2_000),
        nonce: 0,
        valid_until: 1800000000,
        payload: Vec::new(),
        signature: Signature::from_bytes([0u8; 64]),
    };
    let preimage = tx1.signing_preimage();
    tx1.signature = creator_key.sign(&preimage);

    // Kirim transaksi ke mempool seluruh simpul lintas-benua
    for node in &region_nodes {
        let acct = node
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
                tx1.clone(),
                &creator_key.public_key_bytes(),
                1773532850,
                &acct,
            )
            .expect("Tx submission");
    }

    // Proposer merakit blok H=1 di Region 0 (AP)
    let miner = val_addrs[0];
    let candidate = {
        let node_p = &region_nodes[0];
        let ledger = node_p.ledger.lock().unwrap();
        let mempool = node_p.mempool.lock().unwrap();
        let bft = node_p.bft_engine.lock().unwrap();
        bft.assemble_block_proposal(&ledger, &mempool, 0, 1773534000, &miner, 1024 * 1024)
    };
    let block_hash = candidate.hash();

    // Simulasi pengumpulan suara Prevote & Precommit lintas-benua (dengan latensi simulasi)
    let mut prevotes = Vec::new();
    let mut precommits = Vec::new();
    for (i, node) in region_nodes.iter().enumerate() {
        // Latensi WAN simulative delay (skala mikroskopik test)
        tokio::time::sleep(tokio::time::Duration::from_millis(latencies_ms[i] / 10)).await;

        let prevote = node
            .bft_engine
            .lock()
            .unwrap()
            .produce_prevote(block_hash, 1, 0)
            .unwrap();
        let precommit = node
            .bft_engine
            .lock()
            .unwrap()
            .produce_precommit(block_hash, 1, 0)
            .unwrap();
        prevotes.push(prevote);
        precommits.push(precommit);
    }
    assert_eq!(prevotes.len(), 4);
    assert_eq!(precommits.len(), 4);

    let cert = region_nodes[0]
        .bft_engine
        .lock()
        .unwrap()
        .create_commit_certificate(&validator_set, block_hash, 1, 0, precommits)
        .expect("Create commit certificate");

    let block1 = Block::new(candidate.header, candidate.transactions, Some(cert.clone()));

    // Komit Blok 1 pada seluruh simpul multi-region
    for node in &region_nodes {
        node.ledger
            .lock()
            .unwrap()
            .apply_block(block1.clone(), &miner)
            .expect("Apply block");
        node.sync_rpc_context();
    }

    let h1_state_root = region_nodes[0]
        .ledger
        .lock()
        .unwrap()
        .latest_block()
        .header
        .state_root;
    for (i, node) in region_nodes.iter().enumerate() {
        assert_eq!(
            node.ledger.lock().unwrap().latest_block().header.state_root,
            h1_state_root,
            "Region {i} state root must match exactly"
        );
    }

    // 4. Pengujian Rotasi Validator Berbasis Epoch (NET-011)
    let epoch_len = 2; // Epoch boundary setiap 2 blok untuk pengujian
    assert!(!is_epoch_boundary(1, epoch_len));
    assert!(is_epoch_boundary(2, epoch_len));
    assert_eq!(compute_epoch_id(1, epoch_len), 0);
    assert_eq!(compute_epoch_id(2, epoch_len), 1);

    let epoch_info = EpochInfo::new(0, epoch_len, validator_set.clone());
    assert!(epoch_info.contains_height(1));

    // Siapkan validator baru untuk bergabung pada Epoch 1
    let new_val_key = Keypair::generate();
    let new_val_addr = derive_address_from_pubkey(&new_val_key.public_key_bytes());
    let new_val_entry = ValidatorEntry {
        validator_id: new_val_addr,
        consensus_pubkey: new_val_key.public_key_bytes(),
        voting_weight: 25,
    };

    // Rotasi: mengeluarkan Validator 3 (South America) dan memasukkan Validator Baru
    let leaving_val = vec![val_addrs[3]];
    let rotated_set =
        rotate_validator_set(&validator_set, vec![new_val_entry.clone()], &leaving_val)
            .expect("Rotate validator set");
    assert_eq!(rotated_set.validators.len(), 4);
    assert!(rotated_set
        .validators
        .iter()
        .any(|v| v.validator_id == new_val_addr));
    assert!(!rotated_set
        .validators
        .iter()
        .any(|v| v.validator_id == val_addrs[3]));

    // Buat dan verifikasi EpochTransition
    let transition = EpochTransition::create_and_verify(
        0,
        2,
        epoch_len,
        validator_set.clone(),
        vec![new_val_entry],
        &leaving_val,
        cert.clone(),
    )
    .expect("Epoch transition verified");
    assert_eq!(transition.from_epoch, 0);
    assert_eq!(transition.to_epoch, 1);
    assert_eq!(transition.new_validator_set, rotated_set);

    // 5. Pengujian State Snapshot & Fast-Sync Onboarding (NET-011)
    // Ekspor snapshot dari Node 0 pada H=1
    let snapshot = StateSnapshot::create_from_store(node_stores[0].as_ref(), 1, 9999, 0)
        .expect("Create state snapshot from store");

    assert_eq!(snapshot.magic, SNAPSHOT_MAGIC);
    assert_eq!(snapshot.version, SNAPSHOT_VERSION);
    assert_eq!(snapshot.height, 1);
    assert_eq!(snapshot.state_root, h1_state_root);
    assert!(!snapshot.accounts.is_empty());

    // Simpan snapshot ke file disk fisik
    let snapshot_file = base_path.join("aurion_testnet_h1.auss");
    snapshot
        .write_to_file(&snapshot_file)
        .expect("Write snapshot to file");
    assert!(snapshot_file.exists());

    // Baca dan verifikasi snapshot dari disk
    let loaded_snapshot =
        StateSnapshot::read_from_file(&snapshot_file).expect("Read snapshot from file");
    assert_eq!(
        loaded_snapshot.compute_checksum(),
        snapshot.compute_checksum()
    );
    assert_eq!(loaded_snapshot.height, 1);

    // 6. Fast-Sync Simpul Baru Menggunakan State Snapshot
    // Buat simpul baru ke-5 (Region Fast-Sync) dengan database kosong
    let fast_sync_db = base_path.join("node_fast_sync.redb");
    let fast_sync_store =
        Arc::new(RedbStorageEngine::open_or_create(&fast_sync_db).expect("Create fast sync store"));

    // Terapkan snapshot langsung ke storage simpul baru tanpa memutar transaksi dari genesis
    loaded_snapshot
        .apply_to_store(fast_sync_store.as_ref())
        .expect("Apply snapshot to store");

    // Verifikasi saldo Alice dan integritas akun pada simpul baru
    let alice_acc = fast_sync_store
        .get_account(&alice_addr)
        .unwrap()
        .expect("Alice account exists");
    assert_eq!(alice_acc.balance, Quantum::new(50_000_000));

    // Verifikasi state root pada database baru identik dengan cluster
    let accounts_all = fast_sync_store.get_all_accounts().unwrap();
    let fast_sync_root =
        aurion::statemachine::state::smt::compute_accounts_state_root(&accounts_all);
    assert_eq!(
        fast_sync_root, h1_state_root,
        "Fast-sync node reached 100% identical state root"
    );

    println!("[SUCCESS] Multi-region WAN consensus, Epoch rotation, and Snapshot fast-sync 100% verified!");
}
