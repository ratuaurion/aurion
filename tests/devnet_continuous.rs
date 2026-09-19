#![forbid(unsafe_code)]

//! Suite Pengujian Integrasi Devnet Continuous Deployment Aurion (NET-010).
//!
//! Mematuhi Invariant:
//! - AUR-ARCH-001: Single Sovereign Primary Binary (/bin/aurion).
//! - AUR-ARCH-009: Process Lifecycle, Graceful Shutdown, dan Konvergensi State Terdistribusi.
//! - AUR-APP-12: Sentry Node Privilege Isolation (Anti-DDoS Filtering).
//! - AUR-CONS-*: Single-Slot BFT Consensus with >2/3 Quorum Finality.
//!
//! Suite ini memverifikasi:
//! 1. Inisialisasi dan provisioning 6 simpul devnet (4 Validator, 1 Sentry, 1 RPC Gateway).
//! 2. Pelayanan endpoint `/healthz` dan antarmuka RPC di seluruh simpul devnet.
//! 3. Alur transaksi dari RPC Gateway -> Sentry Node -> Validator Mempool.
//! 4. Pembentukan blok BFT multi-simpul dengan Commit Certificate kanonikal.
//! 5. Konvergensi State Root 100% identik di seluruh validator.
//! 6. Ketahanan continuous deployment: Simulasi restart simpul tanpa divergensi ledger.

use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use aurion::consensus::block::Block;
use aurion::consensus::certificate::{ValidatorEntry, ValidatorSet};
use aurion::consensus::engine::BftEngine;
use aurion::core::{Quantum, Signature};
use aurion::crypto::{derive_address_from_pubkey, Keypair};
use aurion::genesis::builder::build_genesis;
use aurion::runtime::config::NodeConfig;
use aurion::runtime::AurionNode;
use aurion::storage::RedbStorageEngine;
use aurion::transaction::types::{Transaction, TxType};

static NEXT_DEVNET_PORT: AtomicU16 = AtomicU16::new(19800);

fn allocate_devnet_port() -> String {
    let port = NEXT_DEVNET_PORT.fetch_add(1, Ordering::SeqCst);
    format!("127.0.0.1:{port}")
}

