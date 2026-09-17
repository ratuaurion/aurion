#![forbid(unsafe_code)]

//! Suite Pengujian Integrasi Multi-Node Cluster Aurion (/bin/aurion) (VER-006).
//!
//! Mematuhi Invariant:
//! - AUR-ARCH-001: Single Sovereign Ecosystem & Monolithic Executable (/bin/aurion).
//! - AUR-ARCH-009: Process Lifecycle, Graceful Shutdown, dan Konvergensi State Root Terdistribusi.
//!
//! Suite ini menguji:
//! 1. Inisialisasi kluster 4-simpul validator mandiri berbasis storage persisten `redb`.
//! 2. Pelayanan antarmuka JSON-RPC 2.0 dan endpoint `/healthz` pada masing-masing simpul.
//! 3. Konsensus BFT dua fase (Prevote & Precommit) terdistribusi antar 4 simpul independen.
//! 4. Konvergensi State Root 100% identik di seluruh simpul setelah komit blok transaksi.
//! 5. Rotasi proposer deterministik multi-blok sesuai tinggi dan putaran.
//! 6. Crash & Recovery simpul validator (toleransi 1 simpul offline, catchup saat pulih).
//! 7. Eksekusi biner fisik `/bin/aurion` via CLI subcommands (`version`, `genesis`, `block`).

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use aurion::consensus::block::Block;
use aurion::consensus::certificate::{ValidatorEntry, ValidatorSet};
use aurion::consensus::engine::BftEngine;
use aurion::core::{Address, Quantum, Signature};
use aurion::crypto::{derive_address_from_pubkey, Keypair};
use aurion::genesis::builder::{build_genesis, GenesisInitialization};
use aurion::runtime::config::NodeConfig;
use aurion::runtime::AurionNode;
use aurion::state::ChainLedger;
use aurion::storage::{RedbStorageEngine, StateStore};
use aurion::transaction::types::{Transaction, TxType};

static NEXT_PORT: AtomicU16 = AtomicU16::new(19400);

/// Helper untuk mengalokasikan port TCP lokal unik tanpa tabrakan port antar-test paralel.
fn allocate_unique_cluster_port() -> String {
    let port = NEXT_PORT.fetch_add(1, Ordering::SeqCst);
    format!("127.0.0.1:{port}")
}

/// Helper untuk melakukan panggilan HTTP JSON-RPC 2.0 dengan retry backoff.
async fn rpc_call(addr: &str, method: &str, params: serde_json::Value) -> serde_json::Value {
    let mut attempts = 0;
    loop {
        match TcpStream::connect(addr).await {
            Ok(mut stream) => {
                let req = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": method,
                    "params": params,
                });
                let payload = serde_json::to_string(&req).expect("Serialize JSON-RPC request");
                let post_req = format!(
                    "POST / HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                    payload.len()
                );

                stream
                    .write_all(post_req.as_bytes())
                    .await
                    .expect("Write HTTP POST request");

                let mut buf = [0u8; 4096];
                let n = stream
                    .read(&mut buf)
                    .await
                    .expect("Read HTTP response");

                let resp_str = String::from_utf8_lossy(&buf[..n]);
                let body = resp_str
                    .split("\r\n\r\n")
                    .nth(1)
                    .unwrap_or("");

                return serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
            }
            Err(e) => {
                attempts += 1;
                if attempts > 40 {
                    panic!("Failed to connect to node RPC at {addr} after {attempts} attempts: {e}");
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
            }
        }
    }
}

