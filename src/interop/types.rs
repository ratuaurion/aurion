#![forbid(unsafe_code)]

//! Aurion Layer-4 (L4) Interoperability Domain: Core Types & Data Primitives.
//!
//! Complies strictly with:
//! - AUR-ARCH-011: Absolute Zero Unsafe Code.
//! - AUR-ARCH-012: Absolute Zero Float Arithmetic (Quantum u128 only).
//! - AUR-L4-ARCH-001: Sovereign Root Independence.
//! - AUR-L4-ARCH-002: Universal Cross-Domain Envelope & Canonical Types.
//! - AUR-L4-SEC-001: Fault Containment Boundary.

use crate::primitives::core::Quantum;
use blake3::Hasher;

/// Maximum payload size allowed for an L4 cross-chain packet (64 KB).
pub const MAX_L4_PAYLOAD_BYTES: usize = 65_536;

/// Supported external or internal chain identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChainId {
    AurionL1,
    AurionL2,
    AurionL3(u32),
    Bitcoin,
    Ethereum,
    CosmosIbc,
    Solana,
    ExternalCustom(u64),
}

impl ChainId {
    pub const fn to_u64(self) -> u64 {
        match self {
            Self::AurionL1 => 1,
            Self::AurionL2 => 2,
            Self::AurionL3(domain_id) => 0x1000_0000 + domain_id as u64,
            Self::Bitcoin => 10,
            Self::Ethereum => 60,
            Self::CosmosIbc => 118,
            Self::Solana => 501,
            Self::ExternalCustom(id) => id,
        }
    }

    pub const fn from_u64(val: u64) -> Self {
        match val {
            1 => Self::AurionL1,
            2 => Self::AurionL2,
            10 => Self::Bitcoin,
            60 => Self::Ethereum,
            118 => Self::CosmosIbc,
            501 => Self::Solana,
            other => {
                if other >= 0x1000_0000 && other < 0x2000_0000 {
                    Self::AurionL3((other - 0x1000_0000) as u32)
                } else {
                    Self::ExternalCustom(other)
                }
            }
        }
    }
}

/// Target interoperability protocol identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ProtocolId {
    SpvBitcoin = 0x01,
    EvmSyncCommittee = 0x02,
    CosmosIbc = 0x03,
    ZkRollup = 0x04,
    ThresholdVault = 0x05,
    NativeCrossLayer = 0x06,
    CustomProtocol = 0xFF,
}

impl ProtocolId {
    pub const fn from_u8(val: u8) -> Self {
        match val {
            0x01 => Self::SpvBitcoin,
            0x02 => Self::EvmSyncCommittee,
            0x03 => Self::CosmosIbc,
            0x04 => Self::ZkRollup,
            0x05 => Self::ThresholdVault,
            0x06 => Self::NativeCrossLayer,
            _ => Self::CustomProtocol,
        }
    }
}

/// Formal Channel & Bridge Operational Lifecycle Status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum BridgeStatus {
    Proposed = 0x00,
    Verified = 0x01,
    Active = 0x02,
    Rebalancing = 0x03,
    CircuitBroken = 0x04,
    Paused = 0x05,
    DrainProtection = 0x06,
    Terminated = 0x07,
}

impl BridgeStatus {
    pub const fn from_u8(val: u8) -> Self {
        match val {
            0x00 => Self::Proposed,
            0x01 => Self::Verified,
            0x02 => Self::Active,
            0x03 => Self::Rebalancing,
            0x04 => Self::CircuitBroken,
            0x05 => Self::Paused,
            0x06 => Self::DrainProtection,
            _ => Self::Terminated,
        }
    }

    pub const fn can_process_transfers(self) -> bool {
        matches!(self, Self::Active | Self::Rebalancing)
    }
}

/// Route descriptor for cross-domain dispatching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDescriptor {
    pub source_chain: ChainId,
    pub destination_chain: ChainId,
    pub protocol: ProtocolId,
    pub hop_count: u8,
    pub gas_limit: u64,
    pub max_fee: Quantum,
}

impl RouteDescriptor {
    pub fn new(
        source_chain: ChainId,
        destination_chain: ChainId,
        protocol: ProtocolId,
        gas_limit: u64,
        max_fee: Quantum,
    ) -> Self {
        Self {
            source_chain,
            destination_chain,
            protocol,
            hop_count: 1,
            gas_limit,
            max_fee,
        }
    }
}

/// Cryptographic proof payload attached to a cross-chain packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofPayload {
    None,
    MerkleInclusion(Vec<[u8; 32]>),
    ZkSnark(Vec<u8>),
    ThresholdSignature(Vec<u8>),
    MultiProverAttestation {
        light_client_verified: bool,
        zk_proof: Vec<u8>,
        signatures: Vec<[u8; 64]>,
    },
}

impl ProofPayload {
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Parameters required to construct a new CrossChainMessage envelope.
#[derive(Debug, Clone)]
pub struct CrossChainMessageParams {
    pub source_chain: ChainId,
    pub destination_chain: ChainId,
    pub sequence_nonce: u64,
    pub sender: [u8; 32],
    pub target_contract: [u8; 32],
    pub payload: Vec<u8>,
    pub timeout_timestamp: u64,
    pub protocol: ProtocolId,
    pub gas_limit: u64,
    pub max_fee: Quantum,
    pub proof: ProofPayload,
}

/// Universal 8-field Cross-Chain Message Envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossChainMessage {
    pub packet_id: [u8; 32],
    pub source_chain: ChainId,
    pub destination_chain: ChainId,
    pub sequence_nonce: u64,
    pub sender: [u8; 32],
    pub target_contract: [u8; 32],
    pub payload: Vec<u8>,
    pub timeout_timestamp: u64,
    pub route: RouteDescriptor,
    pub proof: ProofPayload,
}

