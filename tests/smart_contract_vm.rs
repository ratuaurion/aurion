#![forbid(unsafe_code)]

//! Test Suite Integrasi Aurion VM (AVM) Smart Contract Subsystem.
//! Mematuhi Dokumen Aturan Aplikasi 16 (16-SMART-CONTRACT-EXECUTION-SPECIFICATION.md)
//! dan Invariant AUR-VM-001 s.d AUR-VM-010.

use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::crypto::blake3_hash;
use aurion::state::account::Account;
use aurion::state::monetary::MonetaryState;
use aurion::state::stf::{apply_transaction, derive_contract_address};
use aurion::transaction::types::{Transaction, TxType};
use aurion::vm::{
    AvmEngine, BytecodeVerifier, ExecutionContext, ExecutionResult, Opcode, VerifierError,
};
use std::collections::HashMap;

#[test]
fn test_avm_arithmetic_execution() {
    // Bytecode:
    // PUSH1 20 (0x60 0x14)
    // PUSH1 10 (0x60 0x0A)
    // ADD      (0x01)
    // PUSH1 0  (0x60 0x00)
    // MSTORE   (0x52)
    // PUSH1 32 (0x60 0x20)
    // PUSH1 0  (0x60 0x00)
    // RETURN   (0xF3)
    let bytecode = vec![
        Opcode::Push1 as u8,
        20,
        Opcode::Push1 as u8,
        10,
        Opcode::Add as u8,
        Opcode::Push1 as u8,
        0,
        Opcode::MStore as u8,
        Opcode::Push1 as u8,
        32,
        Opcode::Push1 as u8,
        0,
        Opcode::Return as u8,
    ];

    let verified = BytecodeVerifier::verify(&bytecode).expect("Verification must pass");
    let caller = Address::from_bytes([1u8; 32]);
    let contract = Address::from_bytes([2u8; 32]);
    let ctx = ExecutionContext::new(caller, contract, caller, Quantum::ZERO, 100_000, 1, 1000);
    let storage = HashMap::new();

    let res = AvmEngine::execute(&verified, ctx, &storage);
    match res {
        ExecutionResult::Success {
            return_data,
            gas_used,
            ..
        } => {
            assert_eq!(return_data.len(), 32);
            // 20 + 10 = 30 = 0x1E di byte terakhir word 32-byte
            assert_eq!(return_data[31], 30);
            assert!(gas_used > 0);
        }
        other => panic!("Expected success, got {other:?}"),
    }
}

#[test]
fn test_avm_storage_sstore_sload() {
    // Bytecode:
    // PUSH1 42 (value)
    // PUSH1 1  (key)
    // SSTORE
    // PUSH1 1  (key)
    // SLOAD
    // PUSH1 0  (offset)
    // MSTORE
    // PUSH1 32 (len)
    // PUSH1 0  (offset)
    // RETURN
    let bytecode = vec![
        Opcode::Push1 as u8,
        42,
        Opcode::Push1 as u8,
        1,
        Opcode::SStore as u8,
        Opcode::Push1 as u8,
        1,
        Opcode::SLoad as u8,
        Opcode::Push1 as u8,
        0,
        Opcode::MStore as u8,
        Opcode::Push1 as u8,
        32,
        Opcode::Push1 as u8,
        0,
        Opcode::Return as u8,
    ];

    let verified = BytecodeVerifier::verify(&bytecode).expect("Verification must pass");
    let caller = Address::from_bytes([1u8; 32]);
    let contract = Address::from_bytes([2u8; 32]);
    let ctx = ExecutionContext::new(caller, contract, caller, Quantum::ZERO, 100_000, 1, 1000);
    let storage = HashMap::new();

    let res = AvmEngine::execute(&verified, ctx, &storage);
    match res {
        ExecutionResult::Success {
            return_data,
            storage_changes,
            ..
        } => {
            assert_eq!(return_data.len(), 32);
            assert_eq!(return_data[31], 42);

            let mut expected_key = [0u8; 32];
            expected_key[31] = 1;
            let mut expected_val = [0u8; 32];
            expected_val[31] = 42;

            assert_eq!(
                storage_changes.get(&Hash256::from_bytes(expected_key)),
                Some(&Hash256::from_bytes(expected_val))
            );
        }
        other => panic!("Expected success, got {other:?}"),
    }
}

