#![forbid(unsafe_code)]

//! Suite Pengujian Integrasi Public Testnet & Community Sandbox Aurion (NET-012).
//!
//! Mematuhi Invariant:
//! - AUR-ARCH-001: Single Sovereign Primary Binary (/bin/aurion).
//! - AUR-ARCH-009: Process Lifecycle, Graceful Shutdown, dan Konvergensi State.
//! - AUR-ARCH-011: Absolute Zero Unsafe Code (#![forbid(unsafe_code)]).
//! - AUR-ARCH-012: Absolute Zero Float Arithmetic (Fixed Precision Quantum u128).
//!
//! Suite ini memverifikasi:
//! 1. Penanganan CORS Preflight OPTIONS dan header CORS pada seluruh respons HTTP.
//! 2. Metode JSON-RPC `aur_getNetworkStats` dan `aur_requestFaucet`.
//! 3. Testnet Faucet dispenser: transfer token uji coba sukses dan penegakan cooldown anti-abuse.
//! 4. Endpoint REST Explorer (/explorer/stats, /explorer/block/:height, /explorer/tx/:hash).
//! 5. Penyajian antarmuka Web Sandbox interaktif mandiri (/sandbox).
//! 6. Siklus hidup terpadu pengembang komunitas: klaim faucet -> eksekusi blok -> saldo terverifikasi.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::watch;

use aurion::consensus::block::Block;
use aurion::consensus::certificate::{ValidatorEntry, ValidatorSet};
use aurion::consensus::header::BlockHeader;
use aurion::core::{Hash256, Quantum, Signature};
use aurion::crypto::{derive_address_from_pubkey, encode_address_bech32m, Keypair};
use aurion::gateway::faucet::{FaucetConfig, FaucetDispenser};
use aurion::gateway::rpc::methods::RpcContext;
use aurion::gateway::rpc::pubsub::SubscriptionManager;
use aurion::gateway::rpc::server::RpcServer;
use aurion::genesis::builder::build_genesis;
use aurion::runtime::config::NodeConfig;
use aurion::runtime::AurionNode;
use aurion::state::account::Account;
use aurion::storage::RedbStorageEngine;
use aurion::transaction::types::{Transaction, TxType};

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

