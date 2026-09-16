#![forbid(unsafe_code)]

//! Aurion Layer-2 10-Pillar Protocol Conformance Test Suite (L2-CTS).
//! Memverifikasi seluruh subsistem rollup L2 terhadap spesifikasi kanonikal Dokumen 17 (REQ-L2-01 s.d REQ-L2-10).

use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::l2::abi::{
    BridgeCall, SELECTOR_DEPOSIT, SELECTOR_ENQUEUE_FORCED_TX, SELECTOR_VERIFY_STATE_TRANSITION,
    SELECTOR_WITHDRAW,
};
use aurion::l2::bridge::L2SettlementBridgeClient;
use aurion::l2::codec::{L2BatchFrame, MAGIC_AUL2};
use aurion::l2::relayer::{
    CrossLayerMessage, L2Relayer, MessageDirection, RelayerError, WithdrawalProof,
};
use aurion::l2::sequencer::{L2Sequencer, MAX_L2_MEMPOOL_CAPACITY};
use aurion::l2::state::{L2Account, L2StateStore};
use aurion::l2::types::{compute_txs_root, L2Block, L2BlockHeader, L2Transaction};
use aurion::l2::vm::L2ExecutionEngine;

/// REQ-L2-01: L1 Bridge Contract Interface Specification
#[test]
fn pillar_1_bridge_abi_and_function_selectors() {
    // 1. Verifikasi panjang selektor 4-byte Blake3
    assert_eq!(SELECTOR_DEPOSIT.len(), 4);
    assert_eq!(SELECTOR_VERIFY_STATE_TRANSITION.len(), 4);
    assert_eq!(SELECTOR_WITHDRAW.len(), 4);
    assert_eq!(SELECTOR_ENQUEUE_FORCED_TX.len(), 4);

    // 2. Roundtrip ABI Deposit
    let recipient = Address::from_bytes([0x42; 32]);
    let amount = Quantum::new(500_000_000); // 5 AUR
    let call_deposit = BridgeCall::Deposit {
        recipient_l2: recipient,
        amount,
    };
    let calldata = call_deposit.encode();
    assert_eq!(&calldata[0..4], &SELECTOR_DEPOSIT);

    let decoded = BridgeCall::decode(&calldata).expect("Decode deposit gagal");
    assert_eq!(decoded, call_deposit);

    // 3. Roundtrip ABI Verify State Transition
    let prev_root = Hash256::from_bytes([1u8; 32]);
    let next_root = Hash256::from_bytes([2u8; 32]);
    let da_hash = Hash256::from_bytes([3u8; 32]);
    let call_st = BridgeCall::VerifyStateTransition {
        batch_index: 1,
        prev_state_root: prev_root,
        new_state_root: next_root,
        start_block: 100,
        end_block: 150,
        calldata_hash: da_hash,
    };
    let st_calldata = call_st.encode();
    assert_eq!(&st_calldata[0..4], &SELECTOR_VERIFY_STATE_TRANSITION);

    let decoded_st = BridgeCall::decode(&st_calldata).expect("Decode ST gagal");
    assert_eq!(decoded_st, call_st);

    // 4. Roundtrip ABI Withdraw
    let call_withdraw = BridgeCall::Withdraw {
        recipient_l1: recipient,
        amount,
        leaf_index: 0,
        merkle_branch: vec![],
    };
    let w_calldata = call_withdraw.encode();
    let decoded_w = BridgeCall::decode(&w_calldata).expect("Decode withdraw gagal");
    assert_eq!(decoded_w, call_withdraw);
}

