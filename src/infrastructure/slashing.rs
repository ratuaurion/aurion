#![forbid(unsafe_code)]

//! Mesin Arbitrase, Tantangan Penipuan & Pemotongan Jaminan (Slashing) L5 (REQ-L5-11).
//! Invariant: AUR-L5-ARCH-002 (Economic Security Anchoring via Smart Contract Slashing).

use super::node::NodeRegistry;
use super::types::{InfrastructureNodeId, NodeLifecycleStatus, L5_CHALLENGE_WINDOW_SLOTS};
use crate::primitives::core::{Address, Quantum};
use blake3::Hasher;
use std::collections::BTreeMap;

/// Jenis Pelanggaran Infrastruktur L5 yang Dapat Dikenakan Slashing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ViolationType {
    InvalidComputeResult = 0x01,
    MissingStorageChunk = 0x02,
    DataAvailabilityWithholding = 0x03,
    FabricatedQueryResponse = 0x04,
    BreachedAgentMandate = 0x05,
}

impl ViolationType {
    /// Basis points penalti pemotongan jaminan (1..10,000 bps).
    pub const fn penalty_bps(self) -> u16 {
        match self {
            Self::InvalidComputeResult => 5_000,         // 50%
            Self::MissingStorageChunk => 2_500,          // 25%
            Self::DataAvailabilityWithholding => 10_000, // 100% (Pelanggaran Kritis)
            Self::FabricatedQueryResponse => 7_500,      // 75%
            Self::BreachedAgentMandate => 5_000,         // 50%
        }
    }
}

/// Berkas Laporan Tantangan Penipuan (Fraud Challenge Report).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FraudChallenge {
    pub challenge_id: [u8; 32],
    pub accused_node_id: InfrastructureNodeId,
    pub whistleblower: Address,
    pub violation_type: ViolationType,
    pub evidence_hash: [u8; 32],
    pub submitted_at_slot: u64,
    pub challenge_deadline_slot: u64,
    pub resolved: bool,
    pub convicted: bool,
}

impl FraudChallenge {
    pub fn new(
        accused_node_id: InfrastructureNodeId,
        whistleblower: Address,
        violation_type: ViolationType,
        evidence: &[u8],
        submitted_at_slot: u64,
    ) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-FRAUD-EVIDENCE-V1");
        hasher.update(evidence);
        let evidence_hash = *hasher.finalize().as_bytes();

        let mut hasher2 = Hasher::new();
        hasher2.update(b"AURION-L5-FRAUD-CHALLENGE-ID-V1");
        hasher2.update(&accused_node_id);
        hasher2.update(whistleblower.as_bytes());
        hasher2.update(&[violation_type as u8]);
        hasher2.update(&evidence_hash);
        hasher2.update(&submitted_at_slot.to_be_bytes());
        let challenge_id = *hasher2.finalize().as_bytes();

        Self {
            challenge_id,
            accused_node_id,
            whistleblower,
            violation_type,
            evidence_hash,
            submitted_at_slot,
            challenge_deadline_slot: submitted_at_slot + L5_CHALLENGE_WINDOW_SLOTS,
            resolved: false,
            convicted: false,
        }
    }
}

/// Hasil Putusan Arbitrase Slashing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArbitrationVerdict {
    pub challenge_id: [u8; 32],
    pub convicted: bool,
    pub slashed_quanta: Quantum,
    pub whistleblower_bounty: Quantum,
    pub burned_quanta: Quantum,
}

/// Mesin Arbitrase & Slashing Jaminan Infrastruktur L5.
#[derive(Debug, Default)]
pub struct ArbitrationEngine {
    challenges: BTreeMap<[u8; 32], FraudChallenge>,
    total_slashed_burned: Quantum,
}

impl ArbitrationEngine {
    pub fn new() -> Self {
        Self {
            challenges: BTreeMap::new(),
            total_slashed_burned: Quantum::new(0),
        }
    }

    /// Mengajukan tantangan penipuan terhadap node dan mengubah statusnya menjadi Challenged.
    pub fn file_challenge(
        &mut self,
        challenge: FraudChallenge,
        registry: &mut NodeRegistry,
        current_slot: u64,
    ) -> Result<[u8; 32], &'static str> {
        let node = registry
            .get_node(&challenge.accused_node_id)
            .ok_or("Accused node not found in registry")?;

        if node.status.is_slashed() {
            return Err("Node is already slashed");
        }

        registry.update_status(
            &challenge.accused_node_id,
            NodeLifecycleStatus::Challenged,
            current_slot,
        )?;

