#![forbid(unsafe_code)]

//! Manajemen Registrasi & Siklus Hidup Node Infrastruktur L5 (REQ-L5-01).
//! Menegakkan jaminan ekonomi (collateralization), verifikasi Ed25519, dan transisi status formal.

use std::collections::BTreeMap;
use blake3::Hasher;
use ed25519_dalek::{Signature, VerifyingKey, Verifier};
use crate::primitives::core::Quantum;
use super::types::{
    compute_node_id, InfrastructureNodeId, NodeLifecycleStatus, NodeMetadata, NodeType,
    L5_MIN_NODE_COLLATERAL_QUANTA, L5_UNBONDING_DELAY_SLOTS,
};

/// Permohonan Registrasi Node Baru ke Jaringan Infrastruktur L5.
#[derive(Debug, Clone)]
pub struct NodeRegistrationRequest {
    pub pubkey: [u8; 32],
    pub node_type: NodeType,
    pub collateral: Quantum,
    pub endpoint: String,
    pub signature: [u8; 64],
}

impl NodeRegistrationRequest {
    /// Menghasilkan komitmen data yang harus ditandatangani oleh node saat mendaftar.
    pub fn compute_registration_digest(
        pubkey: &[u8; 32],
        node_type: NodeType,
        collateral: Quantum,
        endpoint: &str,
    ) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-NODE-REGISTRATION-V1");
        hasher.update(pubkey);
        hasher.update(&[node_type as u8]);
        hasher.update(&collateral.as_u128().to_be_bytes());
        hasher.update(endpoint.as_bytes());
        *hasher.finalize().as_bytes()
    }
}

/// Rekaman Status Unbonding untuk Penarikan Jaminan Tertib.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnbondingRecord {
    pub node_id: InfrastructureNodeId,
    pub collateral: Quantum,
    pub unlock_slot: u64,
}

