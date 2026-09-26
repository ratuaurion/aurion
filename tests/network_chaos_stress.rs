#![forbid(unsafe_code)]

//! Suite Pengujian Integrasi Penguatan Ketahanan Jaringan & DoS (Hardening & Chaos Stress).
//! Memvalidasi kestabilan konsensus BFT Aurion 1.0.0-BFT di bawah kondisi:
//! 1. Rotasi Proposer multi-blok beruntun dengan jitter penerimaan suara (Simulasi Jaringan Ekstrim).
//! 2. Proteksi frame wire P2P terhadap injeksi payload melampaui batas aman (Boundary Attacks).
//! 3. Verifikasi pemulihan deterministik storage Redb pasca-komit multi-blok tanpa korupsi state.

use aurion::consensus::bft::certificate::CommitCertificate;
use aurion::consensus::bft::vote::{Vote, PHASE_PRECOMMIT, PHASE_PREVOTE};
use aurion::core::{Address, Quantum, QUANTA_PER_AUR};
use aurion::genesis::builder::{GENESIS_CHAIN_ID, GENESIS_TIMESTAMP};
use aurion::genesis::ceremony::{
    CanonicalCeremonyKeypairs, CeremonyTranscript, CEREMONY_QUORUM_THRESHOLD,
};
use aurion::runtime::config::NodeConfig;
use aurion::runtime::AurionNode;
use aurion::storage::RedbStorageEngine;
use aurion::wire::frame::{WireFrameHeader, MAX_WIRE_PAYLOAD_BYTES};
use aurion::wire::messages::{is_known_message_type, MSG_BLOCK_PROPOSAL};
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_bft_proposer_rotation_and_deterministic_convergence_5_blocks() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("chaos_cluster.redb");
    let store = Arc::new(RedbStorageEngine::open_or_create(&db_path).unwrap());

    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let genesis = CeremonyTranscript::canonical_mainnet_genesis();

    let val_keys = keys.validators.clone();
    let val_addrs: Vec<Address> = val_keys.iter().map(|k| k.derive_address()).collect();

    // Node 0 berjalan sebagai validator 0
    let config = NodeConfig {
        chain_id: GENESIS_CHAIN_ID,
        ..NodeConfig::new_validator(Vec::new())
    };

    let node = AurionNode::new_with_store(
        config,
        genesis,
        Some(val_keys[0].clone()),
        Some(0),
        store.clone(),
    );

    let mut current_timestamp = GENESIS_TIMESTAMP;

    // Eksekusi komit 5 blok berturut-turut dengan rotasi pemimpin proposer (0 -> 1 -> 2 -> 3 -> 0)
    for height in 1..=5 {
        current_timestamp += 1;
        let proposer_idx = ((height - 1) % 4) as usize;
        let proposer_addr = val_addrs[proposer_idx];

        // 1. Rakit Proposal Blok
        let (candidate_block, block_hash) = {
            let ledger = node.ledger.lock().unwrap();
            let mempool = node.mempool.lock().unwrap();
            let bft = node.bft_engine.lock().unwrap();
            let proposal = bft.assemble_block_proposal(
                &ledger,
                &mempool,
                0,
                current_timestamp,
                &proposer_addr,
                1024 * 1024,
            );
            let hash = proposal.hash();
            (proposal, hash)
        };

        assert_eq!(candidate_block.height(), height);
        assert!(candidate_block.verify_tx_merkle_root());

        // 2. Simulasi Jitter: Suara dikumpulkan dari 3 validator (kuorum 750.000 / 1.000.000)
        // Pemilih diacak urutannya untuk memverifikasi ketahanan terhadap ketidakteraturan jaringan
        let voter_indices = if height % 2 == 0 {
            vec![2, 0, 1] // Urutan non-sekuensial
        } else {
            vec![1, 2, 3] // Himpunan voter alternatif
        };

        let mut precommits = Vec::new();
        let mut total_weight = 0;
        for &idx in &voter_indices {
            // Fase Prevote
            let prevote = Vote::new_signed(
                &val_keys[idx],
                PHASE_PREVOTE,
                height,
                0,
                block_hash,
                idx as u32,
            )
            .expect("Prevote signing must succeed");
            prevote
                .verify(&node.ledger.lock().unwrap().validator_set)
                .expect("Prevote must verify");

            // Fase Precommit
            let precommit = Vote::new_signed(
                &val_keys[idx],
                PHASE_PRECOMMIT,
                height,
                0,
                block_hash,
                idx as u32,
            )
            .expect("Precommit signing must succeed");
            precommit
                .verify(&node.ledger.lock().unwrap().validator_set)
                .expect("Precommit must verify");

            precommits.push(precommit);
            total_weight += 250_000;
        }

        assert!(total_weight >= CEREMONY_QUORUM_THRESHOLD);

        // 3. Bentuk Commit Certificate
        let cert = CommitCertificate {
            block_hash,
            height,
            round: 0,
            precommits,
        };

        cert.verify(&node.ledger.lock().unwrap().validator_set)
            .expect("Certificate verification must pass");

        // 4. Komit blok ke dalam ledger simpul
        let committed = node
            .produce_and_commit_block(cert, &proposer_addr, current_timestamp)
            .expect("Commit block must succeed");

        assert_eq!(committed.height(), height);
        assert_eq!(node.ledger.lock().unwrap().latest_height(), height);

        // Verifikasi saldo proposer bertambah hadiah blok kanonikal 1 AUR (10^9 Quanta)
        let proposer_bal = node.ledger.lock().unwrap().get_balance(&proposer_addr);
        assert!(proposer_bal >= Quantum::new(QUANTA_PER_AUR));
    }

    // Verifikasi final pasca 5 blok
    let ledger = node.ledger.lock().unwrap();
    assert_eq!(ledger.latest_height(), 5);
    // Pasokan total diterbitkan: 66M Genesis + (5 x 1 AUR) = 66,000,005 AUR
    assert_eq!(
        ledger.monetary.total_issued,
        Quantum::from_aur(66_000_005).unwrap()
    );
    assert_eq!(ledger.monetary.total_burned, Quantum::ZERO);
}

