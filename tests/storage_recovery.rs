#![forbid(unsafe_code)]

//! Test Integrasi Storage Persistence & Crash Recovery Aurion berbasis `redb`.
//! Menguji ACID multi-table atomic commit, restart simpul, dan verifikasi state root dari disk.

use std::sync::Arc;
use tempfile::NamedTempFile;

use aurion::consensus::block::Block;
use aurion::consensus::certificate::{ValidatorEntry, ValidatorSet};
use aurion::consensus::engine::BftEngine;
use aurion::consensus::vote::{Vote, PHASE_PRECOMMIT, PHASE_PREVOTE};
use aurion::core::{Quantum, Signature};
use aurion::crypto::{derive_address_from_pubkey, Keypair};
use aurion::genesis::builder::build_genesis;
use aurion::mempool::MempoolEngine;
use aurion::state::ChainLedger;
use aurion::storage::{RedbStorageEngine, StateStore};
use aurion::transaction::types::{Transaction, TxType};

#[test]
fn test_redb_atomic_commit_and_crash_recovery_deterministic() {
    // 1. Buat berkas database sementara untuk redb
    let tmp_file = NamedTempFile::new().expect("Failed to create temporary file for redb");
    let db_path = tmp_file.path().to_path_buf();

    let creator_key = Keypair::generate();
    let creator_addr = derive_address_from_pubkey(&creator_key.public_key_bytes());
    let recipient_key = Keypair::generate();
    let recipient_addr = derive_address_from_pubkey(&recipient_key.public_key_bytes());

    let val1 = Keypair::generate();
    let val2 = Keypair::generate();
    let val3 = Keypair::generate();
    let val4 = Keypair::generate();
    let val_keys = [val1.clone(), val2.clone(), val3.clone(), val4.clone()];

    let validator_entries = vec![
        ValidatorEntry {
            validator_id: derive_address_from_pubkey(&val1.public_key_bytes()),
            consensus_pubkey: val1.public_key_bytes(),
            voting_weight: 25,
        },
        ValidatorEntry {
            validator_id: derive_address_from_pubkey(&val2.public_key_bytes()),
            consensus_pubkey: val2.public_key_bytes(),
            voting_weight: 25,
        },
        ValidatorEntry {
            validator_id: derive_address_from_pubkey(&val3.public_key_bytes()),
            consensus_pubkey: val3.public_key_bytes(),
            voting_weight: 25,
        },
        ValidatorEntry {
            validator_id: derive_address_from_pubkey(&val4.public_key_bytes()),
            consensus_pubkey: val4.public_key_bytes(),
            voting_weight: 25,
        },
    ];

    let validator_set = ValidatorSet::new(validator_entries.clone());

    let genesis = build_genesis(creator_addr, recipient_addr, validator_entries);
    let block1_hash;
    let expected_state_root_h1;
    let expected_creator_balance;
    let expected_recipient_balance;

    // --- SESI 1: Jalankan node pertama, buat blok H=1, komit ke redb ---
    {
        let redb_engine = RedbStorageEngine::open_or_create(&db_path)
            .expect("Failed to open or create redb database");
        let store: Arc<dyn StateStore> = Arc::new(redb_engine);

        let mut ledger = ChainLedger::from_genesis_with_store(genesis.clone(), Arc::clone(&store))
            .expect("Failed to initialize ledger with store");

        assert_eq!(ledger.latest_height(), 0);
        let genesis_creator_bal = ledger.get_balance(&creator_addr);
        assert_eq!(genesis_creator_bal, Quantum::from_aur(19_800_000).unwrap());

        // Buat transaksi transfer 500 AUR dari Creator ke Recipient
        let amount = Quantum::from_aur(500).unwrap();
        let fee = Quantum::from_aur(1).unwrap();
        let mut tx = Transaction {
            version: 1,
            chain_id: 1001,
            tx_type: TxType::Transfer,
            flags: 0,
            sender: creator_addr,
            recipient: recipient_addr,
            nonce: 0,
            amount,
            fee,
            valid_until: 1800000000,
            payload: Vec::new(),
            signature: Signature::from_bytes([0u8; 64]),
        };
        let sig = creator_key.sign(&tx.signing_preimage());
        tx.signature = sig;

        let mut mempool = MempoolEngine::new(256 * 1024, 3600);
        mempool
            .submit_transaction(
                tx.clone(),
                &creator_key.public_key_bytes(),
                1773532850,
                ledger.get_account(&creator_addr).unwrap(),
            )
            .expect("Mempool submit failed");

        let bft = BftEngine::new(Some(val1.clone()), Some(0));
        let candidate_block = bft.assemble_block_proposal(
            &ledger,
            &mempool,
            0,
            1773532900,
            &creator_addr,
            1024 * 1024,
        );

        let block_hash = candidate_block.hash();

        // Validator 0, 1, 2 memberikan suara (3 * 25 = 75 >= 67 Kuorum)
        let mut precommits = Vec::new();
        for (idx, key) in val_keys.iter().enumerate().take(3) {
            let prevote = Vote::new_signed(key, PHASE_PREVOTE, 1, 0, block_hash, idx as u32)
                .expect("Prevote failed");
            prevote.verify(&validator_set).expect("Prevote verify failed");

            let precommit = Vote::new_signed(key, PHASE_PRECOMMIT, 1, 0, block_hash, idx as u32)
                .expect("Precommit failed");
            precommit.verify(&validator_set).expect("Precommit verify failed");
            precommits.push(precommit);
        }

        let cert = bft
            .create_commit_certificate(&validator_set, block_hash, 1, 0, precommits)
            .expect("Certificate should reach quorum > 2/3");

        let block = Block::new(candidate_block.header, candidate_block.transactions, Some(cert));
        block1_hash = block.hash();
        expected_state_root_h1 = block.header.state_root;

        // Terapkan blok ke ledger -> otomatis disimpan ke redb secara atomik
        ledger
            .apply_block(block, &creator_addr)
            .expect("Block apply should succeed");

        assert_eq!(ledger.latest_height(), 1);
        expected_creator_balance = ledger.get_balance(&creator_addr);
        expected_recipient_balance = ledger.get_balance(&recipient_addr);

        // Pastikan balance berubah sesuai transaksi
        assert!(expected_creator_balance < genesis_creator_bal);
        assert!(expected_recipient_balance > Quantum::ZERO);
    }
    // Sesi 1 berakhir di sini. Seluruh objek di memori dihancurkan (simulasi crash / restart node).

    // --- SESI 2: SIMULASI RESTART NODE DARI DISK ---
    {
        // Buka kembali database redb dari disk fisik yang sama
        let redb_engine = RedbStorageEngine::open_or_create(&db_path)
            .expect("Failed to re-open redb database");
        let store: Arc<dyn StateStore> = Arc::new(redb_engine);

        // Inisialisasi ChainLedger baru mengarah ke redb yang sama
        // ChainLedger harus otomatis mendeteksi blok yang tersimpan, melakukan recovery,
        // dan memverifikasi kesesuaian akar state SMT!
        let recovered_ledger = ChainLedger::from_genesis_with_store(genesis, Arc::clone(&store))
            .expect("Ledger recovery from disk must succeed seamlessly");

        // Verifikasi hasil pemulihan (Crash Recovery Verification)
        assert_eq!(recovered_ledger.latest_height(), 1, "Height harus pulih ke 1");
        assert_eq!(
            recovered_ledger.latest_block().hash(),
            block1_hash,
            "Hash blok H=1 harus identik dengan sebelum crash"
        );
        assert_eq!(
            recovered_ledger.compute_current_state_root(),
            expected_state_root_h1,
            "Akar state yang direkonstruksi dari disk harus cocok 100% dengan header blok H=1"
        );
        assert_eq!(
            recovered_ledger.get_balance(&creator_addr),
            expected_creator_balance,
            "Saldo pengirim harus pulih persis sesuai saldo setelah eksekusi blok"
        );
        assert_eq!(
            recovered_ledger.get_balance(&recipient_addr),
            expected_recipient_balance,
            "Saldo penerima harus pulih persis sesuai saldo setelah eksekusi blok"
        );
        assert_eq!(
            recovered_ledger.get_nonce(&creator_addr),
            1,
            "Nonce pengirim harus pulih ke 1"
        );
    }
}
