#![forbid(unsafe_code)]

use aurion::codec::CanonicalEncode;
use aurion::consensus::bft::ZenohBftTransport;
use aurion::core::{Address, Quantum, Signature};
use aurion::genesis::builder::GENESIS_CHAIN_ID;
use aurion::genesis::ceremony::{CanonicalCeremonyKeypairs, CeremonyTranscript};
use aurion::runtime::config::{NodeConfig, NodeRole};
use aurion::runtime::AurionNode;
use aurion::storage::{RedbStorageEngine, StateStore};
use aurion::transaction::types::{Transaction, TxType};
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn http(addr: &str, method: &str, path: &str, body: Option<String>) -> String {
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let payload = body.unwrap_or_default();
    let request = if method == "POST" {
        format!(
            "POST {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            payload.len()
        )
    } else {
        format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n")
    };
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    String::from_utf8(response).unwrap()
}

async fn wait_height(nodes: &[Arc<AurionNode>], height: u64) {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if nodes
                .iter()
                .all(|node| node.rpc_context.current_height.load(Ordering::SeqCst) >= height)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("devnet must advance autonomously");
}

fn json_balance(response: &str) -> u128 {
    response
        .split("\"balance\":\"")
        .nth(1)
        .and_then(|value| value.split('"').next())
        .and_then(|value| value.parse().ok())
        .expect("account response must contain an integer balance")
}

