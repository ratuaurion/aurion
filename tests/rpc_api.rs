//! Pengujian Integrasi Suite JSON-RPC 2.0 & WebSocket Server Aurion.
//! Mematuhi Dokumen 02 (02-RPC-API-RULES.md) dan Invariant AUR-ARCH-011 / 012.

#![forbid(unsafe_code)]

use aurion::consensus::certificate::CommitCertificate;
use aurion::consensus::header::BlockHeader;
use aurion::core::{Address, Hash256, Quantum};
use aurion::crypto::encode_address_bech32m;
use aurion::gateway::rpc::consistency::ConsistencySelector;
use aurion::gateway::rpc::errors::*;
use aurion::gateway::rpc::methods::RpcContext;
use aurion::gateway::rpc::pubsub::{SubscriptionManager, SubscriptionTopic};
use aurion::gateway::rpc::server::RpcServer;
use aurion::gateway::rpc::types::{JsonRpcId, JsonRpcRequest};
use aurion::state::account::Account;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};

fn make_test_header(height: u64, timestamp: u64) -> BlockHeader {
    BlockHeader {
        version: 1,
        height,
        round: 0,
        timestamp,
        prev_block_hash: Hash256::ZERO,
        tx_merkle_root: Hash256::ZERO,
        state_root: Hash256::ZERO,
    }
}

#[test]
fn test_rpc_request_parser_deterministic() {
    let raw = r#"{"jsonrpc":"2.0","id":42,"method":"aur_blockHeight","params":[]}"#;
    let req = JsonRpcRequest::parse(raw).expect("Harus valid");
    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.id, JsonRpcId::Number(42));
    assert_eq!(req.method, "aur_blockHeight");
    assert!(req.params.is_empty());
}

#[test]
fn test_rpc_consistency_selector_parsing() {
    assert_eq!(
        ConsistencySelector::parse("finalized").unwrap(),
        ConsistencySelector::Finalized
    );
    assert_eq!(
        ConsistencySelector::parse("safe").unwrap(),
        ConsistencySelector::Safe
    );
    assert_eq!(
        ConsistencySelector::parse("latest").unwrap(),
        ConsistencySelector::Latest
    );
    assert_eq!(
        ConsistencySelector::parse("0x64").unwrap(),
        ConsistencySelector::SpecificHeight(100)
    );
    assert_eq!(
        ConsistencySelector::parse("100").unwrap(),
        ConsistencySelector::SpecificHeight(100)
    );

    let err = ConsistencySelector::parse("invalid_status").unwrap_err();
    assert_eq!(err.code, ERR_INVALID_PARAMS);
}

#[test]
fn test_rpc_methods_chain_info() {
    let ctx = RpcContext::new(1);
    ctx.current_height.store(105, Ordering::SeqCst);
    ctx.finalized_height.store(100, Ordering::SeqCst);

    // 1. aur_chainId
    let req_chain = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: JsonRpcId::Number(1),
        method: "aur_chainId".to_string(),
        params: vec![],
    };
    let resp = ctx.dispatch(&req_chain, 1000);
    assert!(resp.error.is_none());
    assert_eq!(resp.result.unwrap(), "1");

    // 2. aur_blockHeight
    let req_height = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: JsonRpcId::Number(2),
        method: "aur_blockHeight".to_string(),
        params: vec![],
    };
    let resp_h = ctx.dispatch(&req_height, 1000);
    assert!(resp_h.error.is_none());
    assert_eq!(resp_h.result.unwrap(), "105");
}

