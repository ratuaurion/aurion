#![forbid(unsafe_code)]

//! Suite Pengujian Integrasi Otomatis: Post-Mainnet Operations, Observability & Governance (PRD-017).
//! Memvalidasi:
//! 1. Endpoint & Format Prometheus OpenMetrics deterministik (Zero-Float).
//! 2. Pemeriksaan Kesehatan Berlapis (Shallow Liveness & Deep Readiness).
//! 3. Tata Kelola On-Chain (Bit-Signaling, Ambang Batas 80%, & Aktivasi Versi).
//! 4. Pemulihan Bencana & Pemutus Sirkuit Darurat (Circuit Breaker & Snapshot Restore).
//! 5. CLI Dispatcher Subcommands (`metrics`, `governance`, `recovery`).

use aurion::cli::{run_cli, CliCommand, OutputFormat};
use aurion::consensus::bft::governance::{GovernanceEngine, ProposalStatus, UpgradeProposal};
use aurion::core::{Address, Quantum};
use aurion::platform::runtime::recovery::{CircuitBreaker, DisasterRecoveryManager};
use aurion::platform::telemetry::health::HealthReporter;
use aurion::platform::telemetry::metrics::MetricsRegistry;
use aurion::runtime::config::NodeRole;
use aurion::state::account::Account;
use aurion::state::snapshot::{StateSnapshot, SNAPSHOT_MAGIC, SNAPSHOT_VERSION};
use aurion::storage::{RedbStorageEngine, StateStore};
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn test_prometheus_metrics_endpoint_openmetrics_format() {
    let registry = MetricsRegistry::new(1001);

    // Rekam mutasi metrik
    registry.record_block(120, 1, 45, 900_000_000, 750);
    registry.set_mempool_size(15);
    registry.set_connected_peers(8);

    assert_eq!(registry.block_height.load(Ordering::SeqCst), 120);
    assert_eq!(registry.bft_round.load(Ordering::SeqCst), 1);
    assert_eq!(
        registry.transactions_processed_total.load(Ordering::SeqCst),
        45
    );
    assert_eq!(*registry.burned_quanta_total.lock().unwrap(), 900_000_000);
    assert_eq!(registry.bft_finality_latency_ms.load(Ordering::SeqCst), 750);
    assert_eq!(registry.mempool_size.load(Ordering::SeqCst), 15);
    assert_eq!(registry.connected_peers.load(Ordering::SeqCst), 8);

    // Render OpenMetrics format
    let text = registry.render_openmetrics();

    // Verifikasi kepatuhan OpenMetrics format
    assert!(text.contains("# HELP aurion_block_height"));
    assert!(text.contains("# TYPE aurion_block_height gauge"));
    assert!(text.contains("aurion_block_height{chain_id=\"1001\"} 120"));

    assert!(text.contains("# HELP aurion_bft_round"));
    assert!(text.contains("# TYPE aurion_bft_round gauge"));
    assert!(text.contains("aurion_bft_round{chain_id=\"1001\"} 1"));

    assert!(text.contains("# HELP aurion_transactions_processed_total"));
    assert!(text.contains("# TYPE aurion_transactions_processed_total counter"));
    assert!(text.contains("aurion_transactions_processed_total{chain_id=\"1001\"} 45"));

    assert!(text.contains("# HELP aurion_burned_quanta_total"));
    assert!(text.contains("# TYPE aurion_burned_quanta_total counter"));
    assert!(text.contains("aurion_burned_quanta_total{chain_id=\"1001\"} 900000000"));

    assert!(text.contains("# HELP aurion_bft_finality_latency_ms"));
    assert!(text.contains("# TYPE aurion_bft_finality_latency_ms gauge"));
    assert!(text.contains("aurion_bft_finality_latency_ms{chain_id=\"1001\"} 750"));

    assert!(text.contains("# EOF"));

    // Verifikasi JSON snapshot
    let snapshot = registry.snapshot();
    assert_eq!(snapshot.chain_id, 1001);
    assert_eq!(snapshot.block_height, 120);
    assert_eq!(snapshot.burned_quanta_total, "900000000");
}