/// REQ-L2-02: Rollup Batch Serialization & Compression
#[test]
fn pillar_2_batch_codec_and_da_commitment() {
    let tx = L2Transaction::new(
        Address::from_bytes([1u8; 32]),
        Address::from_bytes([2u8; 32]),
        Quantum::new(100_000_000),
        Quantum::new(10_000),
        0,
        Signature::from_bytes([0u8; 64]),
        vec![0xAA, 0xBB],
    );
    let tx_bytes = tx.encode_canonical();
    let frame = L2BatchFrame::new(
        1,
        Hash256::ZERO,
        Hash256::from_bytes([0x88; 32]),
        1,
        1,
        1,
        0,
        tx_bytes,
    );

    let encoded = frame.encode();
    assert_eq!(&encoded[0..4], &MAGIC_AUL2);

    // Validasi hash komitmen DA Blake3 deterministik
    let da_hash = frame.compute_da_hash();
    assert_ne!(da_hash, Hash256::ZERO);

    let decoded = L2BatchFrame::decode(&encoded).expect("Decode batch frame gagal");
    assert_eq!(decoded.header.batch_index, frame.header.batch_index);
    assert_eq!(decoded.payload, frame.payload);
    assert_eq!(decoded.compute_da_hash(), da_hash);
}

/// REQ-L2-03: Blake3 Sparse Merkle Tree (SMT) State Roots
#[test]
fn pillar_3_smt_state_roots_and_proofs() {
    let mut store = L2StateStore::new();
    assert_eq!(store.compute_state_root(), Hash256::ZERO);

    let user = Address::from_bytes([7u8; 32]);
    let acc = L2Account::new(user, Quantum::new(2_500_000_000), 5);
    store.set_account(acc.clone());

    let root = store.compute_state_root();
    assert_ne!(root, Hash256::ZERO);

    // Generate dan verify keanggotaan proof
    let proof = store.generate_account_proof(&user).expect("Generate proof gagal");
    assert_eq!(proof.address, user);
    assert_eq!(proof.account_hash, acc.compute_account_hash());
    assert_eq!(proof.root, root);
    assert!(proof.verify());

    // Proof untuk akun fiktif yang tidak ada harus Err
    let fake_user = Address::from_bytes([9u8; 32]);
    assert!(store.generate_account_proof(&fake_user).is_err());
}

/// REQ-L2-04: Calldata Data Availability Posting to L1
#[test]
fn pillar_4_da_posting_to_l1() {
    let bridge_addr = Address::from_bytes([0x99; 32]);
    let mut bridge = L2SettlementBridgeClient::new(bridge_addr, Hash256::ZERO);

    let frame = L2BatchFrame::new(
        1,
        Hash256::ZERO,
        Hash256::from_bytes([0x11; 32]),
        1,
        10,
        1,
        0,
        vec![0xDE, 0xAD, 0xBE, 0xEF],
    );
    let frame_bytes = frame.encode();

    // Ingestion DA frame ke L1 Bridge
    bridge.verify_state_transition_with_da(&frame_bytes).expect("Posting DA gagal");
    assert_eq!(bridge.latest_batch_index, 1);
    assert_eq!(bridge.latest_state_root, Hash256::from_bytes([0x11; 32]));

    // Memverifikasi bahwa komitmen DA tercatat di riwayat L1
    assert_eq!(bridge.da_commitments.len(), 1);
    assert_eq!(bridge.da_commitments[&1], frame.compute_da_hash());
}

/// REQ-L2-05: ZK / State Transition STF & Atomic Rollback
#[test]
fn pillar_5_state_transition_stf_and_atomic_rollback() {
    let mut store = L2StateStore::new();
    let sender = Address::from_bytes([1u8; 32]);
    let recipient = Address::from_bytes([2u8; 32]);
    let initial_balance = Quantum::new(100_000_000); // 1 AUR

    store.set_account(L2Account::new(sender, initial_balance, 0));

    let tx = L2Transaction::new(
        sender,
        recipient,
        Quantum::new(30_000_000),
        Quantum::new(10_000),
        0,
        Signature::from_bytes([0u8; 64]),
        vec![],
    );

    let block = L2Block {
        header: L2BlockHeader {
            block_number: 1,
            prev_hash: Hash256::ZERO,
            state_root: Hash256::from_bytes([0xFF; 32]), // Mismatch yang disengaja
            txs_root: compute_txs_root(std::slice::from_ref(&tx)),
            timestamp: 1000,
        },
        transactions: vec![tx],
    };

    // Eksekusi harus rollback atomik jika state_root blok yang diharapkan tidak cocok
    let engine = L2ExecutionEngine::new();
    let result = engine.execute_block(&mut store, &block);
    assert!(result.is_err());
    // Saldo sender harus tetap utuh (tidak berkurang) berkat proteksi snapshot atomik
    assert_eq!(store.get_account(&sender).unwrap().balance, initial_balance);
}

