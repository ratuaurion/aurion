#![forbid(unsafe_code)]

//! Integration Test End-to-End: Siklus Hidup Rantai Blok Utuh Aurion.
//! Menguji Genesis -> Transaksi -> Mempool -> Proposer -> BFT Voting ->
//! Commit Certificate -> Komit Ledger -> Verifikasi State & Fee Burn -> RPC Query.

use aurion::consensus::certificate::{ValidatorEntry, ValidatorSet};
use aurion::consensus::vote::{Vote, PHASE_PRECOMMIT, PHASE_PREVOTE};
use aurion::core::{Quantum, Signature};
use aurion::crypto::{derive_address_from_pubkey, Keypair};
use aurion::gateway::rpc::types::JsonRpcRequest;
use aurion::genesis::builder::build_genesis;
use aurion::runtime::config::NodeConfig;
use aurion::runtime::AurionNode;
use aurion::transaction::types::{Transaction, TxType};

#[test]
fn test_end_to_end_blockchain_lifecycle() {
    // 1. SETUP GENESIS DENGAN 4 VALIDATOR
    // Masing-masing memiliki bobot 25 (Total: 100, Kuorum Q = 67)
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

    let genesis = build_genesis(creator_addr, val_entries.clone());
    let validator_set = ValidatorSet::new(val_entries);

    let config = NodeConfig::default();
    let node = AurionNode::new(config, genesis, Some(val_keys[0].clone()), Some(0));

    // Verifikasi Inisialisasi Genesis
    assert_eq!(node.ledger.lock().unwrap().latest_height(), 0);
    assert_eq!(
        node.ledger.lock().unwrap().get_balance(&creator_addr),
        Quantum::from_aur(66_000_000).unwrap()
    );

    // 2. PEMBUATAN TRANSAKSI DARI CREATOR KE ALICE
    // Transfer: 1.000 AUR, Fee: 10 AUR, Nonce: 0
    let alice_key = Keypair::generate();
    let alice_addr = derive_address_from_pubkey(&alice_key.public_key_bytes());

    let tx_amount = Quantum::from_aur(1_000).unwrap();
    let tx_fee = Quantum::from_aur(10).unwrap();

    let mut tx = Transaction {
        version: 1,
        chain_id: 1001,
        tx_type: TxType::Transfer,
        flags: 0,
        sender: creator_addr,
        recipient: alice_addr,
        nonce: 0,
        amount: tx_amount,
        fee: tx_fee,
        valid_until: 1800000000,
        payload: Vec::new(),
        signature: Signature::from_bytes([0u8; 64]),
    };

    let sig_preimage = tx.signing_preimage();
    let sig = creator_key.sign(&sig_preimage);
    tx.signature = sig;

    // 3. MASUKKAN TRANSAKSI KE MEMPOOL
    let creator_acct = node
        .ledger
        .lock()
        .unwrap()
        .get_account(&creator_addr)
        .unwrap()
        .clone();

    let tx_id = node
        .mempool
        .lock()
        .unwrap()
        .submit_transaction(
            tx.clone(),
            &creator_key.public_key_bytes(),
            1773532850,
            &creator_acct,
        )
        .expect("Mempool submission failed");

    assert_eq!(tx_id, tx.compute_tx_id());
    assert_eq!(node.mempool.lock().unwrap().len(), 1);

    // 4. BFT CONSENSUS: RAKIT PROPOSAL BLOK #1
    let miner_addr = derive_address_from_pubkey(&val_keys[0].public_key_bytes());
    let candidate_block = {
        let ledger_guard = node.ledger.lock().unwrap();
        let mempool_guard = node.mempool.lock().unwrap();
        let bft = node.bft_engine.lock().unwrap();
        bft.assemble_block_proposal(
            &ledger_guard,
            &mempool_guard,
            0,
            1773532900,
            &miner_addr,
            1024 * 1024,
        )
    };

    assert_eq!(candidate_block.height(), 1);
    assert_eq!(candidate_block.transactions.len(), 1);
    assert!(candidate_block.verify_tx_merkle_root());

    let block_hash = candidate_block.hash();

    // 5. BFT CONSENSUS: DUA FASE PEMUNGUTAN SUARA (PREVOTE & PRECOMMIT)
    // Validator 0, 1, 2 memberikan suara (3 * 25 = 75 >= 67 Kuorum)
    let mut precommits = Vec::new();
    for (idx, key) in val_keys.iter().enumerate().take(3) {
        // Fase 1: Prevote
        let prevote = Vote::new_signed(key, PHASE_PREVOTE, 1, 0, block_hash, idx as u32)
            .expect("Prevote failed");
        prevote
            .verify(&validator_set)
            .expect("Prevote verify failed");

        // Fase 2: Precommit (setelah melihat Polka)
        let precommit = Vote::new_signed(key, PHASE_PRECOMMIT, 1, 0, block_hash, idx as u32)
            .expect("Precommit failed");
        precommit
            .verify(&validator_set)
            .expect("Precommit verify failed");
        precommits.push(precommit);
    }

    // 6. PEMBENTUKAN COMMIT CERTIFICATE
    let commit_cert = node
        .bft_engine
        .lock()
        .unwrap()
        .create_commit_certificate(&validator_set, block_hash, 1, 0, precommits)
        .expect("Commit certificate creation failed");

    // 7. FINALISASI DAN KOMIT KE LEDGER
    let finalized_block = node
        .produce_and_commit_block(commit_cert, &miner_addr, 1773532900)
        .expect("Block commit failed");

    assert_eq!(finalized_block.height(), 1);
    assert_eq!(node.ledger.lock().unwrap().latest_height(), 1);
    assert_eq!(node.ledger.lock().unwrap().finalized_height(), 1);

    // 8. VERIFIKASI PEMBARUAN STATE & MONETARY INVARIANTS
    let ledger_guard = node.ledger.lock().unwrap();

    // Saldo Alice bertambah 1.000 AUR
    assert_eq!(
        ledger_guard.get_balance(&alice_addr),
        Quantum::from_aur(1_000).unwrap()
    );

    // Saldo Creator berkurang 1.010 AUR
    assert_eq!(
        ledger_guard.get_balance(&creator_addr),
        Quantum::from_aur(66_000_000 - 1_010).unwrap()
    );
    assert_eq!(ledger_guard.get_nonce(&creator_addr), 1);

    // Penerbitan Hadiah Blok Kanonikal (1 AUR) + Alokasi Fee 100% Proposer (10 AUR) = 11 AUR
    assert_eq!(
        ledger_guard.get_balance(&miner_addr),
        Quantum::from_aur(11).unwrap()
    );

    // Alokasi Fee 0% Burn
    assert_eq!(ledger_guard.monetary.total_burned, Quantum::ZERO);

    // Total pasokan diterbitkan: Genesis (66.000.000 AUR) + Blok 1 Reward (1 AUR)
    assert_eq!(
        ledger_guard.monetary.total_issued,
        Quantum::from_aur(66_000_001).unwrap()
    );

    // Mempool wajib kosong karena transaksi telah difinalisasi
    drop(ledger_guard);
    assert_eq!(node.mempool.lock().unwrap().len(), 0);

    // 9. VERIFIKASI VIA QUERY JSON-RPC CONTEXT
    node.sync_rpc_context();

    let height_req =
        JsonRpcRequest::parse(r#"{"jsonrpc":"2.0","id":1,"method":"aur_blockHeight","params":[]}"#)
            .unwrap();
    let height_resp = node.rpc_context.dispatch(&height_req, 1773533000);
    assert_eq!(height_resp.result.unwrap(), "1");

    let cert_req = JsonRpcRequest::parse(
        r#"{"jsonrpc":"2.0","id":2,"method":"aur_getCommitCertificate","params":["1"]}"#,
    )
    .unwrap();
    let cert_resp = node.rpc_context.dispatch(&cert_req, 1773533000);
    assert!(cert_resp.result.is_some());
}