#[tokio::test]
async fn test_enhanced_healthz_shallow_and_deep_diagnostics() {
    let reporter = HealthReporter::new(1001, NodeRole::Validator, 3);

    // 1. Shallow Liveness Check
    let shallow = reporter.shallow_check(50, 4, true);
    assert_eq!(shallow.status, "HEALTHY");
    assert_eq!(shallow.service, "aurion-node");
    assert_eq!(shallow.chain_id, 1001);
    assert_eq!(shallow.current_height, 50);
    assert_eq!(shallow.connected_peers, 4);
    assert_eq!(shallow.sync_state, "SYNCED");

    // 2. Deep Readiness Check - Skenario Normal (READY)
    let deep_ok = reporter.deep_check(50, 4, 0, true, true, true);
    assert_eq!(deep_ok.status, "READY");
    assert_eq!(deep_ok.components.len(), 5);
    assert!(deep_ok.components.iter().all(|c| c.healthy));

    // 3. Deep Readiness Check - Skenario Terdegradasi (DEGRADED)
    // Peer count 1 < min_peer_threshold 3
    let deep_degraded = reporter.deep_check(50, 1, 0, true, true, true);
    assert_eq!(deep_degraded.status, "DEGRADED");
    let peer_comp = deep_degraded
        .components
        .iter()
        .find(|c| c.name == "p2p_mesh_connectivity")
        .unwrap();
    assert!(!peer_comp.healthy);
    assert!(peer_comp.message.contains("Insufficient peers"));

    // Storage unhealty
    let deep_storage_fail = reporter.deep_check(50, 4, 0, false, true, true);
    assert_eq!(deep_storage_fail.status, "DEGRADED");
    let storage_comp = deep_storage_fail
        .components
        .iter()
        .find(|c| c.name == "storage_redb_acid")
        .unwrap();
    assert!(!storage_comp.healthy);
}

#[tokio::test]
async fn test_onchain_governance_fork_signaling_and_activation() {
    let mut gov = GovernanceEngine::new(1);

    // Daftarkan proposal peningkatan protokol v2
    // window: blok 10..29 (20 blok), aktivasi di blok 35
    let proposal = UpgradeProposal::new(
        201,
        "Aurion Fast-BFT & Dynamic Fee Market",
        2,
        2, // bit 2
        10,
        20,
        35,
    )
    .unwrap();

    gov.register_proposal(proposal).unwrap();
    assert_eq!(gov.get_status(201), Some(ProposalStatus::Draft));

    // Blok 0..9: Pra-jendela evaluasi
    for h in 0..10 {
        gov.record_block(h, 0);
    }
    assert_eq!(gov.get_status(201), Some(ProposalStatus::Draft));

    // Blok 10..29: Jendela evaluasi (20 blok)
    // Berikan sinyal bit 2 pada 18 dari 20 blok (90% >= 80% ambang batas)
    let signal_mask = 1u32 << 2;
    for h in 10..30 {
        let signal = if h < 28 { signal_mask } else { 0 };
        gov.record_block(h, signal);
        if h == 10 {
            assert_eq!(gov.get_status(201), Some(ProposalStatus::ActiveSignaling));
        }
    }

    // Pada akhir blok 29, proposal terkunci (LockedIn)
    assert_eq!(gov.get_status(201), Some(ProposalStatus::LockedIn));
    assert_eq!(gov.active_protocol_version, 1);

    // Blok 30..34: Menuju ketinggian aktivasi
    for h in 30..35 {
        gov.record_block(h, 0);
    }
    assert_eq!(gov.get_status(201), Some(ProposalStatus::LockedIn));

    // Blok 35: Ketinggian aktivasi tercapai!
    gov.record_block(35, 0);
    assert_eq!(gov.get_status(201), Some(ProposalStatus::Activated));
    assert_eq!(gov.active_protocol_version, 2);

    // Verifikasi rekap proposal
    let summary = gov.list_proposals();
    assert_eq!(summary.len(), 1);
    assert_eq!(summary[0].proposal_id, 201);
    assert_eq!(summary[0].status, ProposalStatus::Activated);
    assert_eq!(summary[0].signaling_blocks, 18);
    assert_eq!(summary[0].support_bps, 9000); // 90.00%
}

