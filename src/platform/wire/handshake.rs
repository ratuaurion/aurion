//! Protokol Handshake Kanonikal dan Autentikasi Node P2P Aurion.
//! Mematuhi Dokumen 06 (AURION-SERIALIZATION-AND-WIRE-PROTOCOL.md Bagian 7).

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::core::{Address, Hash256, Signature};
use crate::crypto::{blake3_hash, derive_address_from_pubkey, ed25519_verify_strict, Keypair};
use thiserror::Error;

pub const PROTOCOL_VERSION_V1: u32 = 1;
pub const MAX_CLOCK_DRIFT_SECS: u64 = 15;

pub const DST_HANDSHAKE_HELLO: &[u8] = b"AURION-HANDSHAKE-HELLO";
pub const DST_HANDSHAKE_ACK: &[u8] = b"AURION-HANDSHAKE-ACK";

pub const HANDSHAKE_STATUS_SUCCESS: u8 = 0x01;
pub const HANDSHAKE_STATUS_FAILED: u8 = 0x00;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HandshakeError {
    #[error("Incompatible protocol version: expected {expected}, got {received}")]
    VersionMismatch { expected: u32, received: u32 },
    #[error("Incompatible chain ID: expected {expected}, got {received}")]
    ChainIdMismatch { expected: u32, received: u32 },
    #[error("Incompatible genesis hash: peer belongs to different network")]
    GenesisMismatch,
    #[error("Peer clock drift exceeded: |peer {peer_ts} - local {local_ts}| > {limit}s")]
    ClockDriftExceeded {
        peer_ts: u64,
        local_ts: u64,
        limit: u64,
    },
    #[error("Derived node address does not match declared node ID")]
    NodeIdMismatch,
    #[error("Cryptographic signature verification failed: forged or invalid handshake")]
    InvalidSignature,
    #[error("Peer responded with failure status: {0}")]
    HandshakeRejected(u8),
    #[error("Codec error: {0}")]
    Codec(#[from] CodecError),
}

/// Pesan inisiasi handshake antar-simpul (Ukuran tepat 184 Bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeHello {
    pub protocol_version: u32,
    pub chain_id: u32,
    pub genesis_hash: Hash256,
    pub best_height: u64,
    pub timestamp: u64,
    pub node_id: Address,
    pub public_key: [u8; 32],
    pub signature: Signature,
}

impl HandshakeHello {
    pub fn compute_preimage(
        protocol_version: u32,
        chain_id: u32,
        genesis_hash: &Hash256,
        best_height: u64,
        timestamp: u64,
        node_id: &Address,
        public_key: &[u8; 32],
    ) -> Hash256 {
        let mut data = Vec::with_capacity(124);
        data.extend_from_slice(DST_HANDSHAKE_HELLO);
        protocol_version.encode_canonical(&mut data);
        chain_id.encode_canonical(&mut data);
        genesis_hash.encode_canonical(&mut data);
        best_height.encode_canonical(&mut data);
        timestamp.encode_canonical(&mut data);
        node_id.encode_canonical(&mut data);
        data.extend_from_slice(public_key);
        blake3_hash(&data)
    }

    pub fn new(
        chain_id: u32,
        genesis_hash: Hash256,
        best_height: u64,
        timestamp: u64,
        keypair: &Keypair,
    ) -> Self {
        let public_key = keypair.public_key_bytes();
        let node_id = derive_address_from_pubkey(&public_key);
        let preimage_hash = Self::compute_preimage(
            PROTOCOL_VERSION_V1,
            chain_id,
            &genesis_hash,
            best_height,
            timestamp,
            &node_id,
            &public_key,
        );
        let signature = keypair.sign(preimage_hash.as_bytes());

        Self {
            protocol_version: PROTOCOL_VERSION_V1,
            chain_id,
            genesis_hash,
            best_height,
            timestamp,
            node_id,
            public_key,
            signature,
        }
    }
}

impl CanonicalEncode for HandshakeHello {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        self.protocol_version.encode_canonical(buf);
        self.chain_id.encode_canonical(buf);
        self.genesis_hash.encode_canonical(buf);
        self.best_height.encode_canonical(buf);
        self.timestamp.encode_canonical(buf);
        self.node_id.encode_canonical(buf);
        buf.extend_from_slice(&self.public_key);
        self.signature.encode_canonical(buf);
    }
}

impl CanonicalDecode for HandshakeHello {
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let protocol_version = u32::decode_canonical(bytes, cursor)?;
        let chain_id = u32::decode_canonical(bytes, cursor)?;
        let genesis_hash = Hash256::decode_canonical(bytes, cursor)?;
        let best_height = u64::decode_canonical(bytes, cursor)?;
        let timestamp = u64::decode_canonical(bytes, cursor)?;
        let node_id = Address::decode_canonical(bytes, cursor)?;

        let mut public_key = [0u8; 32];
        if bytes.len().saturating_sub(*cursor) < 32 {
            return Err(CodecError::UnexpectedEof {
                needed: 32,
                available: bytes.len().saturating_sub(*cursor),
            });
        }
        public_key.copy_from_slice(&bytes[*cursor..*cursor + 32]);
        *cursor += 32;

        let signature = Signature::decode_canonical(bytes, cursor)?;

        Ok(HandshakeHello {
            protocol_version,
            chain_id,
            genesis_hash,
            best_height,
            timestamp,
            node_id,
            public_key,
            signature,
        })
    }
}

/// Pesan balasan handshake antar-simpul (Ukuran tepat 145 Bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeAck {
    pub status: u8,
    pub best_height: u64,
    pub timestamp: u64,
    pub node_id: Address,
    pub public_key: [u8; 32],
    pub signature: Signature,
}