fn signed_transfer(key: &aurion::crypto::Keypair, recipient: Address) -> Transaction {
    let mut tx = Transaction {
        version: 1,
        chain_id: GENESIS_CHAIN_ID,
        tx_type: TxType::Transfer,
        flags: 0,
        sender: key.derive_address(),
        recipient,
        nonce: 0,
        amount: Quantum::new(100),
        fee: Quantum::new(10_000),
        valid_until: u64::MAX,
        payload: Vec::new(),
        signature: Signature::from_bytes([0; 64]),
    };
    tx.signature = key.sign(&tx.signing_preimage());
    tx
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_devnet_multinode_rpc_fault_recovery_and_teardown() {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let genesis = CeremonyTranscript::canonical_mainnet_genesis();
    let ports = [19447u16, 19448, 19449, 19450];
    let rpc_ports = [19547u16, 19548, 19549, 19550];
    let mut nodes = Vec::new();
    let mut paths = Vec::new();
    let mut consensus = Vec::new();
    let mut rpc_tasks = Vec::new();
    let mut shutdowns = Vec::new();
    let mut sessions = Vec::new();
    let mut rx_channels = Vec::new();

    for index in 0..4 {
        let path = NamedTempFile::new().unwrap();
        let store = Arc::new(RedbStorageEngine::open_or_create(path.path()).unwrap());
        let node = Arc::new(AurionNode::new_with_store(
            NodeConfig {
                chain_id: GENESIS_CHAIN_ID,
                is_dev_mode: true,
                role: NodeRole::Validator,
                p2p_bind: format!("127.0.0.1:{}", ports[index]),
                rpc_bind: format!("127.0.0.1:{}", rpc_ports[index]),
                ..Default::default()
            },
            genesis.clone(),
            Some(keys.validators[index].clone()),
            Some(index as u32),
            store,
        ));
        let endpoint = format!("tcp/127.0.0.1:{}", ports[index]);
        let peers = ports
            .iter()
            .enumerate()
            .filter(|(peer, _)| *peer != index)
            .map(|(_, port)| format!("tcp/127.0.0.1:{port}"))
            .collect::<Vec<_>>();
        let peer_refs = peers.iter().map(String::as_str).collect::<Vec<_>>();
        let zenoh_session =
            ZenohBftTransport::open_session(GENESIS_CHAIN_ID, &endpoint, &peer_refs)
                .await
                .unwrap();
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let rpc_node = node.clone();
        let rpc_task = tokio::spawn(async move {
            rpc_node.run_rpc_server(None).await.unwrap();
        });
        nodes.push(node);
        paths.push(path);
        sessions.push(zenoh_session);
        rx_channels.push(shutdown_rx);
        rpc_tasks.push(rpc_task);
        shutdowns.push(shutdown_tx);
    }

    for (index, (session, shutdown_rx)) in sessions.into_iter().zip(rx_channels.into_iter()).enumerate() {
        let consensus_task = nodes[index]
            .clone()
            .spawn_consensus_engine_with_shutdown(
                index as u32,
                keys.validators[index].clone(),
                session,
                shutdown_rx,
            )
            .await
            .unwrap();
        consensus.push(Some(consensus_task));
    }

    wait_height(&nodes, 3).await;
    let mut responses = Vec::new();
    for port in rpc_ports {
        responses.push(
            http(
                &format!("127.0.0.1:{port}"),
                "GET",
                "/rpc/block/latest",
                None,
            )
            .await,
        );
    }
    for response in &responses {
        assert!(response.contains("\"height\":3") || response.contains("\"height\":4"));
        assert!(
            response.contains("\"signatures_count\":3")
                || response.contains("\"signatures_count\":4")
        );
    }
    let first_hash = responses[0]
        .split("\"block_hash\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let first_root = responses[0]
        .split("\"state_root\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    for response in &responses[1..] {
        assert!(response.contains(first_hash));
        assert!(response.contains(first_root));
    }

    let transaction_height = nodes[0].rpc_context.current_height.load(Ordering::SeqCst);
    let recipient = Address([0xD4; 32]);
    let tx = signed_transfer(&keys.creator, recipient);
    let raw = hex::encode(tx.to_canonical_bytes());
    let pubkey = hex::encode(keys.creator.public_key_bytes());
    let request = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"aur_sendRawTransaction","params":["{raw}","{pubkey}"]}}"#
    );
    let sender_hex = hex::encode(tx.sender.0);
    let initial_sender_response = http(
        &format!("127.0.0.1:{}", rpc_ports[1]),
        "POST",
        "/",
        Some(format!(
            r#"{{"jsonrpc":"2.0","id":4,"method":"aur_getAccount","params":["{sender_hex}","latest"]}}"#
        )),
    )
    .await;
    let initial_sender_balance = json_balance(&initial_sender_response);
    let response = http(
        &format!("127.0.0.1:{}", rpc_ports[1]),
        "POST",
        "/",
        Some(request.clone()),
    )
    .await;
    assert!(
        response.contains("\"result\""),
        "RPC transaction submission failed: {response}"
    );
    wait_height(&nodes, transaction_height + 1).await;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let account = http(
                &format!("127.0.0.1:{}", rpc_ports[0]),
                "POST",
                "/",
                Some(format!(
                    r#"{{"jsonrpc":"2.0","id":3,"method":"aur_getAccount","params":["{}","latest"]}}"#,
                    hex::encode(recipient.0)
                )),
            )
            .await;
            if account.contains("\"balance\":\"100\"") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("gossiped transaction must eventually be included");
    for port in rpc_ports {
        let account = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let response = http(
                    &format!("127.0.0.1:{port}"),
                    "POST",
                    "/",
                    Some(format!(
                        r#"{{"jsonrpc":"2.0","id":2,"method":"aur_getAccount","params":["{}","latest"]}}"#,
                        hex::encode(recipient.0)
                    )),
                )
                .await;
                if response.contains("\"balance\":\"100\"") {
                    break response;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("every validator must converge on the transaction state");
        assert!(
            account.contains("\"balance\":\"100\""),
            "gossiped transaction was not reflected by node {port}: {account}"
        );
        let sender = http(
            &format!("127.0.0.1:{port}"),
            "POST",
            "/",
            Some(format!(
                r#"{{"jsonrpc":"2.0","id":5,"method":"aur_getAccount","params":["{sender_hex}","latest"]}}"#
            )),
        )
        .await;
        assert_eq!(
            json_balance(&sender),
            initial_sender_balance - 10_100,
            "sender balance must account for amount and fee on node {port}"
        );
    }

    let offline = consensus[3].take().unwrap();
    shutdowns[3].send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(3), offline)
        .await
        .unwrap()
        .unwrap();
    let old_rpc = std::mem::replace(&mut rpc_tasks[3], tokio::spawn(async {}));
    old_rpc.abort();
    let _ = old_rpc.await;
    let offline_node = nodes.swap_remove(3);
    drop(offline_node);
    let before = nodes[0].rpc_context.current_height.load(Ordering::SeqCst);
    wait_height(&nodes[..3], before + 1).await;

    let rejoin_store = Arc::new(RedbStorageEngine::open_or_create(paths[3].path()).unwrap());
    let rejoined = Arc::new(AurionNode::new_with_store(
        NodeConfig {
            chain_id: GENESIS_CHAIN_ID,
            is_dev_mode: true,
            role: NodeRole::Validator,
            p2p_bind: format!("127.0.0.1:{}", ports[3]),
            rpc_bind: format!("127.0.0.1:{}", rpc_ports[3]),
            ..Default::default()
        },
        genesis.clone(),
        Some(keys.validators[3].clone()),
        Some(3),
        rejoin_store,
    ));
    let endpoint = format!("tcp/127.0.0.1:{}", ports[3]);
    let peers = ports[..3]
        .iter()
        .map(|port| format!("tcp/127.0.0.1:{port}"))
        .collect::<Vec<_>>();
    let peer_refs = peers.iter().map(String::as_str).collect::<Vec<_>>();
    let rejoin_session = ZenohBftTransport::open_session(GENESIS_CHAIN_ID, &endpoint, &peer_refs)
        .await
        .unwrap();

    let mut caught_up = 0;
    for _ in 0..10 {
        let applied = rejoined.catch_up_from_peer(&nodes[0]).unwrap();
        caught_up += applied;
        let target = nodes[0].ledger.lock().unwrap().latest_height();
        let current = rejoined.ledger.lock().unwrap().latest_height();
        if current >= target {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        caught_up >= 1,
        "rejoined validator must replay missed blocks"
    );
    let (rejoin_shutdown, rejoin_rx) = tokio::sync::watch::channel(false);
    let rejoin_consensus = rejoined
        .clone()
        .spawn_consensus_engine_with_shutdown(
            3,
            keys.validators[3].clone(),
            rejoin_session,
            rejoin_rx,
        )
        .await
        .unwrap();
    let rejoin_rpc_node = rejoined.clone();
    let rejoin_rpc = tokio::spawn(async move {
        rejoin_rpc_node.run_rpc_server(None).await.unwrap();
    });
    nodes.push(rejoined);
    shutdowns.push(rejoin_shutdown);
    consensus.push(Some(rejoin_consensus));
    rpc_tasks.push(rejoin_rpc);
    wait_height(&nodes, before + 2).await;

    for shutdown in &shutdowns {
        let _ = shutdown.send(true);
    }
    for task in consensus.into_iter().flatten() {
        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap();
    }
    for task in rpc_tasks {
        task.abort();
        let _ = task.await;
    }
    drop(nodes);
    for path in paths {
        let store = RedbStorageEngine::open_or_create(path.path()).unwrap();
        assert!(store.get_latest_height().unwrap().unwrap() >= 1);
    }
}
