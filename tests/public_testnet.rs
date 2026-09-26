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
use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::crypto::{derive_address_from_pubkey, encode_address_bech32m, Keypair};
use aurion::gateway::faucet::{FaucetConfig, FaucetDispenser, FaucetError};
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

/// Rakit proposal, buat precommit, dan komit satu blok BFT lengkap dengan
/// sertifikat.
///
/// Mengotomatisasi alur berulang (rakit proposal -> precommit -> sertifikat commit
/// -> apply -> sync) sehingga setiap skenario lifecycle hanya perlu fokus pada
/// logika bisnisnya.
fn commit_block(
    node: &Arc<AurionNode>,
    val_key: &Keypair,
    val_addr: Address,
    height: u64,
    timestamp: u64,
) -> Block {
    let candidate = {
        let ledger = node.ledger.lock().unwrap();
        let mempool = node.mempool.lock().unwrap();
        let bft = node.bft_engine.lock().unwrap();
        bft.assemble_block_proposal(&ledger, &mempool, 0, timestamp, &val_addr, 1024 * 1024)
    };

    let block_hash = candidate.hash();
    let precommit = node
        .bft_engine
        .lock()
        .unwrap()
        .produce_precommit(block_hash, height, 0)
        .expect("precommit");
    let cert = node
        .bft_engine
        .lock()
        .unwrap()
        .create_commit_certificate(
            &ValidatorSet::new(vec![ValidatorEntry {
                validator_id: val_addr,
                consensus_pubkey: val_key.public_key_bytes(),
                voting_weight: 100,
            }]),
            block_hash,
            height,
            0,
            vec![precommit],
        )
        .expect("Commit certificate");

    let block = Block::new(candidate.header, candidate.transactions, Some(cert));
    node.ledger
        .lock()
        .unwrap()
        .apply_block(block.clone(), &val_addr)
        .unwrap_or_else(|e| panic!("apply block {height} gagal: {e}"));
    node.sync_rpc_context();
    block
}

/// Saldo sebuah akun pada ledger node.
fn balance_of(node: &Arc<AurionNode>, address: &Address) -> Quantum {
    node.ledger
        .lock()
        .unwrap()
        .get_account(address)
        .cloned()
        .unwrap_or_default()
        .balance
}

