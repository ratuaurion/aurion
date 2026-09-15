//! Modul Serialisasi dan Deserialisasi Kanonikal Aurion.

pub mod decode;
pub mod encode;

pub use decode::{CanonicalDecode, CodecError, MAX_ALLOWED_ALLOCATION_BYTES};
pub use encode::CanonicalEncode;
