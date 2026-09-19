//! Modul Serialisasi dan Deserialisasi Kanonikal Aurion.

pub mod decode;
pub mod encode;

pub use decode::{
    decode_length_prefixed_bytes, CanonicalDecode, CodecError, MAX_ALLOWED_ALLOCATION_BYTES,
};
pub use encode::CanonicalEncode;
