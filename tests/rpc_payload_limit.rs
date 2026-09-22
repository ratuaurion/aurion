use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use aurion::gateway::rpc::methods::RpcContext;
use aurion::gateway::rpc::pubsub::SubscriptionManager;
use aurion::gateway::rpc::server::handle_connection;

const MAX_RPC_PAYLOAD_BYTES: usize = 128 * 1024;

async fn round_trip_http_request(request: &[u8]) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let context = Arc::new(RpcContext::new(1001));
        let pubsub = Arc::new(SubscriptionManager::new());
        let _ = handle_connection(stream, addr, context, pubsub).await;
    });

    let mut client = TcpStream::connect(addr).await.unwrap();
    client.write_all(request).await.unwrap();
    let mut received = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match client.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => received.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }
    }
    let _ = server.await;

    String::from_utf8_lossy(&received).into_owned()
}

#[tokio::test]
async fn rpc_accepts_small_valid_payload() {
    let payload = r#"{"jsonrpc":"2.0","method":"aur_blockHeight","params":[],"id":1}"#;
    let request = format!(
        "POST /rpc HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        payload.len(),
        payload
    );

    let response = round_trip_http_request(request.as_bytes()).await;
    assert!(response.contains("HTTP/1.1 200 OK"));
    assert!(response.contains("\"jsonrpc\":\"2.0\""));
    assert!(response.contains("\"id\":1"));
}

#[tokio::test]
async fn rpc_rejects_oversized_payload_before_buffering() {
    let oversized_len = MAX_RPC_PAYLOAD_BYTES + 1;
    let request = format!(
        "POST /rpc HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        oversized_len
    );

    let response = round_trip_http_request(request.as_bytes()).await;
    assert!(response.contains("HTTP/1.1 413 Payload Too Large"));
    assert!(response.contains("Payload too large"));
}
