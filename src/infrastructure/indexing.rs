#![forbid(unsafe_code)]

//! Jaringan Pengindeksan Terdistribusi & Atestasi Kueri Bebas-Fabrikasi (REQ-L5-05).
//! Invariant: AUR-L5-DATA-002 (Zero-Fabrication Data Provenance Terikat State Root L1).

use std::collections::BTreeMap;
use blake3::Hasher;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use super::types::InfrastructureNodeId;

/// Pengenal Unik Kueri Data L5.
pub type QueryId = [u8; 32];

/// Permintaan Kueri Pengindeksan Historis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexingQuery {
    pub query_id: QueryId,
    pub entity_tag: String,
    pub filter_key: [u8; 32],
    pub target_height: u64,
    pub requester: [u8; 32],
}

impl IndexingQuery {
    pub fn new(
        entity_tag: String,
        filter_key: [u8; 32],
        target_height: u64,
        requester: [u8; 32],
    ) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-INDEXING-QUERY-V1");
        hasher.update(entity_tag.as_bytes());
        hasher.update(&filter_key);
        hasher.update(&target_height.to_be_bytes());
        hasher.update(&requester);
        let query_id = *hasher.finalize().as_bytes();

        Self {
            query_id,
            entity_tag,
            filter_key,
            target_height,
            requester,
        }
    }
}

/// Atestasi Kriptografis Hasil Kueri (Query Attestation) Bebas Fabrikasi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryAttestation {
    pub query_id: QueryId,
    pub indexer_id: InfrastructureNodeId,
    pub indexer_pubkey: [u8; 32],
    pub result_payload: Vec<u8>,
    pub l1_state_root: [u8; 32],
    pub inclusion_proof: Vec<[u8; 32]>,
    pub signature: [u8; 64],
}

impl QueryAttestation {
    /// Menghasilkan intisari komitmen yang ditandatangani oleh indexer.
    pub fn compute_attestation_digest(
        query_id: &[u8; 32],
        result_payload: &[u8],
        l1_state_root: &[u8; 32],
    ) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-QUERY-ATTESTATION-DIGEST-V1");
        hasher.update(query_id);
        hasher.update(&(result_payload.len() as u64).to_be_bytes());
        hasher.update(result_payload);
        hasher.update(l1_state_root);
        *hasher.finalize().as_bytes()
    }

    /// Memverifikasi integritas hasil kueri terhadap tanda tangan indexer dan state root L1.
    pub fn verify_zero_fabrication(&self, expected_l1_state_root: &[u8; 32]) -> bool {
        // 1. Periksa keselarasan state root L1
        if self.l1_state_root != *expected_l1_state_root {
            return false;
        }

        // 2. Verifikasi tanda tangan Ed25519 indexer
        let digest = Self::compute_attestation_digest(
            &self.query_id,
            &self.result_payload,
            &self.l1_state_root,
        );

        let vk = match VerifyingKey::from_bytes(&self.indexer_pubkey) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let sig = Signature::from_bytes(&self.signature);

        if vk.verify(&digest, &sig).is_err() {
            return false;
        }

        // 3. Verifikasi bukti inklusi Merkle terhadap state root
        let mut current = Self::compute_leaf_hash(&self.result_payload);
        for sibling in &self.inclusion_proof {
            let mut hasher = Hasher::new();
            hasher.update(b"AURION-L5-INDEX-MERKLE-NODE-V1");
            hasher.update(&current);
            hasher.update(sibling);
            current = *hasher.finalize().as_bytes();
        }

        current == *expected_l1_state_root
    }

    /// Menghitung leaf hash deterministik dari payload hasil kueri.
    pub fn compute_leaf_hash(payload: &[u8]) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-INDEX-MERKLE-LEAF-V1");
        hasher.update(payload);
        *hasher.finalize().as_bytes()
    }
}

/// Jaringan Kueri & Indeks Terdesentralisasi (Decentralized Indexing Mesh).
#[derive(Debug, Default)]
pub struct IndexingMesh {
    registered_indexers: BTreeMap<InfrastructureNodeId, [u8; 32]>,
}

impl IndexingMesh {
    pub fn new() -> Self {
        Self {
            registered_indexers: BTreeMap::new(),
        }
    }

    pub fn register_indexer(&mut self, node_id: InfrastructureNodeId, pubkey: [u8; 32]) {
        self.registered_indexers.insert(node_id, pubkey);
    }

    pub fn is_indexer_registered(&self, node_id: &InfrastructureNodeId) -> bool {
        self.registered_indexers.contains_key(node_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn test_query_attestation_and_zero_fabrication_success() {
        let signing_key = SigningKey::from_bytes(&[0x55; 32]);
        let pubkey = signing_key.verifying_key().to_bytes();
        let indexer_id = [0x77; 32];

        let query = IndexingQuery::new(
            "AccountBalance".to_string(),
            [0x01; 32],
            1_000,
            [0x02; 32],
        );

        let result_payload = b"Account: aur1qqq... Balance: 500000000".to_vec();
        let leaf = QueryAttestation::compute_leaf_hash(&result_payload);
        let sibling = [0x99; 32];

        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-INDEX-MERKLE-NODE-V1");
        hasher.update(&leaf);
        hasher.update(&sibling);
        let expected_l1_state_root = *hasher.finalize().as_bytes();

        let digest = QueryAttestation::compute_attestation_digest(
            &query.query_id,
            &result_payload,
            &expected_l1_state_root,
        );
        let signature = signing_key.sign(&digest).to_bytes();

        let attestation = QueryAttestation {
            query_id: query.query_id,
            indexer_id,
            indexer_pubkey: pubkey,
            result_payload,
            l1_state_root: expected_l1_state_root,
            inclusion_proof: vec![sibling],
            signature,
        };

        assert!(attestation.verify_zero_fabrication(&expected_l1_state_root));
    }

    #[test]
    fn test_query_attestation_fabricated_payload_rejected() {
        let signing_key = SigningKey::from_bytes(&[0x55; 32]);
        let pubkey = signing_key.verifying_key().to_bytes();
        let query = IndexingQuery::new("TxReceipt".to_string(), [0x01; 32], 500, [0x02; 32]);

        let result_payload = b"Valid Payload".to_vec();
        let expected_l1_state_root = [0xEE; 32];

        let digest = QueryAttestation::compute_attestation_digest(
            &query.query_id,
            &result_payload,
            &expected_l1_state_root,
        );
        let signature = signing_key.sign(&digest).to_bytes();

        let mut attestation = QueryAttestation {
            query_id: query.query_id,
            indexer_id: [0x11; 32],
            indexer_pubkey: pubkey,
            result_payload,
            l1_state_root: expected_l1_state_root,
            inclusion_proof: vec![[0x00; 32]],
            signature,
        };

        // Fabricate payload
        attestation.result_payload = b"Fabricated Payload".to_vec();
        assert!(!attestation.verify_zero_fabrication(&expected_l1_state_root));
    }
}
