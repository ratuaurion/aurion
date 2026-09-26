#![forbid(unsafe_code)]

//! Primitif Data Kanonikal Aurion Layer-5 (L5) Global Distributed Infrastructure.
//! Mematuhi Dokumen Aturan Aplikasi 20 (20-L5-GLOBAL-INFRASTRUCTURE-BLUEPRINT.md).
//! Invariant: AUR-L5-ARCH-001 (Non-Consensus), AUR-L5-PREC-001 (Zero-Float Quantum), AUR-L5-DATA-001 (Blake3).

use crate::primitives::core::Quantum;
use blake3::Hasher;

/// Ukuran chunk kanonikal untuk content-addressed storage (64 KB).
pub const L5_STORAGE_CHUNK_BYTES: usize = 65_536;

/// Jaminan minimum (collateral) untuk node infrastruktur L5 (1.000 AUR = 100.000.000.000 Quanta).
pub const L5_MIN_NODE_COLLATERAL_QUANTA: u128 = 100_000_000_000;

/// Masa tenggang unbonding jaminan (1.000 slot/blok).
pub const L5_UNBONDING_DELAY_SLOTS: u64 = 1_000;

/// Jendela sanggahan audit challenge (100 slot/blok).
pub const L5_CHALLENGE_WINDOW_SLOTS: u64 = 100;

/// Batas maksimum kanal streaming aktif per penyedia layanan.
pub const L5_MAX_STREAMING_CHANNELS: usize = 10_000;

/// Batas maksimum spending cap agen AI default (10.000 AUR).
pub const L5_DEFAULT_MAX_AGENT_CAP_QUANTA: u128 = 1_000_000_000_000;

/// Pengenal 32-byte node infrastruktur berbasis Blake3 dari Ed25519 public key.
pub type InfrastructureNodeId = [u8; 32];

/// Menghitung NodeId deterministik dari kunci publik Ed25519.
pub fn compute_node_id(pubkey: &[u8; 32]) -> InfrastructureNodeId {
    let mut hasher = Hasher::new();
    hasher.update(b"AURION-L5-NODE-ID-V1");
    hasher.update(pubkey);
    *hasher.finalize().as_bytes()
}

/// Klasifikasi Jenis Peran Node Infrastruktur L5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum NodeType {
    ComputeWorker = 0x01,
    StorageKeeper = 0x02,
    DasNode = 0x03,
    Indexer = 0x04,
    GatewayRelay = 0x05,
    M2MOrchestrator = 0x06,
}

impl NodeType {
    pub const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x01 => Some(Self::ComputeWorker),
            0x02 => Some(Self::StorageKeeper),
            0x03 => Some(Self::DasNode),
            0x04 => Some(Self::Indexer),
            0x05 => Some(Self::GatewayRelay),
            0x06 => Some(Self::M2MOrchestrator),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ComputeWorker => "ComputeWorker",
            Self::StorageKeeper => "StorageKeeper",
            Self::DasNode => "DasNode",
            Self::Indexer => "Indexer",
            Self::GatewayRelay => "GatewayRelay",
            Self::M2MOrchestrator => "M2MOrchestrator",
        }
    }
}

/// Siklus Hidup Formal Node Infrastruktur L5 (Aturan 20 Bagian 4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum NodeLifecycleStatus {
    Initializing = 0x00,
    Discovery = 0x01,
    Collateralized = 0x02,
    ActiveNode = 0x03,
    Serving = 0x04,
    Auditing = 0x05,
    Challenged = 0x06,
    Slashed = 0x07,
    Unbonding = 0x08,
    Retired = 0x09,
}

impl NodeLifecycleStatus {
    pub const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x00 => Some(Self::Initializing),
            0x01 => Some(Self::Discovery),
            0x02 => Some(Self::Collateralized),
            0x03 => Some(Self::ActiveNode),
            0x04 => Some(Self::Serving),
            0x05 => Some(Self::Auditing),
            0x06 => Some(Self::Challenged),
            0x07 => Some(Self::Slashed),
            0x08 => Some(Self::Unbonding),
            0x09 => Some(Self::Retired),
            _ => None,
        }
    }

    pub const fn can_serve(self) -> bool {
        matches!(self, Self::ActiveNode | Self::Serving | Self::Auditing)
    }

    pub const fn is_slashed(self) -> bool {
        matches!(self, Self::Slashed)
    }

    pub const fn is_active(self) -> bool {
        matches!(self, Self::ActiveNode | Self::Serving)
    }
}

/// Metadata Registrasi Node Infrastruktur L5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeMetadata {
    pub node_id: InfrastructureNodeId,
    pub pubkey: [u8; 32],
    pub node_type: NodeType,
    pub status: NodeLifecycleStatus,
    pub collateral: Quantum,
    pub endpoint: String,
    pub registered_at_slot: u64,
    pub last_active_slot: u64,
    pub reputation_bps: u16, // 0..10,000 basis points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_node_id_determinism() {
        let pubkey = [0x42; 32];
        let id1 = compute_node_id(&pubkey);
        let id2 = compute_node_id(&pubkey);
        assert_eq!(id1, id2);
        assert_ne!(id1, [0u8; 32]);
    }

    #[test]
    fn test_node_type_roundtrip() {
        let types = [
            NodeType::ComputeWorker,
            NodeType::StorageKeeper,
            NodeType::DasNode,
            NodeType::Indexer,
            NodeType::GatewayRelay,
            NodeType::M2MOrchestrator,
        ];
        for t in types {
            let b = t as u8;
            assert_eq!(NodeType::from_u8(b), Some(t));
        }
        assert_eq!(NodeType::from_u8(0xFF), None);
    }

    #[test]
    fn test_node_lifecycle_status_properties() {
        assert!(NodeLifecycleStatus::Serving.can_serve());
        assert!(NodeLifecycleStatus::ActiveNode.can_serve());
        assert!(!NodeLifecycleStatus::Initializing.can_serve());
        assert!(!NodeLifecycleStatus::Challenged.can_serve());
        assert!(NodeLifecycleStatus::Slashed.is_slashed());
        assert!(!NodeLifecycleStatus::Retired.can_serve());
    }
}
