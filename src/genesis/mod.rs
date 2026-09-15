//! Modul Blok dan State Genesis Aurion.

pub mod builder;

pub use builder::{
    build_genesis, GenesisInitialization, GENESIS_CHAIN_ID, GENESIS_TIMESTAMP,
};
