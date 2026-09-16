//! Error Model Kanonikal Aurion.
//! Memetakan error kode mesin terstruktur (1000-6999) sesuai Dokumen 10.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AurionError {
    #[error("Monetary Error: {0}")]
    Monetary(#[from] crate::core::MonetaryError),

    #[error("Crypto Error: {0}")]
    Crypto(String),

    #[error("Codec Error: {0}")]
    Codec(String),

    #[error("Transaction Error: {0}")]
    Transaction(String),

    #[error("State Error: {0}")]
    State(String),

    #[error("Consensus Error: {0}")]
    Consensus(String),

    #[error("Wire Error: {0}")]
    Wire(String),
}