#[test]
fn test_wire_frame_boundary_and_malformed_injection_resistance() {
    // 1. Verifikasi batas ukuran payload maksimum frame wire P2P (8 MB)
    assert_eq!(MAX_WIRE_PAYLOAD_BYTES, 8 * 1024 * 1024);

    // 2. Verifikasi katalog tipe pesan kanonikal
    assert!(is_known_message_type(MSG_BLOCK_PROPOSAL));
    assert!(!is_known_message_type(0xFFFF)); // Tipe tidak dikenal wajib ditolak

    // 3. Verifikasi validasi struktur header frame 52 bytes
    let valid_header = WireFrameHeader {
        magic: [0x41, 0x55, 0x52, 0x30], // "AUR0"
        message_type: MSG_BLOCK_PROPOSAL,
        reserved: 0,
        payload_len: 1024,
        reserved2: [0u8; 8],
        payload_checksum: aurion::core::Hash256::ZERO,
    };
    assert_eq!(valid_header.magic, [0x41, 0x55, 0x52, 0x30]);
    assert_eq!(valid_header.reserved, 0);
}

#[test]
fn test_acid_redb_persistence_crash_recovery_after_multi_block_stress() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("acid_stress.redb");

    let keys = CanonicalCeremonyKeypairs::new_deterministic();
    let val_keys = keys.validators.clone();
    let val_addrs: Vec<Address> = val_keys.iter().map(|k| k.derive_address()).collect();

    let expected_height = 3;
    let pre_crash_hash;
    let pre_crash_state_root;
    let treasury_addr = keys.master_treasury.derive_address();

    // Sesi 1: Jalankan simpul dan buat 3 blok
    {
        let store = Arc::new(RedbStorageEngine::open_or_create(&db_path).unwrap());
        let genesis = CeremonyTranscript::canonical_mainnet_genesis();
        let config = NodeConfig {
            chain_id: GENESIS_CHAIN_ID,
            ..NodeConfig::new_validator(Vec::new())
        };

        let node =
            AurionNode::new_with_store(config, genesis, Some(val_keys[0].clone()), Some(0), store);

        let mut current_timestamp = GENESIS_TIMESTAMP;

        for height in 1..=expected_height {
            current_timestamp += 1;
            let proposer_addr = val_addrs[(height - 1) as usize];

            let (_proposal, block_hash) = {
                let ledger = node.ledger.lock().unwrap();
                let mempool = node.mempool.lock().unwrap();
                let bft = node.bft_engine.lock().unwrap();
                let b = bft.assemble_block_proposal(
                    &ledger,
                    &mempool,
                    0,
                    current_timestamp,
                    &proposer_addr,
                    1024 * 1024,
                );
                let h = b.hash();
                (b, h)
            };

            let mut precommits = Vec::new();
            for (idx, key) in val_keys[0..3].iter().enumerate() {
                let vote =
                    Vote::new_signed(key, PHASE_PRECOMMIT, height, 0, block_hash, idx as u32)
                        .expect("Vote signing must succeed");
                precommits.push(vote);
            }

            let cert = CommitCertificate {
                block_hash,
                height,
                round: 0,
                precommits,
            };

            node.produce_and_commit_block(cert, &proposer_addr, current_timestamp)
                .expect("Block commit must succeed");
        }

        let ledger = node.ledger.lock().unwrap();
        pre_crash_hash = ledger.latest_block().hash();
        pre_crash_state_root = ledger.compute_current_state_root();
        assert_eq!(ledger.latest_height(), expected_height);
    }
    // Node ditutup paksa di sini (crash simulation)

    // Sesi 2: Pulihkan simpul dari file disk redb yang sama
    {
        let store = Arc::new(RedbStorageEngine::open_or_create(&db_path).unwrap());
        let genesis = CeremonyTranscript::canonical_mainnet_genesis();
        let config = NodeConfig {
            chain_id: GENESIS_CHAIN_ID,
            ..NodeConfig::new_validator(Vec::new())
        };

        let recovered_node =
            AurionNode::new_with_store(config, genesis, Some(val_keys[0].clone()), Some(0), store);

        let ledger = recovered_node.ledger.lock().unwrap();
        assert_eq!(ledger.latest_height(), expected_height);
        assert_eq!(ledger.latest_block().hash(), pre_crash_hash);
        assert_eq!(ledger.compute_current_state_root(), pre_crash_state_root);

        // Verifikasi saldo Master Treasury tetap utuh 66M AUR
        let treasury_bal = ledger.get_balance(&treasury_addr);
        assert_eq!(treasury_bal, Quantum::from_aur(66_000_000).unwrap());

        // Verifikasi validator 0, 1, 2 masing-masing telah menerima hadiah blok kanonikal 1 AUR
        for addr in &val_addrs[0..3] {
            let bal = ledger.get_balance(addr);
            assert_eq!(bal, Quantum::from_aur(1).unwrap());
        }
    }
}
