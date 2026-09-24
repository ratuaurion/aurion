//! State Transition Function (STF) deterministik Aurion: σ' = Υ(σ, B).
//! Mematuhi Konstitusi Konsensus dan Aturan Aplikasi 16 (Smart Contract Execution).

use crate::core::{Address, Hash256, Quantum};
use crate::crypto::{blake3_derive_key, blake3_hash};
use crate::state::account::Account;
use crate::state::monetary::MonetaryState;
use crate::transaction::types::{Transaction, TxType};
use crate::vm::context::ExecutionContext;
use crate::vm::engine::{AvmEngine, ExecutionResult};
use crate::vm::verifier::BytecodeVerifier;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StateTransitionError {
    #[error("Account not found: {0}")]
    AccountNotFound(String),
    #[error("Insufficient balance for transaction: required {required}, available {available}")]
    InsufficientBalance {
        required: Quantum,
        available: Quantum,
    },
    #[error("Invalid transaction nonce: expected {expected}, got {got}")]
    InvalidNonce { expected: u64, got: u64 },
    #[error("Monetary operation failed: {0}")]
    Monetary(String),
    #[error("Contract bytecode verification failed: {0}")]
    InvalidContractBytecode(String),
    #[error("Contract execution failed: {0}")]
    ContractExecutionFailed(String),
    #[error("Target recipient is not a smart contract: {0}")]
    NotAContract(String),
}

/// Derivasi alamat kontrak deterministik dari alamat sender dan nonce.
pub fn derive_contract_address(sender: &Address, nonce: u64) -> Address {
    let mut data = Vec::with_capacity(40);
    data.extend_from_slice(sender.as_bytes());
    data.extend_from_slice(&nonce.to_be_bytes());
    let digest = blake3_derive_key("AURION-CONTRACT-ADDRESS-V1", &data);
    Address(digest.0)
}

/// Hasil eksekusi transaksi yang mencakup biaya fee split dan data kontrak opsional.
#[derive(Debug, Clone)]
pub struct TransactionExecutionReceipt {
    pub burned_fee: Quantum,
    pub miner_fee: Quantum,
    pub validator_fee: Quantum,
    pub deployed_contract: Option<Address>,
    pub return_data: Vec<u8>,
    pub storage_changes: HashMap<Hash256, Hash256>,
}

