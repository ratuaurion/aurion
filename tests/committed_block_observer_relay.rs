#![forbid(unsafe_code)]

use aurion::consensus::bft::{publish_committed_block, ZenohBftObserver};
use aurion::consensus::header::BlockHeader;
use aurion::core::Hash256;
use aurion::genesis::builder::GENESIS_CHAIN_ID;
use std::time::Duration;
use zenoh::config::Config as ZenohConfig;

fn sample_block(height: u64) -> aurion::consensus::bft::Block {
    aurion::consensus::bft::Block::new(
        BlockHeader {
            version: 1,
            height,
            round: 0,
            timestamp: height,
            prev_block_hash: if height == 1 {
                Hash256::ZERO
            } else {
                Hash256::from_bytes([height as u8; 32])
            },
            tx_merkle_root: Hash256::ZERO,
            state_root: Hash256::ZERO,
        },
        Vec::new(),
        None,
    )
}

async fn open_peer(
    listen: Option<&str>,
    connect: &[&str],
) -> zenoh::Session {
    let mut config = ZenohConfig::default();
    config
        .insert_json5("mode", r#""peer""#)
        .unwrap();
    config
        .insert_json5("scouting/multicast/enabled", "false")
        .unwrap();
    match listen {
        Some(endpoint) => config
            .insert_json5("listen/endpoints", &format!(r#"["{endpoint}"]"#))
            .unwrap(),
        None => config
            .insert_json5("listen/endpoints", "[]")
            .unwrap(),
    };
    if !connect.is_empty() {
        let endpoints = connect
            .iter()
            .map(|endpoint| format!(r#""{endpoint}""#))
            .collect::<Vec<_>>()
            .join(",");
        config
            .insert_json5("connect/endpoints", &format!("[{endpoints}]"))
            .unwrap();
    }
    zenoh::open(config).await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn committed_block_relays_through_peer_hub_to_observer() {
    let hub = open_peer(Some("tcp/127.0.0.1:29447"), &[]).await;
    let mut observer = ZenohBftObserver::open(GENESIS_CHAIN_ID, None, &["tcp/127.0.0.1:29447"])
        .await
        .unwrap();
    let publisher = open_peer(Some("tcp/127.0.0.1:29448"), &["tcp/127.0.0.1:29447"]).await;
    tokio::time::sleep(Duration::from_millis(1200)).await;

    for height in 1..=4u64 {
        publish_committed_block(&publisher, GENESIS_CHAIN_ID, 1, &sample_block(height))
            .await
            .unwrap();
    }

    let received = tokio::time::timeout(Duration::from_secs(6), observer.recv())
        .await
        .expect("observer should receive a relayed committed block through the hub");
    let block = received.unwrap();
    assert_eq!(block.height(), 1);

    drop(hub);
    drop(observer);
    drop(publisher);
}