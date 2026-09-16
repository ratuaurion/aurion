#![forbid(unsafe_code)]

//! Aurion Layer-3 Full Multi-Layer Lifecycle End-to-End Integration Suite (L1-L2-L3).
//! Tests the complete flow across all three layers:
//! 1. Layer-1 Sovereign Anchor & L2 Bridge Initialization
//! 2. Layer-2 Rollup Deposit & Sequencer Execution
//! 3. Layer-2 -> Layer-3 Two-Way Relayer Deposit (L2 Vault Lock & L3 State Credit)
//! 4. Layer-3 Specialized Domain Execution (Order-Book DEX & Ephemeral Gaming Session)
//! 5. Layer-3 State Tree SMT Update & Witness Proof Generation
//! 6. Layer-3 -> Layer-2 Periodic Checkpoint Settlement & Soft Finality
//! 7. Layer-3 -> Layer-2 Withdrawal with Sparse Merkle Tree Proof Verification
//! 8. Strict Asset Value Conservation & L1 Hard Finality Promotion

use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::crypto::Keypair;
use aurion::l2::bridge::L2SettlementBridgeClient;
use aurion::l2::relayer::L2Relayer;
use aurion::l2::sequencer::L2Sequencer;
use aurion::specialized::domains::{
    GameAction, GameSession, GameSessionStatus, OrderBook, OrderSide, OrderType,
    ShieldedNote, ShieldedPool,
};
use aurion::specialized::messaging::{
    CrossLayerMessage, L2L3TwoWayRelayer, L3WithdrawalProof,
};
use aurion::specialized::runtime::{
    L3ExecutionConfig, L3ExecutionEngine,
};
use aurion::specialized::settlement::{
    L2SettlementClient, L3CheckpointGenerator, L3FinalityStatus, L3FinalityTier,
};
use aurion::specialized::state::L3State;
use aurion::specialized::types::{
    DomainId, L3Block, L3Transaction,
};

