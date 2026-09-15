//! State Transition Function (STF) deterministik Aurion: σ' = Υ(σ, B).

use crate::core::{Address, Quantum};
use crate::state::account::Account;
use crate::state::monetary::MonetaryState;
use crate::transaction::types::Transaction;
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
}

/// Eksekusi transisi state atomik untuk satu transaksi.
pub fn apply_transaction(
    accounts: &mut HashMap<Address, Account>,
    monetary: &mut MonetaryState,
    miner: &Address,
    tx: &Transaction,
) -> Result<(Quantum, Quantum), StateTransitionError> {
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
        },
    );

    // 2. Tambah saldo penerima
    let recipient_acct = accounts.entry(tx.recipient).or_default();
    let new_recipient_balance = recipient_acct
        .balance
        .checked_add(tx.amount)
        .map_err(|e| StateTransitionError::Monetary(e.to_string()))?;
    recipient_acct.balance = new_recipient_balance;

    // 3. Alokasikan fee transaksi (20% burn, 80% miner)
    let (burn_amt, miner_amt) = MonetaryState::split_fee(tx.fee)
        .map_err(|e| StateTransitionError::Monetary(e.to_string()))?;

    monetary
        .apply_burn(burn_amt)
        .map_err(|e| StateTransitionError::Monetary(e.to_string()))?;

    let miner_acct = accounts.entry(*miner).or_default();
    let new_miner_balance = miner_acct
        .balance
        .checked_add(miner_amt)
        .map_err(|e| StateTransitionError::Monetary(e.to_string()))?;
    miner_acct.balance = new_miner_balance;

    Ok((burn_amt, miner_amt))
}