/// Eksekusi transisi state atomik untuk satu transaksi.
pub fn apply_transaction(
    accounts: &mut HashMap<Address, Account>,
    monetary: &mut MonetaryState,
    proposer: &Address,
    tx: &Transaction,
) -> Result<TransactionExecutionReceipt, StateTransitionError> {
    let sender_acct = accounts
        .get(&tx.sender)
        .cloned()
        .ok_or_else(|| StateTransitionError::AccountNotFound(tx.sender.to_hex()))?;

    if tx.nonce != sender_acct.nonce {
        return Err(StateTransitionError::InvalidNonce {
            expected: sender_acct.nonce,
            got: tx.nonce,
        });
    }

    let total_cost = tx
        .amount
        .checked_add(tx.fee)
        .map_err(|e| StateTransitionError::Monetary(e.to_string()))?;

    if sender_acct.balance < total_cost {
        return Err(StateTransitionError::InsufficientBalance {
            required: total_cost,
            available: sender_acct.balance,
        });
    }

    // 1. Kurangi saldo pengirim & tingkatkan nonce
    let new_sender_balance = sender_acct
        .balance
        .checked_sub(total_cost)
        .map_err(|e| StateTransitionError::Monetary(e.to_string()))?;

    accounts.insert(
        tx.sender,
        Account {
            balance: new_sender_balance,
            nonce: sender_acct.nonce + 1,
            code_hash: sender_acct.code_hash,
            storage_root: sender_acct.storage_root,
        },
    );

    // 2. Alokasikan fee transaksi: 100% dialokasikan ke validator/proposer pembuat blok (0% burn)
    let (burn_amt, validator_fee) = MonetaryState::split_fee(tx.fee)
        .map_err(|e| StateTransitionError::Monetary(e.to_string()))?;

    if !burn_amt.is_zero() {
        monetary
            .apply_burn(burn_amt)
            .map_err(|e| StateTransitionError::Monetary(e.to_string()))?;
    }

    let proposer_acct = accounts.entry(*proposer).or_default();
    let new_proposer_balance = proposer_acct
        .balance
        .checked_add(validator_fee)
        .map_err(|e| StateTransitionError::Monetary(e.to_string()))?;
    proposer_acct.balance = new_proposer_balance;

    // 3. Eksekusi spesifik tipe transaksi
    match tx.tx_type {
        TxType::Transfer | TxType::Stake | TxType::Unstake | TxType::GovernanceVote => {
            let recipient_acct = accounts.entry(tx.recipient).or_default();
            let new_recipient_balance = recipient_acct
                .balance
                .checked_add(tx.amount)
                .map_err(|e| StateTransitionError::Monetary(e.to_string()))?;
            recipient_acct.balance = new_recipient_balance;

            Ok(TransactionExecutionReceipt {
                burned_fee: burn_amt,
                miner_fee: validator_fee,
                validator_fee,
                deployed_contract: None,
                return_data: Vec::new(),
                storage_changes: HashMap::new(),
            })
        }

        TxType::ContractDeploy => {
            // Verifikasi bytecode statis
            let verified = BytecodeVerifier::verify(&tx.payload)
                .map_err(|e| StateTransitionError::InvalidContractBytecode(e.to_string()))?;

            let contract_addr = derive_contract_address(&tx.sender, sender_acct.nonce);
            let code_hash = blake3_hash(&tx.payload);

            // Eksekusi konstruktor init bytecode
            let ctx = ExecutionContext::new(
                tx.sender,
                contract_addr,
                tx.sender,
                tx.amount,
                1_000_000, // Default deploy gas limit
                0,
                0,
            );
            let empty_storage = HashMap::new();
            let exec_res = AvmEngine::execute(&verified, ctx, &empty_storage);

            match exec_res {
                ExecutionResult::Success {
                    return_data,
                    storage_changes,
                    ..
                } => {
                    // Buat akun kontrak baru dengan saldo awal yang ditransfer
                    accounts.insert(
                        contract_addr,
                        Account::new_contract(tx.amount, 0, code_hash, Hash256::ZERO),
                    );

                    Ok(TransactionExecutionReceipt {
                        burned_fee: burn_amt,
                        miner_fee: validator_fee,
                        validator_fee,
                        deployed_contract: Some(contract_addr),
                        return_data,
                        storage_changes,
                    })
                }
                ExecutionResult::Revert { reason, .. } => {
                    Err(StateTransitionError::ContractExecutionFailed(format!("Deployment reverted: {reason}")))
                }
                ExecutionResult::OutOfGas => {
                    Err(StateTransitionError::ContractExecutionFailed("Deployment out of gas".to_string()))
                }
                ExecutionResult::Error(e) => {
                    Err(StateTransitionError::ContractExecutionFailed(e))
                }
            }
        }

        TxType::ContractCall => {
            let recipient_acct = accounts
                .get(&tx.recipient)
                .cloned()
                .ok_or_else(|| StateTransitionError::AccountNotFound(tx.recipient.to_hex()))?;

            if !recipient_acct.is_contract() {
                return Err(StateTransitionError::NotAContract(tx.recipient.to_hex()));
            }

            // Transfer amount ke kontrak
            let recipient_mut = accounts.entry(tx.recipient).or_default();
            recipient_mut.balance = recipient_mut
                .balance
                .checked_add(tx.amount)
                .map_err(|e| StateTransitionError::Monetary(e.to_string()))?;

            // Verifikasi bytecode dari payload panggilan (atau eksekusi call)
            let verified = BytecodeVerifier::verify(&tx.payload)
                .map_err(|e| StateTransitionError::InvalidContractBytecode(e.to_string()))?;

            let ctx = ExecutionContext::new(
                tx.sender,
                tx.recipient,
                tx.sender,
                tx.amount,
                1_000_000,
                0,
                0,
            );
            let empty_storage = HashMap::new();
            let exec_res = AvmEngine::execute(&verified, ctx, &empty_storage);

            match exec_res {
                ExecutionResult::Success {
                    return_data,
                    storage_changes,
                    ..
                } => Ok(TransactionExecutionReceipt {
                    burned_fee: burn_amt,
                    miner_fee: validator_fee,
                    validator_fee,
                    deployed_contract: None,
                    return_data,
                    storage_changes,
                }),
                ExecutionResult::Revert { reason, .. } => {
                    Err(StateTransitionError::ContractExecutionFailed(format!("Call reverted: {reason}")))
                }
                ExecutionResult::OutOfGas => {
                    Err(StateTransitionError::ContractExecutionFailed("Call out of gas".to_string()))
                }
                ExecutionResult::Error(e) => {
                    Err(StateTransitionError::ContractExecutionFailed(e))
                }
            }
        }
    }
}