#[test]
fn test_l1_l2_l3_full_multi_layer_lifecycle_e2e() {
    // =========================================================================
    // STEP 1: INITIALIZE L1 SOVEREIGN BRIDGE & L2 ROLLUP ENGINE
    // =========================================================================
    let l1_bridge_address = Address::from_bytes([0xBB; 32]);
    let initial_l2_root = Hash256::ZERO;
    let l2_bridge = L2SettlementBridgeClient::new(l1_bridge_address, initial_l2_root);

    let mut l2_relayer = L2Relayer::new(l2_bridge, 50);
    let mut l2_sequencer = L2Sequencer::new();

    let alice = Address::from_bytes([0x01; 32]);
    let bob = Address::from_bytes([0x02; 32]);

    // Deposit 100 AUR into L2 for Alice
    let l1_to_l2_deposit = Quantum::new(10_000_000_000); // 100 AUR
    l2_relayer
        .process_l1_deposit_to_l2(&mut l2_sequencer.state, alice, alice, l1_to_l2_deposit)
        .expect("L1->L2 deposit failed");
    assert_eq!(l2_relayer.bridge.vault_balance, l1_to_l2_deposit);
    assert_eq!(
        l2_sequencer.state.get_account(&alice).expect("alice in l2").balance,
        l1_to_l2_deposit
    );

    // =========================================================================
    // STEP 2: L2 -> L3 TWO-WAY RELAYER DEPOSIT (LOCK IN L2, CREDIT IN L3)
    // =========================================================================
    let l3_domain = DomainId::named("aurion-dex-gaming-l3");
    let l2_domain = DomainId::named("aurion-l2-rollup");
    let mut l2_l3_relayer = L2L3TwoWayRelayer::new(l3_domain);
    let mut l3_state = L3State::new(l3_domain);

    let l2_to_l3_deposit = Quantum::new(2_000_000_000); // 20 AUR
    let deposit_msg = l2_l3_relayer
        .create_deposit_message(l2_domain, alice, l2_to_l3_deposit)
        .expect("create L2->L3 deposit message");

    assert_eq!(l2_l3_relayer.deposit_vault_l2, l2_to_l3_deposit);
    assert_eq!(deposit_msg.source_domain, l2_domain);
    assert_eq!(deposit_msg.destination_domain, l3_domain);

    // Credit Alice in L3 State
    l3_state.credit(&alice, l2_to_l3_deposit).expect("credit Alice in L3");
    assert_eq!(l3_state.get_balance(&alice), l2_to_l3_deposit);

    // Also credit Bob with 10 AUR for interaction
    let bob_initial = Quantum::new(1_000_000_000); // 10 AUR
    l3_state.credit(&bob, bob_initial).expect("credit Bob in L3");

    // =========================================================================
    // STEP 3: L3 SPECIALIZED DOMAIN EXECUTIONS
    // =========================================================================
    // 3.1: DEX Microsecond Order-Book Execution
    let pair = *b"AUR/USDT";
    let mut order_book = OrderBook::new(l3_domain, pair);

    // Bob places resting limit ask: Sell 5 AUR @ 100 Quanta per unit
    let ask_trades = order_book
        .place_order(
            *bob.as_bytes(),
            OrderSide::Sell,
            OrderType::Limit,
            Quantum(100),
            Quantum(500_000_000), // 5 AUR
        )
        .expect("place Bob ask");
    assert!(ask_trades.is_empty());
    assert_eq!(order_book.best_ask(), Some(Quantum(100)));

    // Alice places aggressive bid: Buy 5 AUR @ 100 Quanta per unit
    let bid_trades = order_book
        .place_order(
            *alice.as_bytes(),
            OrderSide::Buy,
            OrderType::Limit,
            Quantum(100),
            Quantum(500_000_000), // 5 AUR
        )
        .expect("place Alice aggressive bid");
    assert_eq!(bid_trades.len(), 1);
    assert_eq!(bid_trades[0].maker, *bob.as_bytes());
    assert_eq!(bid_trades[0].taker, *alice.as_bytes());
    assert_eq!(bid_trades[0].quantity, Quantum(500_000_000));

    let executed_batch = order_book.drain_executed_batch();
    assert_eq!(executed_batch.len(), 1);
    let trade_root = OrderBook::compute_trade_batch_root(&executed_batch);
    assert_ne!(trade_root, [0u8; 32]);

    // 3.2: High-Frequency Ephemeral Gaming Session
    let session_id = [0x77; 32];
    let players = vec![*alice.as_bytes(), *bob.as_bytes()];
    let stake_per_player = Quantum::new(100_000_000); // 1 AUR
    let mut session = GameSession::new(session_id, l3_domain, players, stake_per_player)
        .expect("create game session");

    // Sequential game actions
    session
        .apply_action(GameAction {
            player: *alice.as_bytes(),
            action_type: 1,
            payload: vec![10, 20, 30],
            score_delta: 50,
            sequence: 1,
        })
        .expect("action 1");

    session
        .apply_action(GameAction {
            player: *bob.as_bytes(),
            action_type: 1,
            payload: vec![40, 50, 60],
            score_delta: 40,
            sequence: 2,
        })
        .expect("action 2");

    let summary = session.finalize_session(*alice.as_bytes()).expect("finalize game");
    assert_eq!(summary.winner, *alice.as_bytes());
    assert_eq!(summary.payout, Quantum::new(200_000_000)); // 2 AUR
    assert_eq!(session.status, GameSessionStatus::Completed);

    // 3.3: Zero-Knowledge Confidential Privacy Shield
    let mut shielded_pool = ShieldedPool::new(l3_domain);
    let note1 = ShieldedNote {
        value: Quantum::new(50_000_000), // 0.5 AUR
        nullifier_preimage: [0x33; 32],
        recipient: *alice.as_bytes(),
        randomness: [0x44; 32],
    };
    let _note_comm = shielded_pool.shield(&note1).expect("shield note");
    assert_eq!(shielded_pool.vault_balance, Quantum::new(50_000_000));

    // =========================================================================
    // STEP 4: L3 STATE MUTATION & WITNESS PROOF GENERATION
    // =========================================================================
    // Perform transfer in L3 runtime: Alice transfers 1 AUR to Bob with fee
    let l3_engine = L3ExecutionEngine::new(L3ExecutionConfig::default_for(l3_domain));
    let transfer_amount = Quantum::new(100_000_000); // 1 AUR
    let fee = Quantum::new(20_000); // 20,000 Quanta

    let l3_tx = L3Transaction::new(
        l3_domain,
        alice,
        bob,
        transfer_amount,
        fee,
        0,
        Signature::from_bytes([0x88; 64]),
        vec![],
    );

    let receipt = l3_engine
        .execute_transaction(&mut l3_state, &l3_tx)
        .expect("execute L3 tx");
    assert!(receipt.success);

    let state_root = l3_state.compute_state_root();
    assert_ne!(state_root, Hash256::ZERO);

    // Generate state witness proof for Alice
    let alice_proof = l3_state
        .generate_account_proof(&alice)
        .expect("generate proof for Alice");
    assert!(alice_proof.verify());

    // =========================================================================
    // STEP 5: PERIODIC CHECKPOINT SETTLEMENT TO L2
    // =========================================================================
    let sequencer_keypair = Keypair::generate();
    let mut checkpoint_gen = L3CheckpointGenerator::new(l3_domain, 1, Hash256::ZERO);

    let l3_block = L3Block::new(
        l3_domain,
        1,
        Hash256::ZERO,
        state_root,
        receipt.tx_id,
        Hash256::ZERO,
        1_700_000_000,
        vec![l3_tx],
    );
    checkpoint_gen.record_block(l3_block).expect("record L3 block");

    let checkpoint = checkpoint_gen
        .create_checkpoint(&sequencer_keypair, vec![0xdd; 32])
        .expect("create checkpoint");

    // Ingest checkpoint into L2 Settlement Client
    let mut l2_client = L2SettlementClient::new(
        l3_domain,
        sequencer_keypair.public_key_bytes(),
        Hash256::ZERO,
    );
    l2_client.ingest_checkpoint(checkpoint).expect("ingest checkpoint into L2");

    // Track finality status
    let mut finality = L3FinalityStatus::new_local(l3_domain, receipt.tx_id, 1);
    finality.promote_to_l2(1);
    assert_eq!(finality.tier, L3FinalityTier::SoftL2Settled);

    // =========================================================================
    // STEP 6: L3 -> L2 WITHDRAWAL WITH SMT PROOF & VAULT CONSERVATION
    // =========================================================================
    let withdraw_amount = Quantum::new(500_000_000); // 5 AUR withdrawal
    let withdrawal_proof = L3WithdrawalProof {
        account_proof: alice_proof,
        recipient_l2: alice,
        amount: withdraw_amount,
    };

    // Construct withdrawal message from L3 to L2
    let mut withdraw_payload = Vec::with_capacity(32 + 16);
    withdraw_payload.extend_from_slice(alice.as_bytes());
    withdraw_payload.extend_from_slice(&withdraw_amount.as_u128().to_be_bytes());

    let withdraw_msg = CrossLayerMessage::new(
        l3_domain,
        l2_domain,
        1,
        withdraw_payload,
        vec![],
    );

    let (unlocked_recipient, unlocked_amount) = l2_l3_relayer
        .verify_withdrawal_and_unlock(&withdraw_msg, state_root, &withdrawal_proof)
        .expect("verify withdrawal & unlock in L2");

    assert_eq!(unlocked_recipient, alice);
    assert_eq!(unlocked_amount, withdraw_amount);

    // Verify Asset Conservation in L2 Vault: 20 AUR initial - 5 AUR withdrawn = 15 AUR remaining in L2 Vault
    let expected_remaining_vault = l2_to_l3_deposit
        .checked_sub(withdraw_amount)
        .expect("sub");
    assert_eq!(l2_l3_relayer.deposit_vault_l2, expected_remaining_vault);

    // =========================================================================
    // STEP 7: MULTI-LAYER HARD FINALITY PROMOTION AT LAYER-1
    // =========================================================================
    finality.promote_to_l1(100);
    assert_eq!(finality.tier, L3FinalityTier::HardL1Finalized);
    assert_eq!(finality.l1_settlement_height, Some(100));

    // Full multi-layer verification completed successfully
}