        let cid = challenge.challenge_id;
        self.challenges.insert(cid, challenge);
        Ok(cid)
    }

    /// Memutuskan tantangan arbitrase: jika terbukti bersalah, jaminan dipotong (50% bounty, 50% burn).
    pub fn adjudicate_challenge(
        &mut self,
        challenge_id: &[u8; 32],
        is_fraud_proven: bool,
        registry: &mut NodeRegistry,
        current_slot: u64,
    ) -> Result<ArbitrationVerdict, &'static str> {
        let challenge = self
            .challenges
            .get_mut(challenge_id)
            .ok_or("Challenge record not found")?;

        if challenge.resolved {
            return Err("Challenge has already been resolved");
        }

        if is_fraud_proven {
            let penalty_bps = challenge.violation_type.penalty_bps();
            let (slashed_amount, _) =
                registry.slash_node(&challenge.accused_node_id, penalty_bps)?;

            // 50% diberikan ke pelapor sebagai bounty, 50% dibakar (burn)
            let bounty_quanta = slashed_amount.as_u128() / 2;
            let burned_quanta = slashed_amount.as_u128() - bounty_quanta;

            let whistleblower_bounty = Quantum::new(bounty_quanta);
            let burned = Quantum::new(burned_quanta);

            self.total_slashed_burned = self
                .total_slashed_burned
                .checked_add(burned)
                .map_err(|_| "Overflow in total burned slashed quanta")?;

            challenge.resolved = true;
            challenge.convicted = true;

            Ok(ArbitrationVerdict {
                challenge_id: *challenge_id,
                convicted: true,
                slashed_quanta: slashed_amount,
                whistleblower_bounty,
                burned_quanta: burned,
            })
        } else {
            // Node bebas dari tuduhan, kembalikan ke status ActiveNode
            registry.update_status(
                &challenge.accused_node_id,
                NodeLifecycleStatus::ActiveNode,
                current_slot,
            )?;

            challenge.resolved = true;
            challenge.convicted = false;

            Ok(ArbitrationVerdict {
                challenge_id: *challenge_id,
                convicted: false,
                slashed_quanta: Quantum::new(0),
                whistleblower_bounty: Quantum::new(0),
                burned_quanta: Quantum::new(0),
            })
        }
    }

    pub fn get_challenge(&self, id: &[u8; 32]) -> Option<&FraudChallenge> {
        self.challenges.get(id)
    }

    pub fn total_burned(&self) -> Quantum {
        self.total_slashed_burned
    }
}

#[cfg(test)]
mod tests {
    use super::super::node::NodeRegistrationRequest;
    use super::super::types::{NodeType, L5_MIN_NODE_COLLATERAL_QUANTA};
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn setup_active_node(registry: &mut NodeRegistry) -> InfrastructureNodeId {
        let sk = SigningKey::from_bytes(&[0x44; 32]);
        let pk = sk.verifying_key().to_bytes();
        let col = Quantum::new(L5_MIN_NODE_COLLATERAL_QUANTA);
        let ep = "https://test.aurion.network".to_string();

        let digest = NodeRegistrationRequest::compute_registration_digest(
            &pk,
            NodeType::ComputeWorker,
            col,
            &ep,
        );
        let sig = sk.sign(&digest).to_bytes();

        let req = NodeRegistrationRequest {
            pubkey: pk,
            node_type: NodeType::ComputeWorker,
            collateral: col,
            endpoint: ep,
            signature: sig,
        };

        registry.register_node(req, 100).unwrap()
    }

    #[test]
    fn test_arbitration_conviction_and_slashing_split() {
        let mut registry = NodeRegistry::new();
        let node_id = setup_active_node(&mut registry);
        let mut arb = ArbitrationEngine::new();

        let whistleblower = Address::from_bytes([0x99; 32]);
        let evidence = b"proof_of_invalid_computation";
        let challenge = FraudChallenge::new(
            node_id,
            whistleblower,
            ViolationType::InvalidComputeResult, // 50% penalty
            evidence,
            200,
        );

        let cid = arb.file_challenge(challenge, &mut registry, 200).unwrap();
        assert_eq!(
            registry.get_node(&node_id).unwrap().status,
            NodeLifecycleStatus::Challenged
        );

        // Adjudicate: Fraud proven
        let verdict = arb
            .adjudicate_challenge(&cid, true, &mut registry, 250)
            .unwrap();
        assert!(verdict.convicted);
        assert_eq!(
            verdict.slashed_quanta,
            Quantum::new(L5_MIN_NODE_COLLATERAL_QUANTA / 2)
        );
        assert_eq!(
            verdict.whistleblower_bounty.as_u128(),
            verdict.slashed_quanta.as_u128() / 2
        );
        assert_eq!(
            verdict.burned_quanta.as_u128(),
            verdict.slashed_quanta.as_u128() / 2
        );

        let node = registry.get_node(&node_id).unwrap();
        assert_eq!(node.status, NodeLifecycleStatus::Slashed);
        assert_eq!(
            node.collateral,
            Quantum::new(L5_MIN_NODE_COLLATERAL_QUANTA / 2)
        );
    }

    #[test]
    fn test_arbitration_exoneration_restores_active_status() {
        let mut registry = NodeRegistry::new();
        let node_id = setup_active_node(&mut registry);
        let mut arb = ArbitrationEngine::new();

        let challenge = FraudChallenge::new(
            node_id,
            Address::from_bytes([0x99; 32]),
            ViolationType::MissingStorageChunk,
            b"bogus_evidence",
            300,
        );

        let cid = arb.file_challenge(challenge, &mut registry, 300).unwrap();
        let verdict = arb
            .adjudicate_challenge(&cid, false, &mut registry, 310)
            .unwrap();
        assert!(!verdict.convicted);
        assert_eq!(verdict.slashed_quanta, Quantum::new(0));
        assert_eq!(
            registry.get_node(&node_id).unwrap().status,
            NodeLifecycleStatus::ActiveNode
        );
    }
}
