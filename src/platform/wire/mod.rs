//! Modul Protokol Wire Jaringan P2P Aurion.

pub mod frame;
pub mod handshake;
pub mod messages;
pub mod peer;
pub mod zenoh_transport;

pub use zenoh_transport::{AurionKeyExpressions, TransportConfig, TransportError, ZenohTransport};

pub use frame::{
    parse_network_frame, serialize_network_frame, WireError, WireFrameHeader,
    MAX_WIRE_PAYLOAD_BYTES, WIRE_FRAME_HEADER_BYTES, WIRE_MAGIC,
};
pub use handshake::{
    validate_handshake_ack, validate_handshake_hello, HandshakeAck, HandshakeError,
    HandshakeHello, DST_HANDSHAKE_ACK, DST_HANDSHAKE_HELLO, HANDSHAKE_STATUS_FAILED,
    HANDSHAKE_STATUS_SUCCESS, MAX_CLOCK_DRIFT_SECS, PROTOCOL_VERSION_V1,
};
pub use messages::*;
pub use peer::{
    PeerRecord, PeerState, PeerViolation, TokenBucket, BAN_DURATION_SECS, BAN_SCORE_THRESHOLD,
    INITIAL_PEER_SCORE, MAX_SYNC_MSGS_PER_SEC, MAX_TX_MSGS_PER_SEC, THROTTLE_SCORE_THRESHOLD,
};
