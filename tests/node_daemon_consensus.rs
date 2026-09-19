#![forbid(unsafe_code)]

use aurion::consensus::bft::ZenohBftTransport;
use aurion::genesis::builder::GENESIS_CHAIN_ID;
use aurion::genesis::ceremony::{CanonicalCeremonyKeypairs, CeremonyTranscript};
use aurion::runtime::config::{NodeConfig, NodeRole};
use aurion::runtime::AurionNode;
use aurion::storage::{RedbStorageEngine, StateStore};
use std::sync::Arc;
use std::time::Duration;
use tempfile::NamedTempFile;

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_daemon_boots_and_advances_height_over_zenoh() {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let genesis = CeremonyTranscript::canonical_mainnet_genesis();
    let ports = [18447u16, 18448, 18449, 18450];
    let mut nodes = Vec::new();
    let mut paths = Vec::new();
    let mut shutdowns = Vec::new();
    let mut handles = Vec::new();

    for (index, port) in ports.iter().enumerate() {
        let path = NamedTempFile::new().unwrap();
        let store = Arc::new(RedbStorageEngine::open_or_create(path.path()).unwrap());
        let config = NodeConfig {
            chain_id: GENESIS_CHAIN_ID,
            is_dev_mode: true,
            role: NodeRole::Validator,
            p2p_bind: format!("127.0.0.1:{port}"),
            rpc_bind: format!("127.0.0.1:{}", 18547 + index as u16),
            ..Default::default()
        };
        let node = Arc::new(AurionNode::new_with_store(
            config,
            genesis.clone(),
            Some(keys.validators[index].clone()),
            Some(index as u32),
            store,
        ));
        let endpoint = format!("tcp/127.0.0.1:{port}");
        let peers = ports
            .iter()
            .enumerate()
            .filter(|(peer_index, _)| *peer_index != index)
            .map(|(_, peer)| format!("tcp/127.0.0.1:{peer}"))
            .collect::<Vec<_>>();
        let peer_refs = peers.iter().map(String::as_str).collect::<Vec<_>>();
        let zenoh = ZenohBftTransport::open(index as u32, GENESIS_CHAIN_ID, &endpoint, &peer_refs)
            .await
            .unwrap();
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = node
            .clone()
            .clone()
            .spawn_consensus_engine_with_shutdown(
                index as u32,
                keys.validators[index].clone(),
                zenoh.session(),
                shutdown_rx,
            )
            .await
            .unwrap();
        nodes.push(node);
        paths.push(path);
        shutdowns.push(shutdown_tx);
        handles.push(handle);
    }

    tokio::time::sleep(Duration::from_secs(2)).await;
    let reached = tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            if nodes.iter().all(|node| {
                node.rpc_context
                    .current_height
                    .load(std::sync::atomic::Ordering::SeqCst)
                    >= 1
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        reached.is_ok(),
        "all daemon consensus engines must commit height 1"
    );

    let heights = nodes
        .iter()
        .map(|node| {
            node.rpc_context
                .current_height
                .load(std::sync::atomic::Ordering::SeqCst)
        })
        .collect::<Vec<_>>();
    assert!(heights.iter().all(|height| *height >= 1));
    let blocks = nodes
        .iter()
        .map(|node| node.ledger.lock().unwrap().latest_block().clone())
        .collect::<Vec<_>>();
    assert!(blocks
        .windows(2)
        .all(|pair| pair[0].hash() == pair[1].hash()));
    assert!(blocks
        .windows(2)
        .all(|pair| pair[0].header.state_root == pair[1].header.state_root));

    for shutdown in &shutdowns {
        shutdown.send(true).unwrap();
    }
    for handle in handles {
        tokio::time::timeout(Duration::from_secs(3), handle)
            .await
            .unwrap()
            .unwrap();
    }
    drop(nodes);

    for path in paths {
        let store = RedbStorageEngine::open_or_create(path.path()).unwrap();
        assert!(store.get_latest_height().unwrap().unwrap() >= 1);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_daemon_graceful_shutdown_releases_redb_lock() {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let genesis = CeremonyTranscript::canonical_mainnet_genesis();
    let path = NamedTempFile::new().unwrap();
    let store = Arc::new(RedbStorageEngine::open_or_create(path.path()).unwrap());
    let node = Arc::new(AurionNode::new_with_store(
        NodeConfig {
            chain_id: GENESIS_CHAIN_ID,
            is_dev_mode: true,
            role: NodeRole::Validator,
            p2p_bind: "127.0.0.1:18451".to_string(),
            rpc_bind: "127.0.0.1:18551".to_string(),
            ..Default::default()
        },
        genesis,
        Some(keys.validators[0].clone()),
        Some(0),
        store,
    ));
    let zenoh = ZenohBftTransport::open(0, GENESIS_CHAIN_ID, "tcp/127.0.0.1:18451", &[])
        .await
        .unwrap();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let handle = node
        .clone()
        .spawn_consensus_engine_with_shutdown(
            0,
            keys.validators[0].clone(),
            zenoh.session(),
            shutdown_rx,
        )
        .await
        .unwrap();
    shutdown_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(3), handle)
        .await
        .unwrap()
        .unwrap();
    drop(node);
    let reopened = RedbStorageEngine::open_or_create(path.path()).unwrap();
    assert_eq!(reopened.get_latest_height().unwrap(), Some(0));
}
