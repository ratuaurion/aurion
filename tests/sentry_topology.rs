//! Pengujian Integrasi Arsitektur Sentry Node, Isolasi Validator, dan Health Checks.
//! Memvalidasi Kepatuhan Dokumen 12 (12-OPERATIONAL-RULES.md).

use aurion::runtime::{HealthCheckError, NodeConfig, NodeRole, RuntimeSupervisor};

#[test]
fn test_validator_isolation_mandate_enforcement() {
    let sentry_peer_1 = "tcp/198.51.100.10:9000".to_string();
    let sentry_peer_2 = "tcp/198.51.100.11:9000".to_string();
    let config = NodeConfig::new_validator(vec![sentry_peer_1.clone(), sentry_peer_2.clone()]);

    let supervisor = RuntimeSupervisor::new(config);

    // 1. Koneksi dari Sentry resmi milik sendiri WAJIB diizinkan
    assert!(supervisor.is_peer_allowed(&sentry_peer_1));
    assert!(supervisor.is_peer_allowed(&sentry_peer_2));

    // 2. Koneksi dari IP publik / peer asing WAJIB DITOLAK SEKETIKA
    let unauthorized_ip = "tcp/203.0.113.42:9000";
    assert!(
        !supervisor.is_peer_allowed(unauthorized_ip),
        "Simpul validator konsensus dilarang menerima koneksi dari publik luar!"
    );
}

#[test]
fn test_sentry_node_allows_public_traffic() {
    let config = NodeConfig::new_sentry("0.0.0.0:9000".to_string());
    let supervisor = RuntimeSupervisor::new(config);

    assert_eq!(supervisor.config.role, NodeRole::Sentry);
    // Sentry bertindak sebagai lapisan scrubbing publik
    assert!(supervisor.is_peer_allowed("tcp/203.0.113.99:9000"));
}

#[test]
fn test_shallow_liveness_health_check() {
    let mut supervisor = RuntimeSupervisor::new(NodeConfig::default());

    assert!(!supervisor.check_liveness(), "Saat belum start, liveness wajib false");
    supervisor.start();
    assert!(supervisor.check_liveness(), "Saat running, liveness wajib true (HTTP 200)");
    supervisor.stop();
    assert!(!supervisor.check_liveness(), "Saat stop, liveness wajib false");
}

#[test]
fn test_deep_readiness_health_check() {
    let mut supervisor = RuntimeSupervisor::new(NodeConfig::default());
    supervisor.start();

    // Skenario 1: Peer kurang dari 3 (< min_peer_threshold) -> REJECT
    supervisor.active_peer_count = 2;
    supervisor.sync_lag = 0;
    supervisor.last_state_latency_ms = 10;
    let res = supervisor.check_readiness();
    assert!(matches!(res, Err(HealthCheckError::InsufficientPeers { .. })));

    // Skenario 2: Peer cukup (>= 3), tetapi sedang sinkronisasi ketinggalan blok (> 1) -> REJECT
    supervisor.active_peer_count = 4;
    supervisor.sync_lag = 5;
    let res = supervisor.check_readiness();
    assert!(matches!(res, Err(HealthCheckError::SyncLagTooHigh { .. })));

    // Skenario 3: Peer cukup, sync_lag <= 1, tetapi database latency tinggi (> 50ms) -> REJECT
    supervisor.sync_lag = 0;
    supervisor.last_state_latency_ms = 75; // Melebihi 50ms
    let res = supervisor.check_readiness();
    assert!(matches!(res, Err(HealthCheckError::StateLatencyTooHigh { .. })));

    // Skenario 4: Semua kriteria Dokumen 12 terpenuhi sempurna -> PASS (HTTP 200)
    supervisor.last_state_latency_ms = 25; // <= 50ms
    let res = supervisor.check_readiness();
    assert!(res.is_ok(), "Semua kriteria deep readiness wajib lolos");
}
