#![forbid(unsafe_code)]

//! Test Integrasi Emisi Hadiah Blok BFT Aurion (Insentif Validator)
//! Memverifikasi transisi state moneter:
//! - Penerbitan hadiah blok R = 1 AUR di setiap blok BFT (H >= 1) ke Proposer
//! - Konservasi pasokan moneter total_issued dan circulating_supply
//! - Alokasi fee transaksi 100% ke validator produser blok (0% burn)
//! - Deterministik StateRoot SMT
//! - Emisi tetap kanonikal Aurion-BFT

use aurion::consensus::bft::engine::BftEngine;
use aurion::consensus::block::Block;
use aurion::consensus::certificate::{ValidatorEntry, ValidatorSet};
use aurion::consensus::vote::{Vote, PHASE_PRECOMMIT, PHASE_PREVOTE};
use aurion::core::{Quantum, Signature};
use aurion::crypto::{derive_address_from_pubkey, Keypair};
use aurion::genesis::builder::build_genesis;
use aurion::mempool::MempoolEngine;
use aurion::state::chain::ChainLedger;
use aurion::state::monetary::calculate_block_reward;
use aurion::transaction::types::{Transaction, TxType};

#[test]
fn test_bft_block_reward_and_fee_distribution() {
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

    let treasury_key = Keypair::generate();
    let treasury_addr = derive_address_from_pubkey(&treasury_key.public_key_bytes());

    let genesis = build_genesis(treasury_addr, val_entries.clone());
    let validator_set = ValidatorSet::new(val_entries);
    let mut ledger = ChainLedger::from_genesis(genesis);

    let initial_issued = ledger.monetary.total_issued;
    assert_eq!(initial_issued, Quantum::from_aur(66_000_000).unwrap());
    assert_eq!(ledger.monetary.total_burned, Quantum::ZERO);

    let proposer_key = Keypair::generate();
    let proposer_addr = derive_address_from_pubkey(&proposer_key.public_key_bytes());
    assert_eq!(ledger.get_balance(&proposer_addr), Quantum::ZERO);

    let mempool = MempoolEngine::new(1024 * 1024, 3600);
    let bft = BftEngine::new(Some(val_keys[0].clone()), Some(0));

    // --- BLOK 1: EMPTY BLOCK (Murni Hadiah Blok BFT 1 AUR) ---
    {
        let candidate_block = bft.assemble_block_proposal(
            &ledger,
            &mempool,
            0,
            1773532860,
            &proposer_addr,
            1024 * 1024,
        );

        assert_eq!(candidate_block.height(), 1);
        assert_eq!(candidate_block.transactions.len(), 0);

        let block_hash = candidate_block.hash();
        let mut precommits = Vec::new();
        for (idx, key) in val_keys.iter().enumerate().take(3) {
            let prevote =
                Vote::new_signed(key, PHASE_PREVOTE, 1, 0, block_hash, idx as u32).unwrap();
            prevote.verify(&validator_set).unwrap();
            let precommit =
                Vote::new_signed(key, PHASE_PRECOMMIT, 1, 0, block_hash, idx as u32).unwrap();
            precommit.verify(&validator_set).unwrap();
            precommits.push(precommit);
        }

        let cert = bft
            .create_commit_certificate(&validator_set, block_hash, 1, 0, precommits)
            .unwrap();

        let block = Block::new(
            candidate_block.header,
            candidate_block.transactions,
            Some(cert),
        );

        ledger
            .apply_block(block, &proposer_addr)
            .expect("Block 1 apply failed");

        // Verifikasi saldo proposer bertambah tepat 1 AUR
        assert_eq!(
            ledger.get_balance(&proposer_addr),
            Quantum::from_aur(1).unwrap()
        );

        // Verifikasi total penerbitan bertambah 1 AUR
        assert_eq!(
            ledger.monetary.total_issued,
            Quantum::from_aur(66_000_001).unwrap()
        );
        assert_eq!(ledger.monetary.total_burned, Quantum::ZERO);
        assert_eq!(
            ledger.monetary.circulating_supply().unwrap(),
            Quantum::from_aur(66_000_001).unwrap()
        );
    }

    // --- BLOK 2: BLOCK DENGAN TRANSAKSI TRANSFER (Hadiah 1 AUR + 100% Fee Validator) ---
    {
        let recipient_key = Keypair::generate();
        let recipient_addr = derive_address_from_pubkey(&recipient_key.public_key_bytes());

        // Transfer 100 AUR dengan fee 10 AUR (100% Fee ke Proposer, 0% Burn)
        let amount = Quantum::from_aur(100).unwrap();
        let fee = Quantum::from_aur(10).unwrap();

        let mut tx = Transaction {
            version: 1,
            chain_id: 1001,
            tx_type: TxType::Transfer,
            flags: 0,
            sender: treasury_addr,
            recipient: recipient_addr,
            nonce: 0,
            amount,
            fee,
            valid_until: 1800000000,
            payload: Vec::new(),
            signature: Signature::from_bytes([0u8; 64]),
        };
        tx.signature = treasury_key.sign(&tx.signing_preimage());

        let mut block2_mempool = MempoolEngine::new(1024 * 1024, 3600);
        block2_mempool
            .submit_transaction(
                tx,
                &treasury_key.public_key_bytes(),
                1773532900,
                ledger.get_account(&treasury_addr).unwrap(),
            )
            .expect("Submit tx failed");

        let candidate_block = bft.assemble_block_proposal(
            &ledger,
            &block2_mempool,
            0,
            1773532920,
            &proposer_addr,
            1024 * 1024,
        );

        assert_eq!(candidate_block.height(), 2);
        assert_eq!(candidate_block.transactions.len(), 1);

        let block_hash = candidate_block.hash();
        let mut precommits = Vec::new();
        for (idx, key) in val_keys.iter().enumerate().take(3) {
            let prevote =
                Vote::new_signed(key, PHASE_PREVOTE, 2, 0, block_hash, idx as u32).unwrap();
            prevote.verify(&validator_set).unwrap();
            let precommit =
                Vote::new_signed(key, PHASE_PRECOMMIT, 2, 0, block_hash, idx as u32).unwrap();
            precommit.verify(&validator_set).unwrap();
            precommits.push(precommit);
        }

        let cert = bft
            .create_commit_certificate(&validator_set, block_hash, 2, 0, precommits)
            .unwrap();

        let block = Block::new(
            candidate_block.header,
            candidate_block.transactions,
            Some(cert),
        );

        ledger
            .apply_block(block, &proposer_addr)
            .expect("Block 2 apply failed");

        // Saldo Proposer:
        // Awal: 1 AUR (dari Blok 1)
        // Blok 2: + 1 AUR (reward) + 10 AUR (100% dari fee 10 AUR) = 11 AUR
        // Total akumulasi: 1 + 11 = 12 AUR!
        assert_eq!(
            ledger.get_balance(&proposer_addr),
            Quantum::from_aur(12).unwrap()
        );

        // Saldo Recipient: 100 AUR
        assert_eq!(
            ledger.get_balance(&recipient_addr),
            Quantum::from_aur(100).unwrap()
        );

        // Saldo Treasury berkurang 110 AUR (100 amount + 10 fee)
        assert_eq!(
            ledger.get_balance(&treasury_addr),
            Quantum::from_aur(66_000_000 - 110).unwrap()
        );

        // Total Burned: 0 AUR (0% Burn)
        assert_eq!(ledger.monetary.total_burned, Quantum::ZERO);

        // Total Issued: Genesis (66.000.000) + Blok 1 (1) + Blok 2 (1) = 66.000.002 AUR
        assert_eq!(
            ledger.monetary.total_issued,
            Quantum::from_aur(66_000_002).unwrap()
        );

        // Circulating Supply: Total Issued (66.000.002) - Burned (0) = 66.000.002 AUR
        assert_eq!(
            ledger.monetary.circulating_supply().unwrap(),
            Quantum::from_aur(66_000_002).unwrap()
        );
    }
}

#[test]
fn test_bft_block_reward_invariants() {
    // Blok 0 (Genesis): 0 AUR
    assert_eq!(calculate_block_reward(0), Quantum::ZERO);

    // Blok >= 1: Hadiah tetap R = 1 AUR (1.000.000.000 Quantum)
    assert_eq!(calculate_block_reward(1), Quantum::from_aur(1).unwrap());
    assert_eq!(calculate_block_reward(100), Quantum::from_aur(1).unwrap());
    assert_eq!(
        calculate_block_reward(2_145_000),
        Quantum::from_aur(1).unwrap()
    );
    assert_eq!(
        calculate_block_reward(10_000_000),
        Quantum::from_aur(1).unwrap()
    );
}
