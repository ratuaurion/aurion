//! Modul penyimpanan state, blok, dan sertifikat rantai blok Aurion.
//! Mengikuti arsitektur StateStore decoupled dengan engine fisik murni Rust `redb`.

pub mod memory_engine;
pub mod redb_engine;
pub mod traits;

pub use memory_engine::MemoryStorageEngine;
pub use redb_engine::RedbStorageEngine;
pub use traits::{StateStore, StorageError};