impl HandshakeAck {
    pub fn compute_preimage(
        status: u8,
        best_height: u64,
        timestamp: u64,
        node_id: &Address,
        public_key: &[u8; 32],
    ) -> Hash256 {
        let mut data = Vec::with_capacity(100);
        data.extend_from_slice(DST_HANDSHAKE_ACK);
        status.encode_canonical(&mut data);
        best_height.encode_canonical(&mut data);
        timestamp.encode_canonical(&mut data);
        node_id.encode_canonical(&mut data);
        data.extend_from_slice(public_key);
        blake3_hash(&data)
    }

    pub fn new(best_height: u64, timestamp: u64, keypair: &Keypair) -> Self {
        let public_key = keypair.public_key_bytes();
        let node_id = derive_address_from_pubkey(&public_key);
        let preimage_hash = Self::compute_preimage(
            HANDSHAKE_STATUS_SUCCESS,
            best_height,
            timestamp,
            &node_id,
            &public_key,
        );
        let signature = keypair.sign(preimage_hash.as_bytes());

        Self {
            status: HANDSHAKE_STATUS_SUCCESS,
            best_height,
            timestamp,
            node_id,
            public_key,
            signature,
        }
    }
}

impl CanonicalEncode for HandshakeAck {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        self.status.encode_canonical(buf);
        self.best_height.encode_canonical(buf);
        self.timestamp.encode_canonical(buf);
        self.node_id.encode_canonical(buf);
        buf.extend_from_slice(&self.public_key);
        self.signature.encode_canonical(buf);
    }
}

impl CanonicalDecode for HandshakeAck {
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let status = u8::decode_canonical(bytes, cursor)?;
        let best_height = u64::decode_canonical(bytes, cursor)?;
        let timestamp = u64::decode_canonical(bytes, cursor)?;
        let node_id = Address::decode_canonical(bytes, cursor)?;

        let mut public_key = [0u8; 32];
        if bytes.len().saturating_sub(*cursor) < 32 {
            return Err(CodecError::UnexpectedEof {
                needed: 32,
                available: bytes.len().saturating_sub(*cursor),
            });
        }
        public_key.copy_from_slice(&bytes[*cursor..*cursor + 32]);
        *cursor += 32;

        let signature = Signature::decode_canonical(bytes, cursor)?;

        Ok(HandshakeAck {
            status,
            best_height,
            timestamp,
            node_id,
            public_key,
            signature,
        })
    }
}

/// Validasi menyeluruh pesan HANDSHAKE_HELLO yang diterima dari peer.
pub fn validate_handshake_hello(
    hello: &HandshakeHello,
    expected_chain_id: u32,
    expected_genesis: &Hash256,
    local_timestamp: u64,
) -> Result<(), HandshakeError> {
    if hello.protocol_version != PROTOCOL_VERSION_V1 {
        return Err(HandshakeError::VersionMismatch {
            expected: PROTOCOL_VERSION_V1,
            received: hello.protocol_version,
        });
    }

    if hello.chain_id != expected_chain_id {
        return Err(HandshakeError::ChainIdMismatch {
            expected: expected_chain_id,
            received: hello.chain_id,
        });
    }

    if hello.genesis_hash != *expected_genesis {
        return Err(HandshakeError::GenesisMismatch);
    }

    if hello.timestamp.abs_diff(local_timestamp) > MAX_CLOCK_DRIFT_SECS {
        return Err(HandshakeError::ClockDriftExceeded {
            peer_ts: hello.timestamp,
            local_ts: local_timestamp,
            limit: MAX_CLOCK_DRIFT_SECS,
        });
    }

    let expected_address = derive_address_from_pubkey(&hello.public_key);
    if hello.node_id != expected_address {
        return Err(HandshakeError::NodeIdMismatch);
    }

    let preimage_hash = HandshakeHello::compute_preimage(
        hello.protocol_version,
        hello.chain_id,
        &hello.genesis_hash,
        hello.best_height,
        hello.timestamp,
        &hello.node_id,
        &hello.public_key,
    );

    if ed25519_verify_strict(
        &hello.public_key,
        preimage_hash.as_bytes(),
        &hello.signature,
    )
    .is_err()
    {
        return Err(HandshakeError::InvalidSignature);
    }

    Ok(())
}

/// Validasi menyeluruh pesan HANDSHAKE_ACK yang diterima dari peer penerima.
pub fn validate_handshake_ack(
    ack: &HandshakeAck,
    local_timestamp: u64,
) -> Result<(), HandshakeError> {
    if ack.status != HANDSHAKE_STATUS_SUCCESS {
        return Err(HandshakeError::HandshakeRejected(ack.status));
    }

    if ack.timestamp.abs_diff(local_timestamp) > MAX_CLOCK_DRIFT_SECS {
        return Err(HandshakeError::ClockDriftExceeded {
            peer_ts: ack.timestamp,
            local_ts: local_timestamp,
            limit: MAX_CLOCK_DRIFT_SECS,
        });
    }

    let expected_address = derive_address_from_pubkey(&ack.public_key);
    if ack.node_id != expected_address {
        return Err(HandshakeError::NodeIdMismatch);
    }

    let preimage_hash = HandshakeAck::compute_preimage(
        ack.status,
        ack.best_height,
        ack.timestamp,
        &ack.node_id,
        &ack.public_key,
    );

    if ed25519_verify_strict(&ack.public_key, preimage_hash.as_bytes(), &ack.signature).is_err() {
        return Err(HandshakeError::InvalidSignature);
    }

    Ok(())
}