impl CrossChainMessage {
    /// Creates and deterministically computes the `packet_id` for an envelope from parameters.
    pub fn new(params: CrossChainMessageParams) -> Result<Self, &'static str> {
        if params.payload.len() > MAX_L4_PAYLOAD_BYTES {
            return Err("Payload exceeds MAX_L4_PAYLOAD_BYTES (64 KB)");
        }

        let route = RouteDescriptor::new(
            params.source_chain,
            params.destination_chain,
            params.protocol,
            params.gas_limit,
            params.max_fee,
        );

        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L4-PACKET-V1");
        hasher.update(&params.source_chain.to_u64().to_be_bytes());
        hasher.update(&params.destination_chain.to_u64().to_be_bytes());
        hasher.update(&params.sequence_nonce.to_be_bytes());
        hasher.update(&params.sender);
        hasher.update(&params.target_contract);
        hasher.update(&(params.payload.len() as u32).to_be_bytes());
        hasher.update(&params.payload);
        hasher.update(&params.timeout_timestamp.to_be_bytes());
        hasher.update(&[params.protocol as u8]);

        let packet_id = *hasher.finalize().as_bytes();

        Ok(Self {
            packet_id,
            source_chain: params.source_chain,
            destination_chain: params.destination_chain,
            sequence_nonce: params.sequence_nonce,
            sender: params.sender,
            target_contract: params.target_contract,
            payload: params.payload,
            timeout_timestamp: params.timeout_timestamp,
            route,
            proof: params.proof,
        })
    }

    /// Verifies that the internal `packet_id` strictly matches the Blake3 digest of its contents.
    pub fn verify_integrity(&self) -> bool {
        if self.payload.len() > MAX_L4_PAYLOAD_BYTES {
            return false;
        }

        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L4-PACKET-V1");
        hasher.update(&self.source_chain.to_u64().to_be_bytes());
        hasher.update(&self.destination_chain.to_u64().to_be_bytes());
        hasher.update(&self.sequence_nonce.to_be_bytes());
        hasher.update(&self.sender);
        hasher.update(&self.target_contract);
        hasher.update(&(self.payload.len() as u32).to_be_bytes());
        hasher.update(&self.payload);
        hasher.update(&self.timeout_timestamp.to_be_bytes());
        hasher.update(&[self.route.protocol as u8]);

        let computed = *hasher.finalize().as_bytes();
        computed == self.packet_id
    }

    /// Computes the unique anti-replay nullifier for this message.
    pub fn compute_nullifier(&self) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L4-NULLIFIER-V1");
        hasher.update(&self.packet_id);
        hasher.update(&self.destination_chain.to_u64().to_be_bytes());
        hasher.update(&self.sequence_nonce.to_be_bytes());
        *hasher.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_id_roundtrip() {
        let chains = [
            ChainId::AurionL1,
            ChainId::AurionL2,
            ChainId::AurionL3(42),
            ChainId::Bitcoin,
            ChainId::Ethereum,
            ChainId::CosmosIbc,
            ChainId::Solana,
            ChainId::ExternalCustom(9999),
        ];

        for chain in chains {
            let u = chain.to_u64();
            let recovered = ChainId::from_u64(u);
            assert_eq!(chain, recovered);
        }
    }

    #[test]
    fn test_cross_chain_message_integrity_and_nullifier() {
        let params = CrossChainMessageParams {
            source_chain: ChainId::AurionL1,
            destination_chain: ChainId::Ethereum,
            sequence_nonce: 101,
            sender: [1u8; 32],
            target_contract: [2u8; 32],
            payload: vec![0xAA, 0xBB, 0xCC],
            timeout_timestamp: 1_800_000_000,
            protocol: ProtocolId::EvmSyncCommittee,
            gas_limit: 500_000,
            max_fee: Quantum::new(100_000_000),
            proof: ProofPayload::None,
        };
        let msg = CrossChainMessage::new(params).expect("Message creation should succeed");

        assert!(msg.verify_integrity());
        let nullifier = msg.compute_nullifier();
        assert_ne!(nullifier, [0u8; 32]);
    }

    #[test]
    fn test_cross_chain_message_payload_overflow_rejected() {
        let oversized = vec![0u8; MAX_L4_PAYLOAD_BYTES + 1];
        let params = CrossChainMessageParams {
            source_chain: ChainId::Bitcoin,
            destination_chain: ChainId::AurionL1,
            sequence_nonce: 1,
            sender: [0u8; 32],
            target_contract: [0u8; 32],
            payload: oversized,
            timeout_timestamp: 1_800_000_000,
            protocol: ProtocolId::SpvBitcoin,
            gas_limit: 10_000,
            max_fee: Quantum::new(100_000_000),
            proof: ProofPayload::None,
        };
        let res = CrossChainMessage::new(params);

        assert!(res.is_err());
    }
}
