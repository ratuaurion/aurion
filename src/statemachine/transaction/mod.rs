//! Modul Transaksi Kanonikal Aurion.

pub mod builder;
pub mod types;
pub mod validator;

pub use types::{
    transaction_wire_size, Transaction, TxType, MAX_TRANSACTION_PAYLOAD_BYTES,
    MAX_TRANSACTION_WIRE_BYTES, TRANSACTION_BASE_BYTES, TRANSACTION_LENGTH_PREFIX_BYTES,
};
pub use validator::{validate_transaction_stateless, TransactionValidationError};
