//! Modul Protokol Wire Jaringan P2P Aurion.

pub mod frame;
pub mod messages;

pub use frame::{
    parse_network_frame, serialize_network_frame, WireError, WireFrameHeader,
    MAX_WIRE_PAYLOAD_BYTES, WIRE_FRAME_HEADER_BYTES, WIRE_MAGIC,
};
pub use messages::*;