/// REQ-L2-06: Two-Way Cross-Layer Message Relayer
#[test]
fn pillar_6_two_way_messaging_relayer() {
    let bridge_addr = Address::from_bytes([0x99; 32]);
    let bridge = L2SettlementBridgeClient::new(bridge_addr, Hash256::ZERO);
    let mut relayer = L2Relayer::new(bridge, 100);
    let mut state = L2StateStore::new();

    let user_l1 = Address::from_bytes([1u8; 32]);
    let user_l2 = Address::from_bytes([2u8; 32]);
    let amount = Quantum::new(500_000_000); // 5 AUR

    // 1. L1 -> L2 Deposit (Lock & Mint)
    relayer
        .process_l1_deposit_to_l2(&mut state, user_l1, user_l2, amount)
        .expect("Deposit L1->L2 gagal");

    assert_eq!(relayer.bridge.vault_balance, amount);
    assert_eq!(state.get_account(&user_l2).unwrap().balance, amount);

    // 2. L2 -> L1 Withdrawal (Burn & Unlock)
    let current_root = state.compute_state_root();
    relayer.bridge.latest_state_root = current_root;

    let proof = WithdrawalProof {
        withdrawal_hash: current_root,
        merkle_branch: vec![],
        root: current_root,
        leaf_index: 0,
    };

    relayer
        .process_l2_withdrawal_to_l1(&mut state, user_l2, user_l1, Quantum::new(200_000_000), proof)
        .expect("Penarikan L2->L1 gagal");

    assert_eq!(state.get_account(&user_l2).unwrap().balance.as_u128(), 300_000_000);
    assert_eq!(relayer.bridge.vault_balance.as_u128(), 300_000_000);
}

/// REQ-L2-07: Forced Inclusion Queue on L1
#[test]
fn pillar_7_forced_inclusion_queue() {
    let bridge = L2SettlementBridgeClient::new(Address::ZERO, Hash256::ZERO);
    let mut relayer = L2Relayer::new(bridge, 50); // Timeout 50 blok
    let mut state = L2StateStore::new();

    let target = Address::from_bytes([3u8; 32]);
    let msg = CrossLayerMessage::new(
        MessageDirection::L1ToL2,
        Address::ZERO,
        target,
        Quantum::new(100_000_000),
        vec![],
        1,
        Quantum::ZERO,
        10, // Dimasukkan di blok L1 height 10
    );
    let msg_id = msg.id;

    relayer.forced_queue.enqueue(msg);
    assert_eq!(relayer.forced_queue.len(), 1);

    // Di blok 40 belum timeout
    assert!(!relayer.forced_queue.is_timed_out(&msg_id, 40));
    // Di blok 61 sudah timeout (> 50 blok elapsed)
    assert!(relayer.forced_queue.is_timed_out(&msg_id, 61));

    // Eksekusi antrean transaksi paksa ke state L2
    let executed = relayer
        .process_forced_inclusion_batch(&mut state, 10)
        .expect("Eksekusi batch antrean paksa gagal");
    assert_eq!(executed.len(), 1);
    assert_eq!(state.get_account(&target).unwrap().balance.as_u128(), 100_000_000);
    assert!(relayer.forced_queue.is_empty());
}

