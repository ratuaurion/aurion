//! Suite Pengujian Integrasi Otomatis Genesis Ceremony (PRD-015).
//!
//! Menguji pembentukan deterministik, atestasi multi-pihak Ed25519 (RFC 8032),
//! proteksi pemalsuan, ambang batas kuorum BFT (>2/3 = 666.667), komitmen database redb,
//! dan antarmuka CLI tunggal /bin/aurion.

use aurion::cli::command::CliCommand;
use aurion::cli::dispatcher::dispatch;
use aurion::cli::output::OutputFormat;
use aurion::consensus::certificate::ValidatorEntry;
use aurion::core::Address;
use aurion::genesis::builder::build_genesis;
use aurion::genesis::ceremony::{
    CanonicalCeremonyKeypairs, CeremonyError, CeremonyRole, CeremonyTranscript,
    CEREMONY_QUORUM_THRESHOLD,
};
use aurion::statemachine::state::chain::ChainLedger;
use aurion::storage::{RedbStorageEngine, StateStore};
use tempfile::tempdir;

#[test]
fn test_ceremony_deterministic_genesis_generation() {
    let keys1 = CanonicalCeremonyKeypairs::new_deterministic();
    let transcript1 = CeremonyTranscript::build_and_seal(&keys1).expect("Seal 1 failed");

    let keys2 = CanonicalCeremonyKeypairs::new_deterministic();
    let transcript2 = CeremonyTranscript::build_and_seal(&keys2).expect("Seal 2 failed");

    // Block Hash, State Root, dan Ceremony Hash wajib 100% identik antar eksekusi terpisah
    assert_eq!(transcript1.genesis_block_hash, transcript2.genesis_block_hash);
    assert_eq!(transcript1.state_root, transcript2.state_root);
    assert_eq!(transcript1.ceremony_hash, transcript2.ceremony_hash);
    assert_eq!(transcript1.chain_id, 1001);
    assert_eq!(transcript1.timestamp, 1773532800);
}

#[test]
fn test_ceremony_full_attestation_and_signature_verification() {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let transcript = CeremonyTranscript::build_and_seal(&keys).expect("Seal failed");

    let report = transcript.verify().expect("Verification should succeed");

    assert_eq!(report.overall_status, "VERIFIED_CANONICAL");
    assert_eq!(report.total_attestations, 6);
    assert_eq!(report.attested_validator_power, 1_000_000);
    assert_eq!(report.quorum_threshold, CEREMONY_QUORUM_THRESHOLD);
    assert!(report.quorum_status.contains("PASSED"));
    assert!(report.monetary_audit_status.contains("PASSED"));
}

#[test]
fn test_ceremony_tamper_detection_altered_block_hash() {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let mut transcript = CeremonyTranscript::build_and_seal(&keys).expect("Seal failed");

    // Palsukan hash blok genesis
    transcript.genesis_block_hash = "00".repeat(32);

    let err = transcript.verify().expect_err("Must fail on altered block hash");
    match err {
        CeremonyError::GenesisHashMismatch { .. } => {}
        other => panic!("Expected GenesisHashMismatch, got {other:?}"),
    }
}

#[test]
fn test_ceremony_tamper_detection_altered_state_root() {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let mut transcript = CeremonyTranscript::build_and_seal(&keys).expect("Seal failed");

    // Palsukan state root
    transcript.state_root = "ff".repeat(32);

    let err = transcript.verify().expect_err("Must fail on altered state root");
    match err {
        CeremonyError::StateRootMismatch { .. } => {}
        other => panic!("Expected StateRootMismatch, got {other:?}"),
    }
}

#[test]
fn test_ceremony_tamper_detection_forged_signature() {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let mut transcript = CeremonyTranscript::build_and_seal(&keys).expect("Seal failed");

    // Mutasi 1 byte pada signature Creator
    let mut sig_bytes = hex::decode(&transcript.attestations[0].signature_hex).unwrap();
    sig_bytes[0] ^= 0x01;
    transcript.attestations[0].signature_hex = hex::encode(sig_bytes);

    // Recompute transcript hash agar lolos check hash, tapi gagal pada ed25519 verify
    transcript.ceremony_hash = transcript.compute_transcript_hash();

    let err = transcript.verify().expect_err("Must fail on forged signature");
    match err {
        CeremonyError::InvalidSignature { .. } => {}
        other => panic!("Expected InvalidSignature, got {other:?}"),
    }
}