#[tokio::test]
async fn test_disaster_recovery_circuit_breaker_and_snapshot_restore() {
    // 1. Uji Circuit Breaker
    let mut cb = CircuitBreaker::new(4);
    assert!(!cb.is_tripped);

    cb.record_round_failure("Timeout 1");
    cb.record_round_failure("Timeout 2");
    cb.record_round_failure("Timeout 3");
    assert!(!cb.is_tripped);

    cb.record_round_failure("Timeout 4");
    assert!(cb.is_tripped);
    assert!(cb
        .trip_reason
        .as_ref()
        .unwrap()
        .contains("failure threshold exceeded"));

    cb.reset();
    assert!(!cb.is_tripped);
    assert_eq!(cb.consecutive_failed_rounds, 0);

    // 2. Uji Snapshot Recovery
    let addr1 = Address::from_bytes([1u8; 32]);
    let addr2 = Address::from_bytes([2u8; 32]);

    let mut accounts = BTreeMap::new();
    accounts.insert(addr1, Account::new(Quantum::new(50_000_000), 0));
    accounts.insert(addr2, Account::new(Quantum::new(75_000_000), 0));

    let hashmap: std::collections::HashMap<Address, Account> =
        accounts.iter().map(|(k, v)| (*k, v.clone())).collect();
    let state_root = aurion::statemachine::state::smt::compute_accounts_state_root(&hashmap);

    let snapshot = StateSnapshot {
        magic: SNAPSHOT_MAGIC,
        version: SNAPSHOT_VERSION,
        chain_id: 1001,
        height: 10,
        epoch: 0,
        block_hash: aurion::core::Hash256::from_bytes([0u8; 32]),
        state_root,
        accounts: accounts.into_iter().collect(),
        certificate: None,
    };

    let temp_dir = std::env::temp_dir();
    let snap_path = temp_dir.join(format!(
        "aurion_test_snap_{}.auss",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    snapshot
        .write_to_file(&snap_path)
        .expect("Failed to write snapshot");

    let db_path = temp_dir.join(format!(
        "aurion_test_recovery_{}.redb",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let store = RedbStorageEngine::open_or_create(&db_path).expect("Failed to create test redb");

    let report = DisasterRecoveryManager::restore_from_snapshot_file(&snap_path, &store)
        .expect("Snapshot restore failed");

    assert!(report.success);
    assert_eq!(report.recovered_height, 10);
    assert_eq!(report.accounts_restored, 2);

    // Verifikasi saldo akun di store
    let acc1 = store.get_account(&addr1).unwrap().unwrap();
    assert_eq!(acc1.balance.as_u128(), 50_000_000);

    let acc2 = store.get_account(&addr2).unwrap().unwrap();
    assert_eq!(acc2.balance.as_u128(), 75_000_000);

    // Cleanup
    let _ = std::fs::remove_file(snap_path);
    let _ = std::fs::remove_file(db_path);
}

#[tokio::test]
async fn test_post_mainnet_cli_dispatchers() {
    // 1. aurion metrics status
    let cmd = CliCommand::Metrics(vec!["status".to_string()]);
    assert!(aurion::cli::dispatcher::dispatch(cmd, OutputFormat::Text)
        .await
        .is_ok());

    let cmd_json = CliCommand::Metrics(vec!["status".to_string()]);
    assert!(
        aurion::cli::dispatcher::dispatch(cmd_json, OutputFormat::Json)
            .await
            .is_ok()
    );

    // 2. aurion metrics export
    let cmd_export = CliCommand::Metrics(vec!["export".to_string()]);
    assert!(
        aurion::cli::dispatcher::dispatch(cmd_export, OutputFormat::Text)
            .await
            .is_ok()
    );

    // 3. aurion governance status
    let cmd_gov = CliCommand::Governance(vec!["status".to_string()]);
    assert!(
        aurion::cli::dispatcher::dispatch(cmd_gov, OutputFormat::Text)
            .await
            .is_ok()
    );

    let cmd_gov_json = CliCommand::Governance(vec!["status".to_string()]);
    assert!(
        aurion::cli::dispatcher::dispatch(cmd_gov_json, OutputFormat::Json)
            .await
            .is_ok()
    );

    // 4. aurion governance signal
    let cmd_signal = CliCommand::Governance(vec![
        "signal".to_string(),
        "--bit".to_string(),
        "2".to_string(),
    ]);
    assert!(
        aurion::cli::dispatcher::dispatch(cmd_signal, OutputFormat::Text)
            .await
            .is_ok()
    );

    // 5. aurion recovery status
    let cmd_rec_status = CliCommand::Recovery(vec!["status".to_string()]);
    assert!(
        aurion::cli::dispatcher::dispatch(cmd_rec_status, OutputFormat::Text)
            .await
            .is_ok()
    );

    let cmd_rec_status_json = CliCommand::Recovery(vec!["status".to_string()]);
    assert!(
        aurion::cli::dispatcher::dispatch(cmd_rec_status_json, OutputFormat::Json)
            .await
            .is_ok()
    );

    // 6. aurion recovery audit
    let cmd_audit = CliCommand::Recovery(vec!["audit".to_string()]);
    assert!(
        aurion::cli::dispatcher::dispatch(cmd_audit, OutputFormat::Text)
            .await
            .is_ok()
    );

    // 7. run_cli entrypoint
    let args = vec![
        "metrics".to_string(),
        "status".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    assert!(run_cli(&args).await.is_ok());
}
