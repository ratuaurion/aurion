#![forbid(unsafe_code)]

//! Infrastruktur Identitas Berdaulat Global (DID) & Mesin Reputasi Kriptografis (REQ-L5-06).
//! Invariant: AUR-L5-ARCH-001 (Sovereign DIDs & Verifiable Credentials).

use super::types::InfrastructureNodeId;
use crate::primitives::core::Address;
use blake3::Hasher;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::collections::BTreeMap;

/// Format Pengenal Terdesentralisasi Berdaulat Aurion (`did:aurion:<identifier>`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SovereignDid(pub String);

impl SovereignDid {
    /// Membuat DID dari alamat kanonikal Aurion.
    pub fn from_address(address: &Address) -> Self {
        let bech = crate::crypto::encode_address_bech32m(address, "aur")
            .unwrap_or_else(|_| hex::encode(address.as_bytes()));
        Self(format!("did:aurion:{bech}"))
    }

    /// Membuat DID langsung dari kunci publik Ed25519 32-byte.
    pub fn from_pubkey(pubkey: &[u8; 32]) -> Self {
        Self(format!("did:aurion:{}", hex::encode(pubkey)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Kredensial Terverifikasi Kriptografis (Verifiable Credential).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiableCredential {
    pub credential_id: [u8; 32],
    pub issuer_did: SovereignDid,
    pub subject_did: SovereignDid,
    pub claim_type: String,
    pub claim_value: Vec<u8>,
    pub issued_at_timestamp: u64,
    pub expires_at_timestamp: u64,
    pub signature: [u8; 64],
}

impl VerifiableCredential {
    pub fn compute_digest(
        issuer_did: &SovereignDid,
        subject_did: &SovereignDid,
        claim_type: &str,
        claim_value: &[u8],
        issued_at: u64,
        expires_at: u64,
    ) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-VERIFIABLE-CREDENTIAL-V1");
        hasher.update(issuer_did.as_str().as_bytes());
        hasher.update(subject_did.as_str().as_bytes());
        hasher.update(claim_type.as_bytes());
        hasher.update(&(claim_value.len() as u64).to_be_bytes());
        hasher.update(claim_value);
        hasher.update(&issued_at.to_be_bytes());
        hasher.update(&expires_at.to_be_bytes());
        *hasher.finalize().as_bytes()
    }

    /// Memvalidasi kredensial terhadap kunci publik issuer dan waktu kedaluwarsa.
    pub fn verify(&self, issuer_pubkey: &[u8; 32], current_timestamp: u64) -> bool {
        if current_timestamp > self.expires_at_timestamp {
            return false;
        }

        let digest = Self::compute_digest(
            &self.issuer_did,
            &self.subject_did,
            &self.claim_type,
            &self.claim_value,
            self.issued_at_timestamp,
            self.expires_at_timestamp,
        );

        let vk = match VerifyingKey::from_bytes(issuer_pubkey) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let sig = Signature::from_bytes(&self.signature);

        vk.verify(&digest, &sig).is_ok()
    }
}

/// Mesin Penilai Reputasi Deterministik Berbasis Basis Points (0..10,000 bps).
#[derive(Debug, Default)]
pub struct ReputationEngine {
    scores: BTreeMap<InfrastructureNodeId, u16>,
}

impl ReputationEngine {
    pub fn new() -> Self {
        Self {
            scores: BTreeMap::new(),
        }
    }

    pub fn get_score(&self, node_id: &InfrastructureNodeId) -> u16 {
        self.scores.get(node_id).copied().unwrap_or(10_000) // Default 100.00%
    }

    /// Menambah reputasi atas keberhasilan pemenuhan tugas/layanan.
    pub fn record_successful_service(&mut self, node_id: &InfrastructureNodeId, reward_bps: u16) {
        let current = self.get_score(node_id);
        let updated = current.saturating_add(reward_bps).min(10_000);
        self.scores.insert(*node_id, updated);
    }

    /// Mengurangi reputasi atas keterlambatan atau pelanggaran SLA.
    pub fn record_infraction(&mut self, node_id: &InfrastructureNodeId, penalty_bps: u16) {
        let current = self.get_score(node_id);
        let updated = current.saturating_sub(penalty_bps);
        self.scores.insert(*node_id, updated);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn test_sovereign_did_and_credential_verification() {
        let issuer_sk = SigningKey::from_bytes(&[0x66; 32]);
        let issuer_pk = issuer_sk.verifying_key().to_bytes();
        let issuer_did = SovereignDid::from_pubkey(&issuer_pk);

        let subject_pk = [0x77; 32];
        let subject_did = SovereignDid::from_pubkey(&subject_pk);

        let claim_type = "InfrastructureProviderLicense";
        let claim_value = b"Tier1-ComputeWorker";
        let issued_at = 1_000;
        let expires_at = 2_000;

        let digest = VerifiableCredential::compute_digest(
            &issuer_did,
            &subject_did,
            claim_type,
            claim_value,
            issued_at,
            expires_at,
        );
        let signature = issuer_sk.sign(&digest).to_bytes();

        let vc = VerifiableCredential {
            credential_id: [0x01; 32],
            issuer_did,
            subject_did,
            claim_type: claim_type.to_string(),
            claim_value: claim_value.to_vec(),
            issued_at_timestamp: issued_at,
            expires_at_timestamp: expires_at,
            signature,
        };

        assert!(vc.verify(&issuer_pk, 1_500));
        // Expired
        assert!(!vc.verify(&issuer_pk, 2_500));
    }

    #[test]
    fn test_reputation_engine_score_dynamics() {
        let mut engine = ReputationEngine::new();
        let node_id = [0x88; 32];

        assert_eq!(engine.get_score(&node_id), 10_000);

        // Infraction penalty
        engine.record_infraction(&node_id, 1_500);
        assert_eq!(engine.get_score(&node_id), 8_500);

        // Recovery reward
        engine.record_successful_service(&node_id, 500);
        assert_eq!(engine.get_score(&node_id), 9_000);
    }
}