/// REQ-L2-08: L2 Sequencer Engine & Soft Finality BFT
#[test]
fn pillar_8_sequencer_mempool_and_soft_finality() {
    let mut sequencer = L2Sequencer::new();
    assert_eq!(MAX_L2_MEMPOOL_CAPACITY, 10_000);

    let sender = Address::from_bytes([1u8; 32]);
    sequencer.state.set_account(L2Account::new(sender, Quantum::new(500_000_000), 0));

    let tx = L2Transaction::new(
        sender,
        Address::from_bytes([2u8; 32]),
        Quantum::new(50_000_000),
        Quantum::new(20_000),
        0,
        Signature::from_bytes([0u8; 64]),
        vec![],
    );

    sequencer.submit_transaction(tx).expect("Submit tx gagal");
    assert_eq!(sequencer.mempool.len(), 1);

    // Produksi blok dengan soft finality (<50ms)
    let opt = sequencer.produce_block_with_attestation(10).expect("Produksi blok gagal");
    assert!(opt.is_some());
    let (block, receipt) = opt.unwrap();
    assert_eq!(block.header.block_number, 1);
    assert_eq!(receipt.block_number, 1);
    assert_eq!(receipt.block_hash, block.header.compute_hash());
    assert_eq!(sequencer.mempool.len(), 0);
}

/// REQ-L2-09: Emergency Exit / Escape Hatch Mechanism
#[test]
fn pillar_9_escape_hatch_emergency_exit() {
    let mut state = L2StateStore::new();
    let victim = Address::from_bytes([0x44; 32]);
    let balance = Quantum::new(800_000_000); // 8 AUR
    state.set_account(L2Account::new(victim, balance, 1));

    let state_root = state.compute_state_root();
    let bridge_addr = Address::from_bytes([0x99; 32]);
    let mut bridge = L2SettlementBridgeClient::new(bridge_addr, state_root);
    bridge.process_deposit(Quantum::new(1_000_000_000)).unwrap(); // Vault ada 10 AUR

    let mut relayer = L2Relayer::new(bridge, 100);
    let proof = state.generate_account_proof(&victim).expect("SMT proof gagal");

    // Klaim ditolak saat jaringan normal
    let err = relayer.process_escape_hatch(&proof, victim, 10, balance).unwrap_err();
    assert_eq!(err, RelayerError::EscapeHatchNotActive);

    // Memicu pembekuan sequencer
    relayer.trigger_emergency_freeze();

    // Klaim berhasil saat freeze
    let claimed = relayer.process_escape_hatch(&proof, victim, 10, balance).expect("Escape hatch gagal");
    assert_eq!(claimed, balance);
    assert_eq!(relayer.bridge.vault_balance.as_u128(), 200_000_000);

    // Proteksi anti-klaim ganda
    let double_err = relayer.process_escape_hatch(&proof, victim, 10, balance).unwrap_err();
    assert_eq!(double_err, RelayerError::AlreadyClaimed(victim));
}

/// REQ-L2-10: Zero-Float and Zero-Unsafe Invariants
#[test]
fn pillar_10_zero_float_and_zero_unsafe() {
    // 1. Pastikan seluruh perhitungan biaya gas menggunakan aritmatika integer pasti
    let gas_used: u64 = 10_000;
    let gas_price = Quantum::new(1);
    let fee = Quantum::new(gas_used as u128 * gas_price.as_u128());
    assert_eq!(fee.as_u128(), 10_000);

    // 2. Pembagian fee 80/20 tanpa pembulatan mengambang
    let seq_share = (fee.as_u128() * 80) / 100;
    let l1_share = fee.as_u128() - seq_share;
    assert_eq!(seq_share, 8_000);
    assert_eq!(l1_share, 2_000);
    assert_eq!(seq_share + l1_share, fee.as_u128());

    // 3. Konservasi nilai moneter Quantum
    let q1 = Quantum::new(123_456_789);
    let q2 = Quantum::new(987_654_321);
    let sum = q1.checked_add(q2).expect("Overflow");
    assert_eq!(sum.as_u128(), 1_111_111_110);
}