#[test]
fn test_ceremony_validator_quorum_threshold() {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let mut transcript = CeremonyTranscript::build_and_seal(&keys).expect("Seal failed");

    // Hapus 2 atestasi validator (Validator 3 dan Validator 4), menyisakan 500.000 bobot (< 666.667)
    transcript.attestations.retain(|a| {
        a.role != CeremonyRole::Validator(3) && a.role != CeremonyRole::Validator(4)
    });
    transcript.ceremony_hash = transcript.compute_transcript_hash();

    let err = transcript.verify().expect_err("Must fail when quorum is below 666,667");
    match err {
        CeremonyError::QuorumNotAchieved { attested, required } => {
            assert_eq!(attested, 500_000);
            assert_eq!(required, CEREMONY_QUORUM_THRESHOLD);
        }
        other => panic!("Expected QuorumNotAchieved, got {other:?}"),
    }
}

#[test]
fn test_ceremony_monetary_conservation_invariants() {
    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let mut transcript = CeremonyTranscript::build_and_seal(&keys).expect("Seal failed");

    // Uji pelanggaran hard cap
    transcript.hard_cap_aur = 100_000_000;
    assert!(matches!(
        transcript.verify(),
        Err(CeremonyError::MonetaryInvariantViolation { .. })
    ));

    // Uji pelanggaran alokasi 35% genesis
    transcript.hard_cap_aur = 66_000_000;
    transcript.initial_supply_aur = 25_000_000;
    assert!(matches!(
        transcript.verify(),
        Err(CeremonyError::MonetaryInvariantViolation { .. })
    ));
}

#[test]
fn test_ceremony_redb_storage_initialization() {
    let dir = tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("ceremony_genesis.redb");

    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let transcript = CeremonyTranscript::build_and_seal(&keys).expect("Seal failed");

    let creator_addr = Address::from_bytes(hex::decode(&transcript.creator_address_hex).unwrap().try_into().unwrap());
    let dev_addr = Address::from_bytes(hex::decode(&transcript.developer_address_hex).unwrap().try_into().unwrap());

    let val_entries: Vec<ValidatorEntry> = transcript
        .participants
        .iter()
        .filter_map(|p| {
            if let CeremonyRole::Validator(_) = p.role {
                Some(ValidatorEntry {
                    validator_id: Address::from_bytes(hex::decode(&p.address_hex).unwrap().try_into().unwrap()),
                    consensus_pubkey: hex::decode(&p.public_key_hex).unwrap().try_into().unwrap(),
                    voting_weight: p.voting_weight,
                })
            } else {
                None
            }
        })
        .collect();

    let genesis = build_genesis(creator_addr, dev_addr, val_entries);

    // Buka RedbStorageEngine fisik baru
    let store = std::sync::Arc::new(RedbStorageEngine::open_or_create(&db_path).expect("Failed to open redb"));

    // Inisialisasi ChainLedger dari Genesis
    let ledger = ChainLedger::from_genesis_with_store(genesis.clone(), store.clone())
        .expect("ChainLedger genesis init failed");

    assert_eq!(ledger.latest_height(), 0);
    assert_eq!(store.get_latest_height().unwrap(), Some(0));

    // Verifikasi crash-recovery: Buka kembali store dan bangun ledger baru
    drop(ledger);
    drop(store);

    let store_reopened = std::sync::Arc::new(RedbStorageEngine::open_or_create(&db_path).expect("Reopen failed"));
    let ledger_recovered = ChainLedger::from_genesis_with_store(genesis, store_reopened.clone())
        .expect("Ledger recovery failed");

    assert_eq!(ledger_recovered.latest_height(), 0);
    assert_eq!(ledger_recovered.compute_current_state_root().to_hex(), transcript.state_root);
}

#[tokio::test]
async fn test_cli_genesis_ceremony_subcommands() {
    // 1. aurion genesis ceremony run (Text)
    let res = dispatch(
        CliCommand::Genesis(vec!["ceremony".to_string(), "run".to_string()]),
        OutputFormat::Text,
    )
    .await;
    assert!(res.is_ok());

    // 2. aurion genesis ceremony run (JSON)
    let res = dispatch(
        CliCommand::Genesis(vec!["ceremony".to_string(), "run".to_string()]),
        OutputFormat::Json,
    )
    .await;
    assert!(res.is_ok());

    // 3. aurion genesis ceremony verify (Text)
    let res = dispatch(
        CliCommand::Genesis(vec!["ceremony".to_string(), "verify".to_string()]),
        OutputFormat::Text,
    )
    .await;
    assert!(res.is_ok());

    // 4. aurion genesis ceremony verify (JSON)
    let res = dispatch(
        CliCommand::Genesis(vec!["ceremony".to_string(), "verify".to_string()]),
        OutputFormat::Json,
    )
    .await;
    assert!(res.is_ok());

    // 5. aurion genesis ceremony inspect
    let res = dispatch(
        CliCommand::Genesis(vec!["ceremony".to_string(), "inspect".to_string()]),
        OutputFormat::Text,
    )
    .await;
    assert!(res.is_ok());
}