#[tokio::test]
async fn test_public_gateway_cors_preflight_and_endpoints() {
    let ctx = Arc::new(RpcContext::new(9999));
    ctx.current_height.store(10, Ordering::SeqCst);
    ctx.finalized_height.store(10, Ordering::SeqCst);

    // Daftarkan dummy block header
    let h1 = make_test_header(1, 1773534000);
    ctx.headers.lock().unwrap().insert(1, h1);

    // Daftarkan dummy akun
    let dummy_key = Keypair::generate();
    let dummy_addr = derive_address_from_pubkey(&dummy_key.public_key_bytes());
    ctx.accounts.lock().unwrap().insert(dummy_addr, Account::new(Quantum::new(50_000_000), 1));

    // Siapkan Faucet
    let faucet_key = Keypair::generate();
    let faucet_addr = derive_address_from_pubkey(&faucet_key.public_key_bytes());
    ctx.accounts.lock().unwrap().insert(faucet_addr, Account::new(Quantum::new(100_000_000_000), 0));
    let dispenser = FaucetDispenser::new(faucet_key, 9999, FaucetConfig::default());
    ctx.attach_faucet(dispenser);

    let pubsub = Arc::new(SubscriptionManager::new());
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    drop(listener);

    let server = RpcServer::new(ctx.clone(), pubsub, &local_addr.to_string()).with_shutdown(shutdown_rx);
    let server_handle = tokio::spawn(async move {
        let _ = server.run().await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // 1. Verifikasi CORS Preflight OPTIONS
    {
        let mut stream = TcpStream::connect(local_addr).await.expect("Connect TCP");
        let options_req = "OPTIONS / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        stream.write_all(options_req.as_bytes()).await.unwrap();

        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(resp.contains("204 No Content"), "Must respond 204 No Content to OPTIONS");
        assert!(resp.contains("Access-Control-Allow-Origin: *"), "Must include CORS allow origin");
        assert!(resp.contains("Access-Control-Allow-Methods:"), "Must include CORS methods");
    }

    // 2. Verifikasi Header CORS pada Healthz GET
    {
        let mut stream = TcpStream::connect(local_addr).await.expect("Connect TCP");
        let get_req = "GET /healthz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        stream.write_all(get_req.as_bytes()).await.unwrap();

        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(resp.contains("200 OK"));
        assert!(resp.contains("Access-Control-Allow-Origin: *"));
    }

    // 3. Verifikasi REST Explorer Stats (/explorer/stats)
    {
        let mut stream = TcpStream::connect(local_addr).await.expect("Connect TCP");
        let get_req = "GET /explorer/stats HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        stream.write_all(get_req.as_bytes()).await.unwrap();

        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(resp.contains("200 OK"));
        assert!(resp.contains(r#""chain_id":9999"#));
        assert!(resp.contains(r#""current_height":10"#));
        assert!(resp.contains(r#""faucet":{"enabled":true"#));
    }

    // 4. Verifikasi REST Explorer Block Detail (/explorer/block/1)
    {
        let mut stream = TcpStream::connect(local_addr).await.expect("Connect TCP");
        let get_req = "GET /explorer/block/1 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        stream.write_all(get_req.as_bytes()).await.unwrap();

        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(resp.contains("200 OK"));
        assert!(resp.contains(r#""height":1"#));
        assert!(resp.contains(r#""block_hash":"#));
    }

    // 5. Verifikasi Penyajian Web Sandbox UI (/sandbox)
    {
        let mut stream = TcpStream::connect(local_addr).await.expect("Connect TCP");
        let get_req = "GET /sandbox HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        stream.write_all(get_req.as_bytes()).await.unwrap();

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        let resp = String::from_utf8_lossy(&buf);
        assert!(resp.contains("200 OK"));
        assert!(resp.contains("text/html"));
        assert!(resp.contains("AURION PUBLIC TESTNET"));
        assert!(resp.contains("Claim 10 AUR"));
    }

    // 6. Verifikasi Metode JSON-RPC `aur_getNetworkStats`
    {
        let mut stream = TcpStream::connect(local_addr).await.expect("Connect TCP");
        let payload = r#"{"jsonrpc":"2.0","id":10,"method":"aur_getNetworkStats","params":[]}"#;
        let post_req = format!(
            "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            payload.len(),
            payload
        );
        stream.write_all(post_req.as_bytes()).await.unwrap();

        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(resp.contains("200 OK"));
        assert!(resp.contains(r#""chain_id":9999"#));
        assert!(resp.contains(r#""faucet_active":true"#));
    }

    // 7. Verifikasi Permintaan Faucet via JSON-RPC `aur_requestFaucet`
    let alice = Keypair::generate();
    let alice_addr = derive_address_from_pubkey(&alice.public_key_bytes());
    let alice_bech = encode_address_bech32m(&alice_addr, "aur").unwrap();

    let tx_hash_str = {
        let mut stream = TcpStream::connect(local_addr).await.expect("Connect TCP");
        let payload = format!(
            r#"{{"jsonrpc":"2.0","id":11,"method":"aur_requestFaucet","params":["{}"]}}"#,
            alice_bech
        );
        let post_req = format!(
            "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            payload.len(),
            payload
        );
        stream.write_all(post_req.as_bytes()).await.unwrap();

        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(resp.contains("200 OK"));
        assert!(resp.contains(r#""result":"0x"#), "Faucet must return tx hash");

        // Ekstrak hash dari respons
        let idx = resp.find(r#""result":"0x"#).unwrap();
        let start = idx + 10;
        let end = resp[start..].find('"').unwrap() + start;
        resp[start..end].to_string()
    };

    // Verifikasi transaksi faucet telah masuk antrean mempool
    assert_eq!(ctx.mempool.lock().unwrap().entries.len(), 1);

    // 8. Verifikasi REST Explorer Tx Detail (/explorer/tx/<hash>)
    {
        let mut stream = TcpStream::connect(local_addr).await.expect("Connect TCP");
        let get_req = format!("GET /explorer/tx/{} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n", tx_hash_str);
        stream.write_all(get_req.as_bytes()).await.unwrap();

        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(resp.contains("200 OK"));
        assert!(resp.contains("PENDING_IN_MEMPOOL"));
        assert!(resp.contains(&alice_bech));
        assert!(resp.contains("1000000000")); // 10 AUR in Quanta
    }

    // 9. Verifikasi Penolakan Cooldown (Anti-Abuse Rate Limiting)
    {
        let mut stream = TcpStream::connect(local_addr).await.expect("Connect TCP");
        let payload = format!(
            r#"{{"jsonrpc":"2.0","id":12,"method":"aur_requestFaucet","params":["{}"]}}"#,
            alice_bech
        );
        let post_req = format!(
            "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            payload.len(),
            payload
        );
        stream.write_all(post_req.as_bytes()).await.unwrap();

        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(resp.contains("Faucet cooldown active"), "Immediate repeated request must be rejected with cooldown active");
    }

    // Shutdown server
    let _ = shutdown_tx.send(true);
    let _ = server_handle.await;
}

#[tokio::test]
async fn test_end_to_end_community_faucet_and_transfer_lifecycle() {
    let temp_dir = TempDir::new().expect("Create temp dir");
    let db_path = temp_dir.path().join("community_node.redb");
    let store = Arc::new(RedbStorageEngine::open_or_create(&db_path).expect("Open redb"));

    let val_key = Keypair::generate();
    let val_addr = derive_address_from_pubkey(&val_key.public_key_bytes());
    let dev_key = Keypair::generate();
    let dev_addr = derive_address_from_pubkey(&dev_key.public_key_bytes());

    let val_entry = ValidatorEntry {
        validator_id: val_addr,
        consensus_pubkey: val_key.public_key_bytes(),
        voting_weight: 100,
    };
    let genesis = build_genesis(val_addr, dev_addr, vec![val_entry]);

    let config = NodeConfig {
        chain_id: 9999,
        rpc_bind: "127.0.0.1:19985".to_string(),
        p2p_bind: "127.0.0.1:19986".to_string(),
        ..NodeConfig::new_validator(Vec::new())
    };

    let node = Arc::new(AurionNode::new_with_store(
        config,
        genesis.clone(),
        Some(val_key.clone()),
        Some(0),
        store,
    ));

    // Siapkan Faucet dengan Developer Keypair (memiliki alokasi 3.300.000 AUR di genesis)
    let mut faucet = FaucetDispenser::new(dev_key.clone(), 9999, FaucetConfig::default());
    assert_eq!(faucet.address(), dev_addr);

    // Pengembang komunitas baru (Bob) meminta 10 AUR dari Faucet
    let bob_key = Keypair::generate();
    let bob_addr = derive_address_from_pubkey(&bob_key.public_key_bytes());

    let (faucet_tx_hash, faucet_tx) = {
        let ledger = node.ledger.lock().unwrap();
        let mut accounts_map = HashMap::new();
        // Ambil akun dev
        let dev_acc = ledger.get_account(&dev_addr).unwrap().clone();
        accounts_map.insert(dev_addr, dev_acc);

        let mut mempool = node.mempool.lock().unwrap();
        faucet.dispense(&bob_addr, &accounts_map, &mut mempool, 1773533000).expect("Dispense faucet tokens")
    };
    assert_eq!(faucet_tx.amount, Quantum::new(1_000_000_000)); // 10 AUR

    // Simpul memproses proposal Blok 1 yang memuat transaksi Faucet
    let candidate = {
        let ledger = node.ledger.lock().unwrap();
        let mempool = node.mempool.lock().unwrap();
        let bft = node.bft_engine.lock().unwrap();
        bft.assemble_block_proposal(&ledger, &mempool, 0, 1773533100, &val_addr, 1024 * 1024)
    };
    assert_eq!(candidate.transactions.len(), 1);
    assert_eq!(candidate.transactions[0].compute_tx_id(), faucet_tx_hash);

    let block_hash = candidate.hash();
    let precommit = node.bft_engine.lock().unwrap().produce_precommit(block_hash, 1, 0).unwrap();
    let cert = node.bft_engine.lock().unwrap().create_commit_certificate(
        &ValidatorSet::new(vec![ValidatorEntry {
            validator_id: val_addr,
            consensus_pubkey: val_key.public_key_bytes(),
            voting_weight: 100,
        }]),
        block_hash,
        1,
        0,
        vec![precommit],
    ).expect("Commit certificate");

    let block1 = Block::new(candidate.header, candidate.transactions, Some(cert));

    // Komit Blok 1 ke ledger
    node.ledger.lock().unwrap().apply_block(block1, &val_addr).expect("Apply block 1");
    node.sync_rpc_context();

    // Verifikasi saldo Bob di on-chain ledger telah menerima 10 AUR penuh
    let bob_acc = node.ledger.lock().unwrap().get_account(&bob_addr).unwrap().clone();
    assert_eq!(bob_acc.balance, Quantum::new(1_000_000_000), "Bob received 10 AUR from testnet faucet");
    assert_eq!(bob_acc.nonce, 0);

    // Sekarang Bob mengirim 2 AUR (200.000.000 Quanta) ke Charlie menggunakan dana faucet
    let charlie_key = Keypair::generate();
    let charlie_addr = derive_address_from_pubkey(&charlie_key.public_key_bytes());

    let mut bob_tx = Transaction {
        version: 1,
        chain_id: 9999,
        tx_type: TxType::Transfer,
        flags: 0,
        sender: bob_addr,
        recipient: charlie_addr,
        amount: Quantum::new(200_000_000), // 2 AUR
        fee: Quantum::new(2_000),
        nonce: 0,
        valid_until: 1800000000,
        payload: Vec::new(),
        signature: Signature::from_bytes([0u8; 64]),
    };
    let preimage = bob_tx.signing_preimage();
    bob_tx.signature = bob_key.sign(&preimage);

    node.mempool.lock().unwrap().submit_transaction(
        bob_tx.clone(),
        &bob_key.public_key_bytes(),
        1773533200,
        &bob_acc,
    ).expect("Bob tx submitted to mempool");

    // Rakit & Komit Blok 2
    let candidate2 = {
        let ledger = node.ledger.lock().unwrap();
        let mempool = node.mempool.lock().unwrap();
        let bft = node.bft_engine.lock().unwrap();
        bft.assemble_block_proposal(&ledger, &mempool, 0, 1773533300, &val_addr, 1024 * 1024)
    };
    assert_eq!(candidate2.transactions.len(), 1);

    let block2_hash = candidate2.hash();
    let precommit2 = node.bft_engine.lock().unwrap().produce_precommit(block2_hash, 2, 0).unwrap();
    let cert2 = node.bft_engine.lock().unwrap().create_commit_certificate(
        &ValidatorSet::new(vec![ValidatorEntry {
            validator_id: val_addr,
            consensus_pubkey: val_key.public_key_bytes(),
            voting_weight: 100,
        }]),
        block2_hash,
        2,
        0,
        vec![precommit2],
    ).expect("Commit certificate 2");

    let block2 = Block::new(candidate2.header, candidate2.transactions, Some(cert2));
    node.ledger.lock().unwrap().apply_block(block2, &val_addr).expect("Apply block 2");
    node.sync_rpc_context();

    // Verifikasi saldo Charlie = 2 AUR
    let charlie_acc = node.ledger.lock().unwrap().get_account(&charlie_addr).unwrap().clone();
    assert_eq!(charlie_acc.balance, Quantum::new(200_000_000));

    // Verifikasi saldo Bob = 10 AUR - 2 AUR - 2000 fee = 799.998.000 Quanta
    let bob_acc_updated = node.ledger.lock().unwrap().get_account(&bob_addr).unwrap().clone();
    assert_eq!(bob_acc_updated.balance, Quantum::new(1_000_000_000 - 200_000_000 - 2_000));
    assert_eq!(bob_acc_updated.nonce, 1);

    println!("[SUCCESS] Public Testnet Faucet and Community Lifecycle 100% verified!");
}