/// Helper untuk menguji endpoint HTTP GET `/healthz` dengan retry backoff.
async fn rpc_healthz(addr: &str) -> String {
    let mut attempts = 0;
    loop {
        match TcpStream::connect(addr).await {
            Ok(mut stream) => {
                let get_req = format!("GET /healthz HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
                stream
                    .write_all(get_req.as_bytes())
                    .await
                    .expect("Write HTTP GET request");

                let mut buf = [0u8; 2048];
                let n = stream
                    .read(&mut buf)
                    .await
                    .expect("Read HTTP response");

                return String::from_utf8_lossy(&buf[..n]).to_string();
            }
            Err(e) => {
                attempts += 1;
                if attempts > 40 {
                    panic!("Failed to connect to healthz at {addr} after {attempts} attempts: {e}");
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
            }
        }
    }
}

/// Struktur fixture kluster 4-simpul validator Aurion.
struct MultiNodeClusterFixture {
    _temp_dirs: Vec<TempDir>,
    db_paths: Vec<PathBuf>,
    rpc_binds: Vec<String>,
    _val_keys: Vec<Keypair>,
    val_addrs: Vec<Address>,
    _val_entries: Vec<ValidatorEntry>,
    validator_set: ValidatorSet,
    creator_key: Keypair,
    creator_addr: Address,
    dev_addr: Address,
    genesis: GenesisInitialization,
    nodes: Vec<Option<Arc<AurionNode>>>,
    server_handles: Vec<Option<tokio::task::JoinHandle<()>>>,
}

impl MultiNodeClusterFixture {
    async fn start(node_count: usize) -> Self {
        assert_eq!(node_count, 4, "Canonical cluster requires 4 validator nodes");

        let val_keys: Vec<Keypair> = (0..node_count).map(|_| Keypair::generate()).collect();
        let val_addrs: Vec<Address> = val_keys
            .iter()
            .map(|kp| derive_address_from_pubkey(&kp.public_key_bytes()))
            .collect();

        let val_entries: Vec<ValidatorEntry> = val_keys
            .iter()
            .enumerate()
            .map(|(idx, kp)| ValidatorEntry {
                validator_id: val_addrs[idx],
                consensus_pubkey: kp.public_key_bytes(),
                voting_weight: 25,
            })
            .collect();

        let validator_set = ValidatorSet::new(val_entries.clone());
        assert_eq!(validator_set.total_voting_power(), 100);
        assert_eq!(validator_set.quorum_threshold(), 67);

        let creator_key = Keypair::generate();
        let creator_addr = derive_address_from_pubkey(&creator_key.public_key_bytes());

        let dev_key = Keypair::generate();
        let dev_addr = derive_address_from_pubkey(&dev_key.public_key_bytes());

        let genesis = build_genesis(creator_addr, dev_addr, val_entries.clone());

        let mut temp_dirs = Vec::with_capacity(node_count);
        let mut db_paths = Vec::with_capacity(node_count);
        let mut rpc_binds = Vec::with_capacity(node_count);
        let mut nodes = Vec::with_capacity(node_count);
        let mut server_handles = Vec::with_capacity(node_count);

        for (idx, val_key) in val_keys.iter().enumerate().take(node_count) {
            let tmp_dir = TempDir::new().expect("Create tempdir for node");
            let db_path = tmp_dir.path().join(format!("aurion_node_{idx}.redb"));
            let rpc_bind = allocate_unique_cluster_port();

            let config = NodeConfig {
                chain_id: 1,
                rpc_bind: rpc_bind.clone(),
                ..NodeConfig::new_validator(Vec::new())
            };

            let store: Arc<dyn StateStore> = Arc::new(
                RedbStorageEngine::open_or_create(&db_path)
                    .expect("Storage engine open/create"),
            );

            let node = Arc::new(AurionNode::new_with_store(
                config,
                genesis.clone(),
                Some(val_key.clone()),
                Some(idx as u32),
                store,
            ));

            let node_for_server = Arc::clone(&node);
            let handle = tokio::spawn(async move {
                if let Err(e) = node_for_server.run_rpc_server(None).await {
                    eprintln!("RPC server error on {}: {e}", node_for_server.config.rpc_bind);
                }
            });

            temp_dirs.push(tmp_dir);
            db_paths.push(db_path);
            rpc_binds.push(rpc_bind);
            nodes.push(Some(node));
            server_handles.push(Some(handle));
        }

        // Beri waktu 100ms agar seluruh listener TCP RPC aktif
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Self {
            _temp_dirs: temp_dirs,
            db_paths,
            rpc_binds,
            _val_keys: val_keys,
            val_addrs,
            _val_entries: val_entries,
            validator_set,
            creator_key,
            creator_addr,
            dev_addr,
            genesis,
            nodes,
            server_handles,
        }
    }

    /// Ambil instance node aktif berdasarkan indeks.
    fn get_node(&self, idx: usize) -> Arc<AurionNode> {
        self.nodes[idx].as_ref().expect("Node is running").clone()
    }

    /// Matikan satu simpul secara mendadak (abort server dan lepaskan database lock).
    async fn crash_node(&mut self, idx: usize) {
        if let Some(handle) = self.server_handles[idx].take() {
            handle.abort();
            let _ = handle.await;
        }
        self.nodes[idx] = None;
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    /// Shutdown seluruh server RPC simpul kluster.
    async fn shutdown_all(&mut self) {
        for handle_opt in &mut self.server_handles {
            if let Some(handle) = handle_opt.take() {
                handle.abort();
                let _ = handle.await;
            }
        }
        self.nodes.clear();
    }
}

// ============================================================================
// SKENARIO 1: INISIALISASI KLUSTER & VERIFIKASI RPC/HEALTHZ PADA SELURUH SIMPUL
// ============================================================================
#[tokio::test]
async fn test_multi_node_cluster_initialization_and_rpc_health() {
    let mut cluster = MultiNodeClusterFixture::start(4).await;

    for (idx, addr) in cluster.rpc_binds.iter().enumerate() {
        // 1. Verifikasi HTTP GET /healthz
        let health = rpc_healthz(addr).await;
        assert!(
            health.contains("200 OK"),
            "Node {idx} healthz endpoint must return 200 OK"
        );
        assert!(
            health.contains(r#"{"status":"OK","service":"aurion-rpc"}"#),
            "Node {idx} healthz payload match"
        );

        // 2. Verifikasi JSON-RPC aur_blockHeight (Genesis H=0)
        let height_resp = rpc_call(addr, "aur_blockHeight", serde_json::json!([])).await;
        assert_eq!(
            height_resp.get("result").and_then(|v| v.as_u64()),
            Some(0),
            "Node {idx} initial height must be 0"
        );

        // 3. Verifikasi JSON-RPC aur_chainId
        let chain_resp = rpc_call(addr, "aur_chainId", serde_json::json!([])).await;
        assert_eq!(
            chain_resp.get("result").and_then(|v| v.as_u64()),
            Some(1),
            "Node {idx} chainId must be 1"
        );
    }

    cluster.shutdown_all().await;
}

// ============================================================================
// SKENARIO 2: KONSENSUS BFT KLUSTER & KONVERGENSI STATE ROOT IDENTIK
// ============================================================================
#[tokio::test]
async fn test_multi_node_bft_consensus_and_state_root_convergence() {
    let mut cluster = MultiNodeClusterFixture::start(4).await;

    // 1. Buat transaksi transfer 1.000 AUR dari Creator ke Dev di Node 0
    let tx_amount = Quantum::from_aur(1_000).unwrap();
    let tx_fee = Quantum::from_aur(10).unwrap();
    let mut tx = Transaction {
        version: 1,
        chain_id: 1,
        tx_type: TxType::Transfer,
        flags: 0,
        sender: cluster.creator_addr,
        recipient: cluster.dev_addr,
        nonce: 0,
        amount: tx_amount,
        fee: tx_fee,
        valid_until: 1800000000,
        payload: Vec::new(),
        signature: Signature::from_bytes([0u8; 64]),
    };
    let preimage = tx.signing_preimage();
    tx.signature = cluster.creator_key.sign(&preimage);

    // Kirim transaksi ke mempool seluruh simpul
    for idx in 0..4 {
        let node = cluster.get_node(idx);
        let acct = node.ledger.lock().unwrap().get_account(&cluster.creator_addr).unwrap().clone();
        node.mempool
            .lock()
            .unwrap()
            .submit_transaction(
                tx.clone(),
                &cluster.creator_key.public_key_bytes(),
                1773532850,
                &acct,
            )
            .unwrap_or_else(|e| panic!("Node {idx} mempool submission failed: {e:?}"));
    }

    // 2. Pilih Proposer Deterministik untuk H=1, R=0
    let prev_hash = cluster.get_node(0).ledger.lock().unwrap().latest_block().hash();
    let proposer_idx = BftEngine::select_proposer(&cluster.validator_set, 1, 0, &prev_hash) as usize;
    let miner_addr = cluster.val_addrs[proposer_idx];

    // Proposer merakit proposal blok H=1
    let candidate = {
        let node_p = cluster.get_node(proposer_idx);
        let ledger = node_p.ledger.lock().unwrap();
        let mempool = node_p.mempool.lock().unwrap();
        let bft = node_p.bft_engine.lock().unwrap();
        bft.assemble_block_proposal(&ledger, &mempool, 0, 1773532900, &miner_addr, 1024 * 1024)
    };
    assert_eq!(candidate.height(), 1);
    assert_eq!(candidate.transactions.len(), 1);

    let block_hash = candidate.hash();

    // 3. Pertukaran Suara Konsensus: Fase 1 (Prevote) dari Seluruh 4 Simpul
    let mut prevotes = Vec::new();
    for idx in 0..4 {
        let node = cluster.get_node(idx);
        let prevote = node.bft_engine
            .lock()
            .unwrap()
            .produce_prevote(block_hash, 1, 0)
            .unwrap_or_else(|e| panic!("Node {idx} prevote failed: {e:?}"));
        prevote.verify(&cluster.validator_set).expect("Prevote verify");
        prevotes.push(prevote);
    }
    assert_eq!(prevotes.len(), 4);

    // 4. Pertukaran Suara Konsensus: Fase 2 (Precommit) dari Seluruh 4 Simpul
    let mut precommits = Vec::new();
    for idx in 0..4 {
        let node = cluster.get_node(idx);
        let precommit = node.bft_engine
            .lock()
            .unwrap()
            .produce_precommit(block_hash, 1, 0)
            .unwrap_or_else(|e| panic!("Node {idx} precommit failed: {e:?}"));
        precommit.verify(&cluster.validator_set).expect("Precommit verify");
        precommits.push(precommit);
    }

    // 5. Agregasi CommitCertificate
    let cert = cluster.get_node(proposer_idx)
        .bft_engine
        .lock()
        .unwrap()
        .create_commit_certificate(&cluster.validator_set, block_hash, 1, 0, precommits)
        .expect("Commit certificate reaches 100 voting power (>= 67 quorum)");

    let finalized_block = Block::new(candidate.header, candidate.transactions, Some(cert));

    // 6. Terapkan blok ke ledger & persistent store pada seluruh 4 simpul
    for idx in 0..4 {
        let node = cluster.get_node(idx);
        node.ledger
            .lock()
            .unwrap()
            .apply_block(finalized_block.clone(), &miner_addr)
            .unwrap_or_else(|e| panic!("Node {idx} apply block failed: {e:?}"));
        node.sync_rpc_context();
    }

    // 7. VERIFIKASI IDENTITAS KONSENSUS: Seluruh 4 simpul memiliki State Root 100% identik
    let expected_state_root = cluster.get_node(0).ledger.lock().unwrap().latest_block().header.state_root;
    let expected_block_hash = finalized_block.hash();

    for idx in 0..4 {
        let node = cluster.get_node(idx);
        let ledger = node.ledger.lock().unwrap();
        assert_eq!(ledger.latest_height(), 1, "Node {idx} height must be 1");
        assert_eq!(
            ledger.latest_block().hash(),
            expected_block_hash,
            "Node {idx} block hash mismatch"
        );
        assert_eq!(
            ledger.latest_block().header.state_root,
            expected_state_root,
            "Node {idx} state root mismatch"
        );
    }

    // 8. Kueri via JSON-RPC pada seluruh simpul untuk membuktikan konsistensi data luar
    for (idx, addr) in cluster.rpc_binds.iter().enumerate() {
        let h_resp = rpc_call(addr, "aur_blockHeight", serde_json::json!([])).await;
        assert_eq!(
            h_resp.get("result").and_then(|v| v.as_u64()),
            Some(1),
            "Node {idx} RPC block height must be 1"
        );

        let dev_addr_str = hex::encode(cluster.dev_addr.as_bytes());
        let bal_resp = rpc_call(addr, "aur_getBalance", serde_json::json!([dev_addr_str])).await;
        let bal_hex = bal_resp.get("result").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            !bal_hex.is_empty(),
            "Node {idx} RPC getBalance must return valid balance"
        );
    }

    cluster.shutdown_all().await;
}

// ============================================================================
// SKENARIO 3: ROTASI PROPOSER DETERMINISTIK MULTI-BLOK
// ============================================================================
#[tokio::test]
async fn test_multi_node_proposer_rotation_multi_block() {
    let mut cluster = MultiNodeClusterFixture::start(4).await;

    // Produksi 3 blok berturut-turut dengan rotasi proposer
    for target_height in 1..=3 {
        let prev_hash = cluster.get_node(0).ledger.lock().unwrap().latest_block().hash();
        let proposer_idx = BftEngine::select_proposer(&cluster.validator_set, target_height, 0, &prev_hash) as usize;
        let miner = cluster.val_addrs[proposer_idx];

        // 1. Proposer merakit blok
        let candidate = {
            let node_p = cluster.get_node(proposer_idx);
            let ledger = node_p.ledger.lock().unwrap();
            let mempool = node_p.mempool.lock().unwrap();
            let bft = node_p.bft_engine.lock().unwrap();
            bft.assemble_block_proposal(
                &ledger,
                &mempool,
                0,
                1773533000 + target_height * 10,
                &miner,
                1024 * 1024,
            )
        };
        let block_hash = candidate.hash();

        // 2. Kumpulkan Precommit dari seluruh simpul
        let mut precommits = Vec::new();
        for idx in 0..4 {
            let node = cluster.get_node(idx);
            let precommit = node.bft_engine
                .lock()
                .unwrap()
                .produce_precommit(block_hash, target_height, 0)
                .unwrap();
            precommits.push(precommit);
        }

        // 3. Buat sertifikat
        let cert = cluster.get_node(proposer_idx)
            .bft_engine
            .lock()
            .unwrap()
            .create_commit_certificate(&cluster.validator_set, block_hash, target_height, 0, precommits)
            .unwrap();

        let block = Block::new(candidate.header, candidate.transactions, Some(cert));

        // 4. Komit pada seluruh simpul
        for idx in 0..4 {
            let node = cluster.get_node(idx);
            node.ledger.lock().unwrap().apply_block(block.clone(), &miner).unwrap();
            node.sync_rpc_context();
        }
    }

    // Seluruh 4 simpul berada di H=3 dengan state root identik
    let ref_root = cluster.get_node(0).ledger.lock().unwrap().latest_block().header.state_root;
    for idx in 0..4 {
        let node = cluster.get_node(idx);
        let ledger = node.ledger.lock().unwrap();
        assert_eq!(ledger.latest_height(), 3);
        assert_eq!(ledger.latest_block().header.state_root, ref_root, "Node {idx} desynchronized at H=3");
    }

    cluster.shutdown_all().await;
}

// ============================================================================
// SKENARIO 4: SIMULASI CRASH, TOLERANSI BYZANTINE, DAN CATCHUP RECOVERY
// ============================================================================
#[tokio::test]
async fn test_multi_node_crash_tolerance_and_catchup_recovery() {
    let mut cluster = MultiNodeClusterFixture::start(4).await;

    // Produksi Blok 1 secara normal
    let prev_hash_0 = cluster.get_node(0).ledger.lock().unwrap().latest_block().hash();
    let p1 = BftEngine::select_proposer(&cluster.validator_set, 1, 0, &prev_hash_0) as usize;
    let miner1 = cluster.val_addrs[p1];

    let cand1 = {
        let node_p = cluster.get_node(p1);
        let l = node_p.ledger.lock().unwrap();
        let m = node_p.mempool.lock().unwrap();
        let b = node_p.bft_engine.lock().unwrap();
        b.assemble_block_proposal(&l, &m, 0, 1773533100, &miner1, 1024 * 1024)
    };
    let mut votes1 = Vec::new();
    for idx in 0..4 {
        votes1.push(cluster.get_node(idx).bft_engine.lock().unwrap().produce_precommit(cand1.hash(), 1, 0).unwrap());
    }
    let cert1 = cluster.get_node(p1)
        .bft_engine
        .lock()
        .unwrap()
        .create_commit_certificate(&cluster.validator_set, cand1.hash(), 1, 0, votes1)
        .unwrap();
    let b1 = Block::new(cand1.header, cand1.transactions, Some(cert1));

    for idx in 0..4 {
        let n = cluster.get_node(idx);
        n.ledger.lock().unwrap().apply_block(b1.clone(), &miner1).unwrap();
        n.sync_rpc_context();
    }

    // SIMULASI CRASH: Node 3 berhenti mendadak (abort server dan lepaskan database lock)
    let node3_db_path = cluster.db_paths[3].clone();
    cluster.crash_node(3).await;

    // Node 0, 1, 2 tetap melanjutkan rantai ke Blok 2 (Bobot: 3 * 25 = 75 >= 67 Kuorum)
    let prev_hash_1 = cluster.get_node(0).ledger.lock().unwrap().latest_block().hash();
    let mut round = 0;
    let mut p2 = BftEngine::select_proposer(&cluster.validator_set, 2, round, &prev_hash_1) as usize;
    // Jika proposer terpilih adalah Node 3 yang sedang mati, naikkan round
    while p2 == 3 {
        round += 1;
        p2 = BftEngine::select_proposer(&cluster.validator_set, 2, round, &prev_hash_1) as usize;
    }
    let miner2 = cluster.val_addrs[p2];

    let cand2 = {
        let node_p = cluster.get_node(p2);
        let l = node_p.ledger.lock().unwrap();
        let m = node_p.mempool.lock().unwrap();
        let b = node_p.bft_engine.lock().unwrap();
        b.assemble_block_proposal(&l, &m, round, 1773533200, &miner2, 1024 * 1024)
    };
    // Suara hanya dari simpul online (0, 1, 2)
    let mut votes2 = Vec::new();
    for &idx in &[0usize, 1usize, 2usize] {
        votes2.push(cluster.get_node(idx).bft_engine.lock().unwrap().produce_precommit(cand2.hash(), 2, round).unwrap());
    }
    let cert2 = cluster.get_node(p2)
        .bft_engine
        .lock()
        .unwrap()
        .create_commit_certificate(&cluster.validator_set, cand2.hash(), 2, round, votes2)
        .unwrap();
    let b2 = Block::new(cand2.header, cand2.transactions, Some(cert2));

    for &idx in &[0usize, 1usize, 2usize] {
        let n = cluster.get_node(idx);
        n.ledger.lock().unwrap().apply_block(b2.clone(), &miner2).unwrap();
        n.sync_rpc_context();
    }

    assert_eq!(cluster.get_node(0).ledger.lock().unwrap().latest_height(), 2);
    assert!(cluster.nodes[3].is_none(), "Node 3 is offline");

    // PEMULIHAN (RECOVERY): Buka kembali storage Node 3 dari disk dan lakukan catchup
    {
        let store_recovered: Arc<dyn StateStore> = Arc::new(
            RedbStorageEngine::open_or_create(&node3_db_path).expect("Reopen node 3 redb"),
        );
        let mut recovered_ledger = ChainLedger::from_genesis_with_store(
            cluster.genesis.clone(),
            store_recovered,
        ).expect("Recover node 3 ledger");

        // Verifikasi bahwa Node 3 pulih pada tinggi H=1 dengan state root persisten yang utuh
        assert_eq!(recovered_ledger.latest_height(), 1);

        // Lakukan catchup Blok 2 dari peer
        recovered_ledger.apply_block(b2.clone(), &miner2).expect("Catchup block 2 apply");
        assert_eq!(recovered_ledger.latest_height(), 2);

        // State root Node 3 setelah catchup 100% cocok dengan Node 0
        let expected_h2_root = cluster.get_node(0).ledger.lock().unwrap().latest_block().header.state_root;
        assert_eq!(recovered_ledger.latest_block().header.state_root, expected_h2_root);
    }

    cluster.shutdown_all().await;
}

// ============================================================================
// SKENARIO 5: EKSEKUSI PROSES BINER NYATA /bin/aurion VIA CLI SUBCOMMANDS
// ============================================================================
#[test]
fn test_multi_node_real_binary_cli_execution() {
    let bin_path = env!("CARGO_BIN_EXE_aurion");

    // 1. Eksekusi /bin/aurion --version
    let output = Command::new(bin_path)
        .arg("--version")
        .output()
        .expect("Failed to execute /bin/aurion --version");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_ver = env!("CARGO_PKG_VERSION");
    assert!(
        stdout.contains(expected_ver) && stdout.to_lowercase().contains("aurion"),
        "Binary version string must match"
    );

    // 2. Eksekusi /bin/aurion version --output json (AUR-CLI-007)
    let output_json = Command::new(bin_path)
        .args(["version", "--output", "json"])
        .output()
        .expect("Failed to execute /bin/aurion version -o json");

    assert!(output_json.status.success());
    let parsed_ver: serde_json::Value = serde_json::from_slice(&output_json.stdout)
        .expect("Valid JSON output from version command");
    assert_eq!(parsed_ver.get("version").and_then(|v| v.as_str()), Some(expected_ver));
    assert_eq!(parsed_ver.get("application").and_then(|v| v.as_str()), Some("aurion"));

    // 3. Eksekusi /bin/aurion genesis inspect --output json
    let output_genesis = Command::new(bin_path)
        .args(["genesis", "inspect", "--output", "json"])
        .output()
        .expect("Failed to execute /bin/aurion genesis inspect");

    assert!(output_genesis.status.success());
    let parsed_gen: serde_json::Value = serde_json::from_slice(&output_genesis.stdout)
        .expect("Valid JSON output from genesis command");
    assert_eq!(parsed_gen.get("chain_id").and_then(|v| v.as_u64()), Some(1001));
    assert!(parsed_gen.get("genesis_block_hash").is_some());

    // 4. Eksekusi /bin/aurion block latest pada database persisten terpisah
    let tmp_dir = TempDir::new().expect("Create tempdir");
    let db_path = tmp_dir.path().join("cli_test_node.redb");
    let db_path_str = db_path.to_str().unwrap().to_string();

    // Buat database dengan blok genesis menggunakan RedbStorageEngine
    {
        let store = RedbStorageEngine::open_or_create(&db_path).expect("Create db");
        let creator = Address::from_bytes([9u8; 32]);
        let dev = Address::from_bytes([8u8; 32]);
        let genesis = build_genesis(creator, dev, Vec::new());
        let _ = ChainLedger::from_genesis_with_store(genesis, Arc::new(store)).unwrap();
    }

    let output_block = Command::new(bin_path)
        .args(["block", "latest", "--db-path", &db_path_str, "--output", "json"])
        .output()
        .expect("Failed to execute /bin/aurion block latest");

    assert!(output_block.status.success());
    let parsed_block: serde_json::Value = serde_json::from_slice(&output_block.stdout)
        .expect("Valid JSON output from block command");
    assert_eq!(parsed_block.get("height").and_then(|v| v.as_u64()), Some(0));
    assert!(parsed_block.get("hash").is_some());
    assert!(parsed_block.get("state_root").is_some());
}
