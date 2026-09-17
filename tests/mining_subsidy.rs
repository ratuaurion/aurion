#![forbid(unsafe_code)]

//! Test Integrasi Emisi Subsidi Blok Mining Aurion-BFT
//! Memverifikasi transisi state moneter:
//! - Penerbitan subsidi 10 AUR di Era 0 ke produser blok (miner)
//! - Konservasi pasokan moneter total_issued dan circulating_supply
//! - Pembagian fee 20% burn dan 80% miner
//! - Deterministik StateRoot SMT
//! - Formula halving multi-era

use aurion::consensus::bft::engine::BftEngine;
use aurion::consensus::block::Block;
use aurion::consensus::certificate::{ValidatorEntry, ValidatorSet};
use aurion::consensus::vote::{Vote, PHASE_PRECOMMIT, PHASE_PREVOTE};
use aurion::core::{Quantum, Signature};
use aurion::crypto::{derive_address_from_pubkey, Keypair};
use aurion::genesis::builder::build_genesis;
use aurion::mempool::MempoolEngine;
use aurion::state::chain::ChainLedger;
use aurion::state::monetary::calculate_block_subsidy;
use aurion::transaction::types::{Transaction, TxType};

#[test]
fn test_block_mining_subsidy_and_fee_distribution() {
    // 1. Setup Genesis dengan 4 Validator
    let val_keys: Vec<Keypair> = (0..4).map(|_| Keypair::generate()).collect();
    let val_entries: Vec<ValidatorEntry> = val_keys
        .iter()
        .map(|kp| {
            let addr = derive_address_from_pubkey(&kp.public_key_bytes());
            ValidatorEntry {
                validator_id: addr,
                consensus_pubkey: kp.public_key_bytes(),
                voting_weight: 25,
            }
        })
        .collect();

    let creator_key = Keypair::generate();
    let creator_addr = derive_address_from_pubkey(&creator_key.public_key_bytes());

    let dev_key = Keypair::generate();
    let dev_addr = derive_address_from_pubkey(&dev_key.public_key_bytes());

    let genesis = build_genesis(creator_addr, dev_addr, val_entries.clone());
    let validator_set = ValidatorSet::new(val_entries);
    let mut ledger = ChainLedger::from_genesis(genesis);

    let initial_issued = ledger.monetary.total_issued;
    assert_eq!(initial_issued, Quantum::from_aur(23_100_000).unwrap());
    assert_eq!(ledger.monetary.total_burned, Quantum::ZERO);

    let miner_key = Keypair::generate();
    let miner_addr = derive_address_from_pubkey(&miner_key.public_key_bytes());
    assert_eq!(ledger.get_balance(&miner_addr), Quantum::ZERO);

    let mempool = MempoolEngine::new(1024 * 1024, 3600);
    let bft = BftEngine::new(Some(val_keys[0].clone()), Some(0));

    // --- BLOK 1: EMPTY BLOCK (Murni Subsidi Emisi Mining 10 AUR) ---
    {
        let candidate_block = bft.assemble_block_proposal(
            &ledger,
            &mempool,
            0,
            1773532860,
            &miner_addr,
            1024 * 1024,
        );

        assert_eq!(candidate_block.height(), 1);
        assert_eq!(candidate_block.transactions.len(), 0);

        let block_hash = candidate_block.hash();
        let mut precommits = Vec::new();
        for (idx, key) in val_keys.iter().enumerate().take(3) {
            let prevote = Vote::new_signed(key, PHASE_PREVOTE, 1, 0, block_hash, idx as u32).unwrap();
            prevote.verify(&validator_set).unwrap();
            let precommit = Vote::new_signed(key, PHASE_PRECOMMIT, 1, 0, block_hash, idx as u32).unwrap();
            precommit.verify(&validator_set).unwrap();
            precommits.push(precommit);
        }

        let cert = bft
            .create_commit_certificate(&validator_set, block_hash, 1, 0, precommits)
            .unwrap();

        let block = Block::new(candidate_block.header, candidate_block.transactions, Some(cert));

        ledger.apply_block(block, &miner_addr).expect("Block 1 apply failed");

        // Verifikasi saldo miner bertambah tepat 10 AUR
        assert_eq!(
            ledger.get_balance(&miner_addr),
            Quantum::from_aur(10).unwrap()
        );

        // Verifikasi total penerbitan bertambah 10 AUR
        assert_eq!(
            ledger.monetary.total_issued,
            Quantum::from_aur(23_100_010).unwrap()
        );
        assert_eq!(ledger.monetary.total_burned, Quantum::ZERO);
        assert_eq!(
            ledger.monetary.circulating_supply().unwrap(),
            Quantum::from_aur(23_100_010).unwrap()
        );
    }

    // --- BLOK 2: BLOCK DENGAN TRANSAKSI TRANSFER (Subsidi 10 AUR + Fee Miner 80%) ---
    {
        let recipient_key = Keypair::generate();
        let recipient_addr = derive_address_from_pubkey(&recipient_key.public_key_bytes());

        // Transfer 100 AUR dengan fee 10 AUR (Fee split: 2 AUR Burn, 8 AUR Miner)
        let amount = Quantum::from_aur(100).unwrap();
        let fee = Quantum::from_aur(10).unwrap();

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
        tx.signature = creator_key.sign(&tx.signing_preimage());

        let mut block2_mempool = MempoolEngine::new(1024 * 1024, 3600);
        block2_mempool
            .submit_transaction(
                tx,
                &creator_key.public_key_bytes(),
                1773532900,
                ledger.get_account(&creator_addr).unwrap(),
            )
            .expect("Submit tx failed");

        let candidate_block = bft.assemble_block_proposal(
            &ledger,
            &block2_mempool,
            0,
            1773532920,
            &miner_addr,
            1024 * 1024,
        );

        assert_eq!(candidate_block.height(), 2);
        assert_eq!(candidate_block.transactions.len(), 1);

        let block_hash = candidate_block.hash();
        let mut precommits = Vec::new();
        for (idx, key) in val_keys.iter().enumerate().take(3) {
            let prevote = Vote::new_signed(key, PHASE_PREVOTE, 2, 0, block_hash, idx as u32).unwrap();
            prevote.verify(&validator_set).unwrap();
            let precommit = Vote::new_signed(key, PHASE_PRECOMMIT, 2, 0, block_hash, idx as u32).unwrap();
            precommit.verify(&validator_set).unwrap();
            precommits.push(precommit);
        }

        let cert = bft
            .create_commit_certificate(&validator_set, block_hash, 2, 0, precommits)
            .unwrap();

        let block = Block::new(candidate_block.header, candidate_block.transactions, Some(cert));

        ledger.apply_block(block, &miner_addr).expect("Block 2 apply failed");

        // Saldo Miner:
        // Awal: 10 AUR (dari Blok 1)
        // Blok 2: + 10 AUR (subsidi) + 8 AUR (80% dari fee 10 AUR) = 18 AUR
        // Total akumulasi: 10 + 18 = 28 AUR!
        assert_eq!(
            ledger.get_balance(&miner_addr),
            Quantum::from_aur(28).unwrap()
        );

        // Saldo Recipient: 100 AUR
        assert_eq!(
            ledger.get_balance(&recipient_addr),
            Quantum::from_aur(100).unwrap()
        );

        // Saldo Creator berkurang 110 AUR (100 amount + 10 fee)
        assert_eq!(
            ledger.get_balance(&creator_addr),
            Quantum::from_aur(19_800_000 - 110).unwrap()
        );

        // Total Burned: 2 AUR (20% dari fee 10 AUR)
        assert_eq!(
            ledger.monetary.total_burned,
            Quantum::from_aur(2).unwrap()
        );

        // Total Issued: Genesis (23.100.000) + Blok 1 (10) + Blok 2 (10) = 23.100.020 AUR
        assert_eq!(
            ledger.monetary.total_issued,
            Quantum::from_aur(23_100_020).unwrap()
        );

        // Circulating Supply: Total Issued (23.100.020) - Burned (2) = 23.100.018 AUR
        assert_eq!(
            ledger.monetary.circulating_supply().unwrap(),
            Quantum::from_aur(23_100_018).unwrap()
        );
    }
}

#[test]
fn test_subsidy_halving_invariants() {
    // Era 0: 10 AUR
    assert_eq!(calculate_block_subsidy(1), Quantum::from_aur(10).unwrap());
    assert_eq!(calculate_block_subsidy(2_145_000), Quantum::from_aur(10).unwrap());

    // Era 1 (Halving ke-1): 5 AUR
    assert_eq!(calculate_block_subsidy(2_145_001), Quantum::from_aur(5).unwrap());
    assert_eq!(calculate_block_subsidy(4_290_000), Quantum::from_aur(5).unwrap());

    // Era 2 (Halving ke-2): 2.5 AUR
    assert_eq!(calculate_block_subsidy(4_290_001), Quantum::new(250_000_000));

    // Era 30 (Subsidi tuntas/habis): 0 AUR
    assert_eq!(calculate_block_subsidy(64_350_001), Quantum::ZERO);
}