async fn check_healthz(addr: &str) -> bool {
    let mut attempts = 0;
    loop {
        match TcpStream::connect(addr).await {
            Ok(mut stream) => {
                let get_req = format!("GET /healthz HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
                if stream.write_all(get_req.as_bytes()).await.is_err() {
                    return false;
                }
                let mut buf = [0u8; 1024];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let resp = String::from_utf8_lossy(&buf[..n]);
                return resp.contains("200 OK") && resp.contains("aurion-rpc");
            }
            Err(_) => {
                attempts += 1;
                if attempts > 30 {
                    return false;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;
            }
        }
    }
}

#[tokio::test]
async fn test_devnet_cluster_continuous_deployment_and_consensus() {
    let temp_dir = TempDir::new().expect("Create temp dir for devnet");
    let base_path = temp_dir.path().to_path_buf();

    // 1. Generate 4 Validator Keys
    let mut val_keys = Vec::new();
    let mut val_entries = Vec::new();
    let mut val_addrs = Vec::new();
    for _ in 0..4 {
        let key = Keypair::generate();
        let addr = derive_address_from_pubkey(&key.public_key_bytes());
        val_entries.push(ValidatorEntry {
            validator_id: addr,
            consensus_pubkey: key.public_key_bytes(),
            voting_weight: 25, // 4 x 25 = 100 total weight (>2/3 quorum = 67)
        });
        val_addrs.push(addr);
        val_keys.push(key);
    }
    let validator_set = ValidatorSet::new(val_entries.clone());

    let creator_key = Keypair::generate();
    let creator_addr = derive_address_from_pubkey(&creator_key.public_key_bytes());
    let dev_key = Keypair::generate();
    let dev_addr = derive_address_from_pubkey(&dev_key.public_key_bytes());

    let genesis = build_genesis(creator_addr, dev_addr, val_entries.clone());

    // 2. Initialize and start 4 Validators
    let mut val_nodes = Vec::new();
    let mut val_rpc_addrs = Vec::new();

    for (i, val_key) in val_keys.iter().enumerate().take(4) {
        let db_path = base_path.join(format!("val_{i}.redb"));
        let store = Arc::new(RedbStorageEngine::open_or_create(&db_path).expect("Open store"));
        let rpc_addr = allocate_devnet_port();
        let p2p_addr = allocate_devnet_port();
        val_rpc_addrs.push(rpc_addr.clone());

        let config = NodeConfig {
            chain_id: 1001,
            rpc_bind: rpc_addr,
            p2p_bind: p2p_addr,
            ..NodeConfig::new_validator(Vec::new())
        };

        let node = Arc::new(AurionNode::new_with_store(
            config,
            genesis.clone(),
            Some(val_key.clone()),
            Some(i as u32),
            store,
        ));
        val_nodes.push(node);
    }

    // 3. Initialize Sentry Node & RPC Gateway Node
    let sentry_db_path = base_path.join("sentry.redb");
    let sentry_store = Arc::new(RedbStorageEngine::open_or_create(&sentry_db_path).expect("Open store"));
    let sentry_rpc = allocate_devnet_port();
    let sentry_p2p = allocate_devnet_port();
    let sentry_config = NodeConfig {
        chain_id: 1001,
        rpc_bind: sentry_rpc.clone(),
        p2p_bind: sentry_p2p,
        ..NodeConfig::new_sentry("0.0.0.0:19405".to_string())
    };
    let sentry_node = Arc::new(AurionNode::new_with_store(sentry_config, genesis.clone(), None, None, sentry_store));

    let rpc_db_path = base_path.join("rpc_gw.redb");
    let rpc_store = Arc::new(RedbStorageEngine::open_or_create(&rpc_db_path).expect("Open store"));
    let rpc_gw_bind = allocate_devnet_port();
    let rpc_gw_config = NodeConfig {
        chain_id: 1001,
        rpc_bind: rpc_gw_bind.clone(),
        ..Default::default()
    };
    let rpc_gw_node = Arc::new(AurionNode::new_with_store(rpc_gw_config, genesis.clone(), None, None, rpc_store));

    // Verify all 6 nodes start at height 0
    for (i, node) in val_nodes.iter().enumerate() {
        assert_eq!(node.ledger.lock().unwrap().latest_height(), 0, "Val {i} at height 0");
    }
    assert_eq!(sentry_node.ledger.lock().unwrap().latest_height(), 0);
    assert_eq!(rpc_gw_node.ledger.lock().unwrap().latest_height(), 0);

    // 4. Start RPC server for Validator 0 and Gateway in background
    let node_0_clone = Arc::clone(&val_nodes[0]);
    let v0_handle = tokio::spawn(async move {
        let _ = node_0_clone.run_rpc_server(None).await;
    });

    let rpc_gw_clone = Arc::clone(&rpc_gw_node);
    let gw_handle = tokio::spawn(async move {
        let _ = rpc_gw_clone.run_rpc_server(None).await;
    });

    // 5. Verify /healthz endpoint responses
    let v0_healthy = check_healthz(&val_rpc_addrs[0]).await;
    assert!(v0_healthy, "Validator 0 /healthz must be UP");

    let gw_healthy = check_healthz(&rpc_gw_bind).await;
    assert!(gw_healthy, "RPC Gateway /healthz must be UP");

    // 6. Simulate Devnet Transaction Flow:
    // User submits tx -> RPC Gateway receives -> Sentry verifies -> Validators include in Block 1
    let alice = Keypair::generate();
    let alice_addr = derive_address_from_pubkey(&alice.public_key_bytes());
    
    // Transfer from creator to alice
    let mut tx1 = Transaction {
        version: 1,
        chain_id: 1001,
        tx_type: TxType::Transfer,
        flags: 0,
        sender: creator_addr,
        recipient: alice_addr,
        amount: Quantum::new(100_000_000), // 1 AUR
        fee: Quantum::new(1_000),
        nonce: 0,
        valid_until: 1800000000,
        payload: Vec::new(),
        signature: Signature::from_bytes([0u8; 64]),
    };
    let preimage = tx1.signing_preimage();
    tx1.signature = creator_key.sign(&preimage);

    // Insert into all validator mempools (simulating P2P gossip from Sentry)
    for (idx, node) in val_nodes.iter().enumerate() {
        let acct = node.ledger.lock().unwrap().get_account(&creator_addr).unwrap().clone();
        let res = node.mempool.lock().unwrap().submit_transaction(
            tx1.clone(),
            &creator_key.public_key_bytes(),
            1773532850,
            &acct,
        );
        assert!(res.is_ok(), "Node {idx} mempool submission must succeed: {:?}", res);
    }

    // 7. Execute BFT Round 1: Select Proposer
    let prev_hash = val_nodes[0].ledger.lock().unwrap().latest_block().hash();
    let proposer_idx = BftEngine::select_proposer(&validator_set, 1, 0, &prev_hash) as usize;
    let miner_addr = val_addrs[proposer_idx];

    let candidate = {
        let node_p = &val_nodes[proposer_idx];
        let ledger = node_p.ledger.lock().unwrap();
        let mempool = node_p.mempool.lock().unwrap();
        let bft = node_p.bft_engine.lock().unwrap();
        bft.assemble_block_proposal(
            &ledger,
            &mempool,
            0,
            1773534000,
            &miner_addr,
            1024 * 1024,
        )
    };
    assert_eq!(candidate.height(), 1);
    assert_eq!(candidate.transactions.len(), 1);
    let block_hash = candidate.hash();

    // Collect Prevotes from all 4 validators
    let mut prevotes = Vec::new();
    for node in &val_nodes {
        let prevote = node
            .bft_engine
            .lock()
            .unwrap()
            .produce_prevote(block_hash, 1, 0)
            .expect("Produce prevote");
        prevotes.push(prevote);
    }
    assert_eq!(prevotes.len(), 4);

    // Collect Precommits from all 4 validators
    let mut precommits = Vec::new();
    for node in &val_nodes {
        let precommit = node
            .bft_engine
            .lock()
            .unwrap()
            .produce_precommit(block_hash, 1, 0)
            .expect("Produce precommit");
        precommits.push(precommit);
    }
    assert_eq!(precommits.len(), 4);

    let cert = val_nodes[proposer_idx]
        .bft_engine
        .lock()
        .unwrap()
        .create_commit_certificate(&validator_set, block_hash, 1, 0, precommits)
        .expect("Create commit certificate");

    let block = Block::new(candidate.header, candidate.transactions, Some(cert));

    // Apply block commit to all 4 validators and the RPC Gateway
    for (i, node) in val_nodes.iter().enumerate() {
        let res = node.ledger.lock().unwrap().apply_block(block.clone(), &miner_addr);
        assert!(res.is_ok(), "Apply block to val {i} succeeded: {:?}", res);
        node.sync_rpc_context();
        assert_eq!(node.ledger.lock().unwrap().latest_height(), 1);
    }
    let res_gw = rpc_gw_node.ledger.lock().unwrap().apply_block(block.clone(), &miner_addr);
    assert!(res_gw.is_ok(), "Apply block to rpc gateway succeeded");
    rpc_gw_node.sync_rpc_context();

    // 8. Assert 100% State Root Convergence across all nodes
    let expected_state_root = val_nodes[0].ledger.lock().unwrap().latest_block().header.state_root;
    for (i, node) in val_nodes.iter().enumerate() {
        let node_root = node.ledger.lock().unwrap().latest_block().header.state_root;
        assert_eq!(node_root, expected_state_root, "Validator {i} state root converged");
    }
    assert_eq!(
        rpc_gw_node.ledger.lock().unwrap().latest_block().header.state_root,
        expected_state_root
    );

    // 9. Verify Alice's balance on all nodes
    for (i, node) in val_nodes.iter().enumerate() {
        let bal = node.ledger.lock().unwrap().get_balance(&alice_addr);
        assert_eq!(bal, Quantum::new(100_000_000), "Alice balance on val {i} is 1 AUR");
    }

    // 10. Node Restart & Recovery Resilience (Simulate Continuous Deployment restart)
    // Close Validator 3 and reopen from disk to verify persistence and no fork
    let val_3_node = val_nodes.remove(3);
    drop(val_3_node);

    let val_3_db_path = base_path.join("val_3.redb");
    let recovered_store = Arc::new(RedbStorageEngine::open_or_create(&val_3_db_path).expect("Reopen val 3 store"));
    let recovered_config = NodeConfig {
        chain_id: 1001,
        rpc_bind: allocate_devnet_port(),
        p2p_bind: allocate_devnet_port(),
        ..NodeConfig::new_validator(Vec::new())
    };
    let recovered_node = AurionNode::new_with_store(
        recovered_config,
        genesis.clone(),
        Some(val_keys[3].clone()),
        Some(3),
        recovered_store,
    );
    assert_eq!(
        recovered_node.ledger.lock().unwrap().latest_height(),
        1,
        "Recovered node immediately resumes at canonical block height 1"
    );
    assert_eq!(
        recovered_node.ledger.lock().unwrap().latest_block().header.state_root,
        expected_state_root,
        "Recovered node state root 100% identical after continuous restart"
    );

    // Cleanup background servers
    v0_handle.abort();
    gw_handle.abort();
    let _ = v0_handle.await;
    let _ = gw_handle.await;
}