#[test]
fn test_rpc_methods_account_state() {
    let ctx = RpcContext::new(1);
    let addr_bytes = [0x42u8; 32];
    let address = Address(addr_bytes);
    let bech32_addr = encode_address_bech32m(&address, "aur").expect("Valid bech32m");

    let acc = Account {
        nonce: 7,
        balance: Quantum::new(500_000_000), // 5 AUR
    };
    ctx.accounts.lock().unwrap().insert(address, acc);

    // aur_getBalance
    let req_bal = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: JsonRpcId::Number(10),
        method: "aur_getBalance".to_string(),
        params: vec![bech32_addr.clone(), "finalized".to_string()],
    };
    let resp_bal = ctx.dispatch(&req_bal, 1000);
    assert!(resp_bal.error.is_none());
    assert_eq!(resp_bal.result.unwrap(), "\"500000000\"");

    // aur_getNonce
    let req_nonce = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: JsonRpcId::Number(11),
        method: "aur_getNonce".to_string(),
        params: vec![bech32_addr.clone()],
    };
    let resp_n = ctx.dispatch(&req_nonce, 1000);
    assert!(resp_n.error.is_none());
    assert_eq!(resp_n.result.unwrap(), "7");

    // aur_getAccount
    let req_acc = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: JsonRpcId::Number(12),
        method: "aur_getAccount".to_string(),
        params: vec![bech32_addr],
    };
    let resp_a = ctx.dispatch(&req_acc, 1000);
    assert!(resp_a.error.is_none());
    assert!(resp_a.result.unwrap().contains(r#""nonce":7"#));
}

#[test]
fn test_rpc_block_and_certificate_queries() {
    let ctx = RpcContext::new(1);
    ctx.finalized_height.store(1, Ordering::SeqCst);
    let header = make_test_header(1, 1000);
    let block_hash = header.compute_block_hash();

    ctx.headers.lock().unwrap().insert(1, header);

    let cert = CommitCertificate {
        height: 1,
        round: 0,
        block_hash,
        precommits: vec![],
    };
    ctx.certificates.lock().unwrap().insert(1, cert);

    // aur_getBlockByHeight dengan parameter "finalized" sukses karena height <= finalized_height
    let req_bh = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: JsonRpcId::Number(20),
        method: "aur_getBlockByHeight".to_string(),
        params: vec!["1".to_string(), "finalized".to_string()],
    };
    let resp_bh = ctx.dispatch(&req_bh, 1000);
    assert!(resp_bh.error.is_none());
    assert!(resp_bh.result.unwrap().contains(r#""height":1"#));

    // Validasi penolakan konsistensi: meminta height 2 dengan "finalized" harus ditolak (-32003)
    let req_unfin = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: JsonRpcId::Number(99),
        method: "aur_getBlockByHeight".to_string(),
        params: vec!["2".to_string(), "finalized".to_string()],
    };
    let resp_unfin = ctx.dispatch(&req_unfin, 1000);
    assert!(resp_unfin.result.is_none());
    assert_eq!(resp_unfin.error.unwrap().code, ERR_FINALITY_NOT_REACHED);

    // aur_getBlockByHash
    let req_hash = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: JsonRpcId::Number(21),
        method: "aur_getBlockByHash".to_string(),
        params: vec![hex::encode(block_hash.as_bytes())],
    };
    let resp_hash = ctx.dispatch(&req_hash, 1000);
    assert!(resp_hash.error.is_none());

    // aur_getCommitCertificate
    let req_cert = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: JsonRpcId::Number(22),
        method: "aur_getCommitCertificate".to_string(),
        params: vec!["1".to_string()],
    };
    let resp_cert = ctx.dispatch(&req_cert, 1000);
    assert!(resp_cert.error.is_none());
    assert!(resp_cert.result.unwrap().contains(r#""height":1"#));
}

#[test]
fn test_rpc_error_handling_unknown_method() {
    let ctx = RpcContext::new(1);
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: JsonRpcId::Number(99),
        method: "aur_unknownMagicFunction".to_string(),
        params: vec![],
    };
    let resp = ctx.dispatch(&req, 1000);
    assert!(resp.result.is_none());
    let err = resp.error.unwrap();
    assert_eq!(err.code, ERR_METHOD_NOT_FOUND);
}

#[tokio::test]
async fn test_websocket_pubsub_manager() {
    let pubsub = Arc::new(SubscriptionManager::new());
    let (tx_new, mut rx_new) = mpsc::unbounded_channel::<String>();
    let (tx_fin, mut rx_fin) = mpsc::unbounded_channel::<String>();

    let sub_id1 = pubsub.subscribe(SubscriptionTopic::NewHeads, tx_new);
    let sub_id2 = pubsub.subscribe(SubscriptionTopic::FinalizedHeads, tx_fin);

    assert_eq!(pubsub.active_subscribers_count(), 2);

    let header = make_test_header(1, 1000);
    pubsub.notify_new_head(&header);
    pubsub.notify_finalized_head(&header);

    let msg1 = rx_new.recv().await.expect("Harus menerima newHeads");
    assert!(msg1.contains("newHeads") || msg1.contains("aur_subscription"));
    assert!(msg1.contains(&format!(r#""subscription":"{sub_id1}""#)));

    let msg2 = rx_fin.recv().await.expect("Harus menerima finalizedHeads");
    assert!(msg2.contains("FINALIZED"));
    assert!(msg2.contains(&format!(r#""subscription":"{sub_id2}""#)));

    // Unsubscribe
    assert!(pubsub.unsubscribe(sub_id1));
    assert_eq!(pubsub.active_subscribers_count(), 1);
}

#[tokio::test]
async fn test_rpc_server_http_and_healthz() {
    let ctx = Arc::new(RpcContext::new(1));
    ctx.current_height.store(42, Ordering::SeqCst);
    let pubsub = Arc::new(SubscriptionManager::new());

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    // Cari port bebas
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    drop(listener);

    let server = RpcServer::new(ctx, pubsub, &local_addr.to_string()).with_shutdown(shutdown_rx);

    let server_handle = tokio::spawn(async move {
        let _ = server.run().await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // 1. Tes Healthz GET
    let mut stream = TcpStream::connect(local_addr).await.expect("Connect TCP");
    let get_req = "GET /healthz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
    stream.write_all(get_req.as_bytes()).await.unwrap();

    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(resp.contains("200 OK"));
    assert!(resp.contains(r#"{"status":"OK","service":"aurion-rpc"}"#));

    // 2. Tes JSON-RPC 2.0 POST
    let mut stream_rpc = TcpStream::connect(local_addr).await.expect("Connect TCP");
    let payload = r#"{"jsonrpc":"2.0","id":1,"method":"aur_blockHeight","params":[]}"#;
    let post_req = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        payload.len(),
        payload
    );
    stream_rpc.write_all(post_req.as_bytes()).await.unwrap();

    let mut buf_rpc = [0u8; 1024];
    let n_rpc = stream_rpc.read(&mut buf_rpc).await.unwrap();
    let resp_rpc = String::from_utf8_lossy(&buf_rpc[..n_rpc]);
    assert!(resp_rpc.contains("200 OK"));
    assert!(resp_rpc.contains(r#""result":42"#));

    // Shutdown server
    let _ = shutdown_tx.send(true);
    let _ = server_handle.await;
}