#[tokio::test]
async fn test_end_to_end_community_faucet_and_transfer_lifecycle() {
    let temp_dir = TempDir::new().expect("Create temp dir");
    let db_path = temp_dir.path().join("community_node.redb");
    let store = Arc::new(RedbStorageEngine::open_or_create(&db_path).expect("Open redb"));

    let val_key = Keypair::generate();
    let val_addr = derive_address_from_pubkey(&val_key.public_key_bytes());
    let faucet_key = Keypair::generate();
    let faucet_addr = derive_address_from_pubkey(&faucet_key.public_key_bytes());

    let val_entry = ValidatorEntry {
        validator_id: val_addr,
        consensus_pubkey: val_key.public_key_bytes(),
        voting_weight: 100,
    };
    // Genesis Aurion mengalokasikan 100% pasokan ke Master Treasury SAJA
    // (AURION-GENESIS-SPECIFICATION.md Bagian 3.1). Akun faucet karena itu
    // lahir dengan saldo 0 dan WAJIB didanai dari Master Treasury terlebih
    // dahulu (AURION CONSTITUTION.md: Aturan Faucet).
    let genesis = build_genesis(val_addr, faucet_addr, vec![val_entry]);

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

    // Treasury memegang 100% pasokan; akun faucet mulai dari nol.
    assert_eq!(
        balance_of(&node, &val_addr),
        Quantum::new(66_000_000_000_000_000)
    );
    assert_eq!(balance_of(&node, &faucet_addr), Quantum::ZERO);

    // BLOK 1: Master Treasury mendanai akun faucet (100.000 AUR).
    let faucet_funding = Quantum::new(100_000_000_000);
    let mut funding_tx = Transaction {
        version: 1,
        chain_id: 9999,
        tx_type: TxType::Transfer,
        flags: 0,
        sender: val_addr,
        recipient: faucet_addr,
        amount: faucet_funding,
        fee: Quantum::new(2_000),
        nonce: 0,
        valid_until: 1_800_000_000,
        payload: Vec::new(),
        signature: Signature::from_bytes([0u8; 64]),
    };
    let preimage = funding_tx.signing_preimage();
    funding_tx.signature = val_key.sign(&preimage);

    {
        let treasury_acc = node
            .ledger
            .lock()
            .unwrap()
            .get_account(&val_addr)
            .cloned()
            .expect("treasury account");
        node.mempool
            .lock()
            .unwrap()
            .submit_transaction(
                funding_tx,
                &val_key.public_key_bytes(),
                1_773_533_050,
                &treasury_acc,
            )
            .expect("funding tx submitted");
    }
    let block1 = commit_block(&node, &val_key, val_addr, 1, 1_773_533_100);
    assert_eq!(block1.transactions.len(), 1);
    assert_eq!(balance_of(&node, &faucet_addr), faucet_funding);

    // BLOK 2: Pengembang komunitas baru (Bob) meminta 10 AUR dari Faucet.
    let mut faucet = FaucetDispenser::new(faucet_key.clone(), 9999, FaucetConfig::default());
    assert_eq!(faucet.address(), faucet_addr);

    let bob_key = Keypair::generate();
    let bob_addr = derive_address_from_pubkey(&bob_key.public_key_bytes());

    let (faucet_tx_hash, faucet_tx) = {
        let ledger = node.ledger.lock().unwrap();
        let mut accounts_map = HashMap::new();
        let faucet_acc = ledger.get_account(&faucet_addr).cloned().expect("faucet account");
        accounts_map.insert(faucet_addr, faucet_acc);

        let mut mempool = node.mempool.lock().unwrap();
        faucet
            .dispense(&bob_addr, &accounts_map, &mut mempool, 1_773_533_200)
            .expect("Dispense faucet tokens")
    };
    assert_eq!(faucet_tx.amount, Quantum::new(1_000_000_000)); // 10 AUR

    let block2 = commit_block(&node, &val_key, val_addr, 2, 1_773_533_300);
    assert_eq!(block2.transactions.len(), 1);
    assert_eq!(block2.transactions[0].compute_tx_id(), faucet_tx_hash);

    // Bob menerima 10 AUR penuh di ledger on-chain.
    assert_eq!(
        balance_of(&node, &bob_addr),
        Quantum::new(1_000_000_000),
        "Bob should receive 10 AUR from the testnet faucet"
    );
    let bob_acc = node
        .ledger
        .lock()
        .unwrap()
        .get_account(&bob_addr)
        .cloned()
        .expect("bob account");
    assert_eq!(bob_acc.nonce, 0);

    // BLOK 3: Bob mengirim 2 AUR (200.000.000 Quanta) ke Charlie memakai dana faucet.
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
        valid_until: 1_800_000_000,
        payload: Vec::new(),
        signature: Signature::from_bytes([0u8; 64]),
    };
    let preimage = bob_tx.signing_preimage();
    bob_tx.signature = bob_key.sign(&preimage);

    node.mempool
        .lock()
        .unwrap()
        .submit_transaction(
            bob_tx,
            &bob_key.public_key_bytes(),
            1_773_533_400,
            &bob_acc,
        )
        .expect("Bob tx submitted to mempool");

    let block3 = commit_block(&node, &val_key, val_addr, 3, 1_773_533_500);
    assert_eq!(block3.transactions.len(), 1);

    // Charlie menerima 2 AUR.
    assert_eq!(balance_of(&node, &charlie_addr), Quantum::new(200_000_000));

    // Bob = 10 AUR - 2 AUR - 2.000 fee = 799.998.000 Quanta, nonce naik ke 1.
    let bob_final = node
        .ledger
        .lock()
        .unwrap()
        .get_account(&bob_addr)
        .cloned()
        .expect("bob account");
    assert_eq!(
        bob_final.balance,
        Quantum::new(1_000_000_000 - 200_000_000 - 2_000)
    );
    assert_eq!(bob_final.nonce, 1);

    // Cooldown anti-abuse: reclaim oleh alamat yang SAMA dalam 60 detik ditolak.
    // Pakai instance faucet yang sama dengan blok 2 (sudah mencatat Bob pada
    // t=1_773_533_200), lalu klaim lagi pada t+10 detik.
    {
        let ledger = node.ledger.lock().unwrap();
        let mut accounts_map = HashMap::new();
        accounts_map.insert(
            faucet_addr,
            ledger.get_account(&faucet_addr).cloned().unwrap_or_default(),
        );
        let mut mempool = node.mempool.lock().unwrap();
        let cooldown = faucet
            .dispense(&bob_addr, &accounts_map, &mut mempool, 1_773_533_210)
            .expect_err("reclaim by same address within cooldown must be rejected");
        assert!(
            matches!(cooldown, FaucetError::CooldownActive(50)),
            "expected CooldownActive(50), got: {cooldown:?}"
        );

        // Charlie adalah penerima berbeda: tidak terkena cooldown Bob.
        faucet
            .dispense(&charlie_addr, &accounts_map, &mut mempool, 1_773_533_210)
            .expect("different recipient is not blocked by another address cooldown");
    }

    println!("[SUCCESS] Public Testnet Faucet and Community Lifecycle 100% verified!");
}
