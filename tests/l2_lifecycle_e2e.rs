#![forbid(unsafe_code)]

//! Aurion Layer-2 Full Lifecycle End-to-End Integration Test Suite.
//! Menguji siklus utuh: Deposit L1 -> L2 Tx -> Soft Finality -> Batch DA -> L1 Commit -> Withdraw Merkle -> Escape Hatch.
//! Memvalidasi konservasi nilai aset (Vault Conservation Invariant) di setiap transisi.

use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::l2::bridge::L2SettlementBridgeClient;
use aurion::l2::codec::L2BatchFrame;
use aurion::l2::relayer::{
    CrossLayerMessage, L2Relayer, MessageDirection, RelayerError, WithdrawalProof,
};
use aurion::l2::sequencer::L2Sequencer;
use aurion::l2::types::L2Transaction;

#[test]
fn test_l2_full_lifecycle_end_to_end() {
    // =========================================================================
    // INSIALISASI: Jaringan L1, Bridge Contract, Relayer, dan Sequencer L2
    // =========================================================================
    let bridge_address = Address::from_bytes([0xBB; 32]);
    let initial_l2_root = Hash256::ZERO;
    let bridge = L2SettlementBridgeClient::new(bridge_address, initial_l2_root);

    let mut relayer = L2Relayer::new(bridge, 50); // Timeout 50 blok L1
    let mut sequencer = L2Sequencer::new();

    let alice_l1 = Address::from_bytes([0x01; 32]);
    let alice_l2 = Address::from_bytes([0x01; 32]);
    let bob_l1 = Address::from_bytes([0x02; 32]);
    let bob_l2 = Address::from_bytes([0x02; 32]);
    let charlie_l2 = Address::from_bytes([0x03; 32]);

    // =========================================================================
    // TAHAP 1: Deposit L1 -> L2 (Lock in L1 Vault & Mint in L2 State)
    // =========================================================================
    let deposit_amount = Quantum::new(10_000_000_000); // 100 AUR
    let deposit_nonce = relayer
        .process_l1_deposit_to_l2(&mut sequencer.state, alice_l1, alice_l2, deposit_amount)
        .expect("Deposit L1->L2 gagal");

    assert_eq!(deposit_nonce, 1);
    assert_eq!(relayer.bridge.vault_balance, deposit_amount);

    let alice_acc = sequencer
        .state
        .get_account(&alice_l2)
        .expect("Akun Alice di L2 tidak ditemukan");
    assert_eq!(alice_acc.balance, deposit_amount);
    assert_eq!(alice_acc.nonce, 0);

    // =========================================================================
    // TAHAP 2: Transaksi L2 (Alice Transfer ke Bob, Gas Metering, Fee 80/20)
    // =========================================================================
    let transfer_amount = Quantum::new(4_000_000_000); // 40 AUR
    let fee_amount = Quantum::new(50_000); // 50,000 Quanta fee

    let tx = L2Transaction::new(
        alice_l2,
        bob_l2,
        transfer_amount,
        fee_amount,
        0, // Nonce 0
        Signature::from_bytes([0u8; 64]),
        vec![0x01, 0x02, 0x03, 0x04],
    );

    // Kirim ke mempool Sequencer
    sequencer
        .submit_transaction(tx.clone())
        .expect("Submit tx ke mempool gagal");
    assert_eq!(sequencer.mempool.len(), 1);

    // Produksi blok dengan soft finality (<50ms)
    let opt_block = sequencer
        .produce_block_with_attestation(10)
        .expect("Produksi blok sequencer gagal");
    assert!(opt_block.is_some());
    let (block, receipt) = opt_block.unwrap();

    assert_eq!(block.header.block_number, 1);
    assert_eq!(receipt.block_number, 1);
    assert_eq!(receipt.tx_count, 1);
    assert_eq!(receipt.block_hash, block.header.compute_hash());

    // Periksa saldo Alice dan Bob pasca transaksi L2
    let alice_post = sequencer.state.get_account(&alice_l2).cloned().unwrap();
    let bob_post = sequencer.state.get_account(&bob_l2).cloned().unwrap();

    let expected_alice_bal = deposit_amount
        .checked_sub(transfer_amount)
        .unwrap()
        .checked_sub(fee_amount)
        .unwrap();
    assert_eq!(alice_post.balance, expected_alice_bal);
    assert_eq!(alice_post.nonce, 1);
    assert_eq!(bob_post.balance, transfer_amount);
    assert_eq!(bob_post.nonce, 0);

    // =========================================================================
    // TAHAP 3: Batch Assembly & Posting Data Availability (DA) ke Layer-1
    // =========================================================================
    let current_l2_root = sequencer.state.compute_state_root();
    let tx_bytes = tx.encode_canonical();

    let batch_frame = L2BatchFrame::new(
        1,
        Hash256::ZERO,   // prev_root
        current_l2_root, // next_root
        1,               // start_block
        1,               // end_block
        1,               // tx_count
        0,               // compression_flags
        tx_bytes,
    );
    let frame_bytes = batch_frame.encode();
    let da_hash = batch_frame.compute_da_hash();

    // L1 Settlement Bridge memvalidasi transisi state dan komitmen DA
    relayer
        .bridge
        .verify_state_transition_with_da(&frame_bytes)
        .expect("Verifikasi state transition di L1 bridge gagal");

    assert_eq!(relayer.bridge.latest_batch_index, 1);
    assert_eq!(relayer.bridge.latest_state_root, current_l2_root);
    assert_eq!(relayer.bridge.da_commitments[&1], da_hash);

    // =========================================================================
    // TAHAP 4: Penarikan L2 -> L1 (Burn in L2 & Unlock from L1 Vault via Merkle Proof)
    // =========================================================================
    let withdraw_amount = Quantum::new(2_500_000_000); // 25 AUR

    let withdrawal_proof = WithdrawalProof {
        withdrawal_hash: current_l2_root,
        merkle_branch: vec![],
        root: current_l2_root,
        leaf_index: 0,
    };

    relayer
        .process_l2_withdrawal_to_l1(
            &mut sequencer.state,
            bob_l2,
            bob_l1,
            withdraw_amount,
            withdrawal_proof,
        )
        .expect("Penarikan L2->L1 gagal");

    // Saldo Bob di L2 berkurang 25 AUR -> sisa 15 AUR
    let bob_withdrawn = sequencer.state.get_account(&bob_l2).cloned().unwrap();
    assert_eq!(bob_withdrawn.balance.as_u128(), 1_500_000_000);

    // Vault L1 mencairkan 25 AUR -> sisa 75 AUR
    assert_eq!(relayer.bridge.vault_balance.as_u128(), 7_500_000_000);

    // =========================================================================
    // TAHAP 5: Invariant Konservasi Nilai Vault (Asset Conservation)
    // =========================================================================
    // Total aset di L2:
    // Alice (59.9995 AUR) + Bob (15.0000 AUR) = 74.9995 AUR
    // Fee yang dikumpulkan sequencer: 50,000 Quanta = 0.0005 AUR
    // Total L2 = 75.0000 AUR == L1 Vault Balance (75.0000 AUR)
    let total_l2_users =
        alice_post.balance.as_u128() + bob_withdrawn.balance.as_u128() + fee_amount.as_u128();
    assert_eq!(total_l2_users, relayer.bridge.vault_balance.as_u128());

    // =========================================================================
    // TAHAP 6: Anti-Censorship (Forced Inclusion Queue Langsung ke L1)
    // =========================================================================
    let forced_msg = CrossLayerMessage::new(
        MessageDirection::L1ToL2,
        Address::ZERO,
        charlie_l2,
        Quantum::new(500_000_000), // 5 AUR
        vec![],
        2,
        Quantum::ZERO,
        15, // Blok L1 15
    );
    relayer.forced_queue.enqueue(forced_msg);
    assert_eq!(relayer.forced_queue.len(), 1);

    // Eksekusi paksa ke status L2
    let executed = relayer
        .process_forced_inclusion_batch(&mut sequencer.state, 5)
        .expect("Eksekusi antrean paksa gagal");
    assert_eq!(executed.len(), 1);
    assert_eq!(
        sequencer
            .state
            .get_account(&charlie_l2)
            .unwrap()
            .balance
            .as_u128(),
        500_000_000
    );

    // =========================================================================
    // TAHAP 7: Emergency Exit / Escape Hatch (Unilateral Claim on Freeze)
    // =========================================================================
    // Hasilkan bukti keanggotaan SMT untuk saldo sisa Bob (15 AUR)
    // Perbarui root bridge L1 dengan root L2 terbaru
    let latest_root = sequencer.state.compute_state_root();
    relayer.bridge.latest_state_root = latest_root;
    let bob_proof = sequencer
        .state
        .generate_account_proof(&bob_l2)
        .expect("SMT Proof Bob gagal");

    // Klaim gagal sebelum freeze
    let pre_err = relayer
        .process_escape_hatch(&bob_proof, bob_l1, 20, Quantum::new(1_500_000_000))
        .unwrap_err();
    assert_eq!(pre_err, RelayerError::EscapeHatchNotActive);

    // Bekukan Sequencer
    relayer.trigger_emergency_freeze();

    // Klaim sepihak berhasil saat freeze
    let claimed = relayer
        .process_escape_hatch(&bob_proof, bob_l1, 20, Quantum::new(1_500_000_000))
        .expect("Klaim escape hatch Bob gagal");
    assert_eq!(claimed.as_u128(), 1_500_000_000);

    // Saldo vault L1 berkurang 15 AUR -> sisa 60 AUR (6,000,000,000 Quanta)
    assert_eq!(relayer.bridge.vault_balance.as_u128(), 6_000_000_000);

    // Anti-klaim ganda: klaim kedua kali harus ditolak mutlak
    let double_claim_err = relayer
        .process_escape_hatch(&bob_proof, bob_l1, 20, Quantum::new(1_500_000_000))
        .unwrap_err();
    assert_eq!(double_claim_err, RelayerError::AlreadyClaimed(bob_l2));
}