#[test]
fn test_avm_blake3_syscall() {
    // Store 4 bytes "AUR0" ke memori 0..4
    // Blake3(offset=0, len=4) -> push hash ke stack
    // MStore hash ke offset 0
    // Return 32 bytes
    let bytecode = vec![
        Opcode::Push4 as u8,
        0x41,
        0x55,
        0x52,
        0x30,
        Opcode::Push1 as u8,
        0,
        Opcode::MStore as u8,
        Opcode::Push1 as u8,
        4,
        Opcode::Push1 as u8,
        28, // Offset 28 karena word 32 byte rata kanan
        Opcode::Blake3 as u8,
        Opcode::Push1 as u8,
        0,
        Opcode::MStore as u8,
        Opcode::Push1 as u8,
        32,
        Opcode::Push1 as u8,
        0,
        Opcode::Return as u8,
    ];

    let verified = BytecodeVerifier::verify(&bytecode).expect("Verification must pass");
    let caller = Address::from_bytes([1u8; 32]);
    let contract = Address::from_bytes([2u8; 32]);
    let ctx = ExecutionContext::new(caller, contract, caller, Quantum::ZERO, 100_000, 1, 1000);
    let storage = HashMap::new();

    let res = AvmEngine::execute(&verified, ctx, &storage);
    match res {
        ExecutionResult::Success { return_data, .. } => {
            let expected_hash = blake3_hash(b"AUR0");
            assert_eq!(return_data, expected_hash.as_bytes());
        }
        other => panic!("Expected success, got {other:?}"),
    }
}

#[test]
fn test_avm_gas_metering_out_of_gas() {
    let bytecode = vec![
        Opcode::Push1 as u8,
        1,
        Opcode::Push1 as u8,
        2,
        Opcode::Add as u8,
    ];
    let verified = BytecodeVerifier::verify(&bytecode).unwrap();
    let caller = Address::from_bytes([1u8; 32]);
    let contract = Address::from_bytes([2u8; 32]);

    // Berikan gas_limit terlalu kecil (misal 5 gas)
    let ctx = ExecutionContext::new(caller, contract, caller, Quantum::ZERO, 5, 1, 1000);
    let storage = HashMap::new();

    let res = AvmEngine::execute(&verified, ctx, &storage);
    assert_eq!(res, ExecutionResult::OutOfGas);
}

#[test]
fn test_avm_revert_semantics() {
    let bytecode = vec![
        Opcode::Push1 as u8,
        0,
        Opcode::Push1 as u8,
        0,
        Opcode::Revert as u8,
    ];
    let verified = BytecodeVerifier::verify(&bytecode).unwrap();
    let caller = Address::from_bytes([1u8; 32]);
    let contract = Address::from_bytes([2u8; 32]);
    let ctx = ExecutionContext::new(caller, contract, caller, Quantum::ZERO, 50_000, 1, 1000);
    let storage = HashMap::new();

    let res = AvmEngine::execute(&verified, ctx, &storage);
    match res {
        ExecutionResult::Revert { .. } => {}
        other => panic!("Expected Revert, got {other:?}"),
    }
}

#[test]
fn test_bytecode_verifier_rejection() {
    // 1. Empty bytecode
    assert_eq!(
        BytecodeVerifier::verify(&[]),
        Err(VerifierError::EmptyBytecode)
    );

    // 2. Illegal opcode
    assert!(matches!(
        BytecodeVerifier::verify(&[0xEE]),
        Err(VerifierError::InvalidOpcode { .. })
    ));

    // 3. Truncated PUSH
    assert!(matches!(
        BytecodeVerifier::verify(&[Opcode::Push4 as u8, 0x01, 0x02]),
        Err(VerifierError::TruncatedPush { .. })
    ));
}

#[test]
fn test_contract_deployment_and_state_transition() {
    let sender = Address::from_bytes([10u8; 32]);
    let miner = Address::from_bytes([30u8; 32]);

    let mut accounts = HashMap::new();
    accounts.insert(sender, Account::new(Quantum::new(1_000_000_000), 0));
    let mut monetary = MonetaryState::new(Quantum::new(1_000_000_000), Quantum::ZERO);

    let deploy_bytecode = vec![
        Opcode::Push1 as u8,
        100,
        Opcode::Push1 as u8,
        0,
        Opcode::SStore as u8,
        Opcode::Stop as u8,
    ];

    let tx = Transaction {
        version: 1,
        chain_id: 1001,
        tx_type: TxType::ContractDeploy,
        flags: 0,
        sender,
        recipient: Address::ZERO,
        nonce: 0,
        amount: Quantum::new(50_000_000),
        fee: Quantum::new(10_000_000),
        valid_until: 100,
        payload: deploy_bytecode,
        signature: Signature::from_bytes([0u8; 64]),
    };

    let receipt = apply_transaction(&mut accounts, &mut monetary, &miner, &tx)
        .expect("Contract deployment must succeed");

    let expected_contract_addr = derive_contract_address(&sender, 0);
    assert_eq!(receipt.deployed_contract, Some(expected_contract_addr));

    // Verifikasi akun kontrak ada di state
    let contract_acct = accounts
        .get(&expected_contract_addr)
        .expect("Contract account must exist");
    assert!(contract_acct.is_contract());
    assert_eq!(contract_acct.balance, Quantum::new(50_000_000));

    // Verifikasi saldo pengirim berkurang (50M amount + 10M fee = 60M)
    let sender_after = accounts.get(&sender).unwrap();
    assert_eq!(sender_after.balance, Quantum::new(940_000_000));
    assert_eq!(sender_after.nonce, 1);
}