/// Registri Terpusat Node Infrastruktur L5.
#[derive(Debug, Default)]
pub struct NodeRegistry {
    nodes: BTreeMap<InfrastructureNodeId, NodeMetadata>,
    unbonding_queue: BTreeMap<InfrastructureNodeId, UnbondingRecord>,
    total_collateral: Quantum,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            unbonding_queue: BTreeMap::new(),
            total_collateral: Quantum::new(0),
        }
    }

    /// Mendaftarkan node baru dengan verifikasi jaminan minimum dan tanda tangan Ed25519.
    pub fn register_node(
        &mut self,
        req: NodeRegistrationRequest,
        current_slot: u64,
    ) -> Result<InfrastructureNodeId, &'static str> {
        // 1. Validasi batas jaminan minimum
        if req.collateral.as_u128() < L5_MIN_NODE_COLLATERAL_QUANTA {
            return Err("Collateral below L5_MIN_NODE_COLLATERAL_QUANTA");
        }

        // 2. Verifikasi tanda tangan Ed25519
        let digest = NodeRegistrationRequest::compute_registration_digest(
            &req.pubkey,
            req.node_type,
            req.collateral,
            &req.endpoint,
        );

        let verifying_key = VerifyingKey::from_bytes(&req.pubkey)
            .map_err(|_| "Invalid Ed25519 public key")?;
        let sig = Signature::from_bytes(&req.signature);

        verifying_key
            .verify(&digest, &sig)
            .map_err(|_| "Node registration signature verification failed")?;

        // 3. Pastikan belum terdaftar
        let node_id = compute_node_id(&req.pubkey);
        if self.nodes.contains_key(&node_id) {
            return Err("Node already registered in registry");
        }

        // 4. Tambah jaminan ke total akumulasi
        self.total_collateral = self
            .total_collateral
            .checked_add(req.collateral)
            .map_err(|_| "Collateral overflow in total calculation")?;

        let meta = NodeMetadata {
            node_id,
            pubkey: req.pubkey,
            node_type: req.node_type,
            status: NodeLifecycleStatus::ActiveNode,
            collateral: req.collateral,
            endpoint: req.endpoint,
            registered_at_slot: current_slot,
            last_active_slot: current_slot,
            reputation_bps: 10_000, // Mulai dari reputasi sempurna 100.00%
        };

        self.nodes.insert(node_id, meta);
        Ok(node_id)
    }

    /// Memperbarui status siklus hidup node.
    pub fn update_status(
        &mut self,
        node_id: &InfrastructureNodeId,
        new_status: NodeLifecycleStatus,
        slot: u64,
    ) -> Result<(), &'static str> {
        let node = self.nodes.get_mut(node_id).ok_or("Node not found")?;
        if node.status.is_slashed() {
            return Err("Cannot update status of slashed node");
        }
        node.status = new_status;
        node.last_active_slot = slot;
        Ok(())
    }

    /// Memulai proses pelepasan jaminan (unbonding) secara tertib.
    pub fn initiate_unbonding(
        &mut self,
        node_id: &InfrastructureNodeId,
        current_slot: u64,
    ) -> Result<u64, &'static str> {
        let node = self.nodes.get_mut(node_id).ok_or("Node not found")?;
        if node.status.is_slashed() {
            return Err("Slashed node cannot unbond");
        }
        if node.status == NodeLifecycleStatus::Unbonding {
            return Err("Node already in unbonding state");
        }
        if node.status == NodeLifecycleStatus::Retired || node.collateral.as_u128() == 0 {
            return Err("Retired node or node with zero collateral cannot unbond");
        }

        node.status = NodeLifecycleStatus::Unbonding;
        node.last_active_slot = current_slot;

        let unlock_slot = current_slot.saturating_add(L5_UNBONDING_DELAY_SLOTS);
        self.unbonding_queue.insert(
            *node_id,
            UnbondingRecord {
                node_id: *node_id,
                collateral: node.collateral,
                unlock_slot,
            },
        );

        Ok(unlock_slot)
    }

    /// Menyelesaikan proses penarikan jaminan setelah masa tenggang terpenuhi.
    pub fn complete_unbonding(
        &mut self,
        node_id: &InfrastructureNodeId,
        current_slot: u64,
    ) -> Result<Quantum, &'static str> {
        let record = self
            .unbonding_queue
            .get(node_id)
            .ok_or("No unbonding record found for this node")?;

        if current_slot < record.unlock_slot {
            return Err("Unbonding delay slot has not elapsed yet");
        }

        let collateral = record.collateral;
        self.unbonding_queue.remove(node_id);

        if let Some(node) = self.nodes.get_mut(node_id) {
            node.status = NodeLifecycleStatus::Retired;
            node.collateral = Quantum::new(0);
        }

        self.total_collateral = self
            .total_collateral
            .checked_sub(collateral)
            .unwrap_or(Quantum::new(0));

        Ok(collateral)
    }

    /// Mengeksekusi penalti pemotongan jaminan (slashing) terhadap node yang curang.
    pub fn slash_node(
        &mut self,
        node_id: &InfrastructureNodeId,
        penalty_bps: u16, // Basis points (1..10,000)
    ) -> Result<(Quantum, Quantum), &'static str> {
        let node = self.nodes.get_mut(node_id).ok_or("Node not found")?;
        if node.status.is_slashed() {
            return Err("Node is already slashed");
        }

        let valid_bps = penalty_bps.min(10_000) as u128;
        let slash_amount_quanta = (node.collateral.as_u128() * valid_bps) / 10_000;
        let slash_amount = Quantum::new(slash_amount_quanta);
        let remaining_amount = node.collateral.checked_sub(slash_amount).unwrap_or(Quantum::new(0));

        node.collateral = remaining_amount;
        node.status = NodeLifecycleStatus::Slashed;
        node.reputation_bps = 0;

        self.total_collateral = self
            .total_collateral
            .checked_sub(slash_amount)
            .unwrap_or(Quantum::new(0));

        Ok((slash_amount, remaining_amount))
    }

    pub fn get_node(&self, node_id: &InfrastructureNodeId) -> Option<&NodeMetadata> {
        self.nodes.get(node_id)
    }

    pub fn list_active_nodes(&self, filter_type: Option<NodeType>) -> Vec<&NodeMetadata> {
        self.nodes
            .values()
            .filter(|n| n.status.can_serve())
            .filter(|n| filter_type.is_none() || Some(n.node_type) == filter_type)
            .collect()
    }

    pub fn total_collateral(&self) -> Quantum {
        self.total_collateral
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    fn make_test_keypair() -> (SigningKey, [u8; 32]) {
        let signing_key = SigningKey::from_bytes(&[0x11; 32]);
        let pubkey = signing_key.verifying_key().to_bytes();
        (signing_key, pubkey)
    }

    #[test]
    fn test_register_node_success() {
        let mut registry = NodeRegistry::new();
        let (sk, pk) = make_test_keypair();
        let col = Quantum::new(L5_MIN_NODE_COLLATERAL_QUANTA);
        let endpoint = "https://node1.aurion.network".to_string();

        let digest = NodeRegistrationRequest::compute_registration_digest(
            &pk,
            NodeType::ComputeWorker,
            col,
            &endpoint,
        );
        use ed25519_dalek::Signer;
        let sig = sk.sign(&digest).to_bytes();

        let req = NodeRegistrationRequest {
            pubkey: pk,
            node_type: NodeType::ComputeWorker,
            collateral: col,
            endpoint,
            signature: sig,
        };

        let node_id = registry.register_node(req, 100).expect("Registration should succeed");
        let node = registry.get_node(&node_id).expect("Node should exist");
        assert_eq!(node.status, NodeLifecycleStatus::ActiveNode);
        assert_eq!(node.reputation_bps, 10_000);
        assert_eq!(registry.total_collateral(), col);
    }

    #[test]
    fn test_register_node_insufficient_collateral_rejected() {
        let mut registry = NodeRegistry::new();
        let (sk, pk) = make_test_keypair();
        let col = Quantum::new(L5_MIN_NODE_COLLATERAL_QUANTA - 1); // 1 bawah batas
        let endpoint = "https://bad.aurion.network".to_string();

        let digest = NodeRegistrationRequest::compute_registration_digest(
            &pk,
            NodeType::StorageKeeper,
            col,
            &endpoint,
        );
        use ed25519_dalek::Signer;
        let sig = sk.sign(&digest).to_bytes();

        let req = NodeRegistrationRequest {
            pubkey: pk,
            node_type: NodeType::StorageKeeper,
            collateral: col,
            endpoint,
            signature: sig,
        };

        let res = registry.register_node(req, 100);
        assert!(res.is_err());
    }

    #[test]
    fn test_node_unbonding_and_slashing_lifecycle() {
        let mut registry = NodeRegistry::new();
        let (sk, pk) = make_test_keypair();
        let col = Quantum::new(L5_MIN_NODE_COLLATERAL_QUANTA);
        let endpoint = "https://worker.aurion.network".to_string();

        let digest = NodeRegistrationRequest::compute_registration_digest(
            &pk,
            NodeType::ComputeWorker,
            col,
            &endpoint,
        );
        use ed25519_dalek::Signer;
        let sig = sk.sign(&digest).to_bytes();

        let req = NodeRegistrationRequest {
            pubkey: pk,
            node_type: NodeType::ComputeWorker,
            collateral: col,
            endpoint,
            signature: sig,
        };

        let node_id = registry.register_node(req, 100).unwrap();

        // 1. Unbonding initiation
        let unlock_slot = registry.initiate_unbonding(&node_id, 200).unwrap();
        assert_eq!(unlock_slot, 200 + L5_UNBONDING_DELAY_SLOTS);

        // 2. Cannot complete early
        assert!(registry.complete_unbonding(&node_id, 250).is_err());

        // 3. Complete at or after unlock_slot
        let returned = registry.complete_unbonding(&node_id, unlock_slot + 1).unwrap();
        assert_eq!(returned, col);
        assert_eq!(registry.get_node(&node_id).unwrap().status, NodeLifecycleStatus::Retired);
        assert_eq!(registry.total_collateral(), Quantum::new(0));
    }
}
