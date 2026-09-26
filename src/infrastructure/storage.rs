#![forbid(unsafe_code)]

//! Jaringan Penyimpanan Terdistribusi Berbasis Pengalamatan Konten Blake3 (REQ-L5-03).
//! Invariant: AUR-L5-DATA-001 (Blake3 Content Addressing), AUR-L5-PREC-001 (Zero-Float Quantum).

use super::types::{InfrastructureNodeId, L5_STORAGE_CHUNK_BYTES};
use crate::primitives::core::Quantum;
use blake3::Hasher;
use std::collections::BTreeMap;

/// Pengenal Potongan Data (Blake3 Chunk Digest).
pub type ChunkId = [u8; 32];

/// Menghitung ChunkId deterministik dari payload biner potongan data.
pub fn compute_chunk_id(data: &[u8]) -> ChunkId {
    let mut hasher = Hasher::new();
    hasher.update(b"AURION-L5-STORAGE-CHUNK-V1");
    hasher.update(data);
    *hasher.finalize().as_bytes()
}

/// Menghitung hash cabang Merkle pohon penyimpanan Blake3.
pub fn hash_storage_branch(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(b"AURION-L5-STORAGE-MERKLE-BRANCH-V1");
    hasher.update(left);
    hasher.update(right);
    *hasher.finalize().as_bytes()
}

/// Manifest Berkas Penyimpanan Terdistribusi L5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageManifest {
    pub content_id: [u8; 32],
    pub total_bytes: u64,
    pub chunk_count: u32,
    pub chunk_ids: Vec<ChunkId>,
    pub storage_root: [u8; 32],
    pub owner: [u8; 32],
    pub rent_quanta_per_slot: Quantum,
}

impl StorageManifest {
    /// Membuat manifest penyimpanan dengan memecah payload biner menjadi chunk kanonikal 64 KB.
    pub fn create(
        data: &[u8],
        owner: [u8; 32],
        rent_quanta_per_slot: Quantum,
    ) -> (Self, Vec<Vec<u8>>) {
        let total_bytes = data.len() as u64;
        let mut chunks = Vec::new();
        let mut chunk_ids = Vec::new();

        if data.is_empty() {
            let empty_chunk = Vec::new();
            let cid = compute_chunk_id(&empty_chunk);
            chunks.push(empty_chunk);
            chunk_ids.push(cid);
        } else {
            for chunk_slice in data.chunks(L5_STORAGE_CHUNK_BYTES) {
                let chunk_vec = chunk_slice.to_vec();
                let cid = compute_chunk_id(&chunk_vec);
                chunks.push(chunk_vec);
                chunk_ids.push(cid);
            }
        }

        let storage_root = Self::compute_merkle_root(&chunk_ids);

        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-CONTENT-ID-V1");
        hasher.update(&storage_root);
        hasher.update(&total_bytes.to_be_bytes());
        hasher.update(&owner);
        let content_id = *hasher.finalize().as_bytes();

        let manifest = Self {
            content_id,
            total_bytes,
            chunk_count: chunk_ids.len() as u32,
            chunk_ids,
            storage_root,
            owner,
            rent_quanta_per_slot,
        };

        (manifest, chunks)
    }

    /// Menghitung root pohon Merkle biner Blake3 dari daftar ChunkId.
    pub fn compute_merkle_root(leaf_hashes: &[ChunkId]) -> [u8; 32] {
        if leaf_hashes.is_empty() {
            return [0u8; 32];
        }
        if leaf_hashes.len() == 1 {
            return leaf_hashes[0];
        }

        let mut current_layer = leaf_hashes.to_vec();
        while current_layer.len() > 1 {
            let mut next_layer = Vec::with_capacity(current_layer.len().div_ceil(2));
            for pair in current_layer.chunks(2) {
                if pair.len() == 2 {
                    next_layer.push(hash_storage_branch(&pair[0], &pair[1]));
                } else {
                    // Duplikasi simpul ganjil sesuai kanonikal pohon Merkle
                    next_layer.push(hash_storage_branch(&pair[0], &pair[0]));
                }
            }
            current_layer = next_layer;
        }

        current_layer[0]
    }

    /// Menghasilkan jalur bukti Merkle untuk chunk pada indeks tertentu.
    pub fn generate_merkle_proof(&self, target_idx: usize) -> Result<Vec<[u8; 32]>, &'static str> {
        if target_idx >= self.chunk_ids.len() {
            return Err("Target index out of bounds for chunk list");
        }

        let mut proof = Vec::new();
        let mut current_layer = self.chunk_ids.clone();
        let mut idx = target_idx;

        while current_layer.len() > 1 {
            let sibling_idx = if idx.is_multiple_of(2) {
                idx + 1
            } else {
                idx - 1
            };
            let sibling = if sibling_idx < current_layer.len() {
                current_layer[sibling_idx]
            } else {
                current_layer[idx]
            };
            proof.push(sibling);

            let mut next_layer = Vec::with_capacity(current_layer.len().div_ceil(2));
            for pair in current_layer.chunks(2) {
                if pair.len() == 2 {
                    next_layer.push(hash_storage_branch(&pair[0], &pair[1]));
                } else {
                    next_layer.push(hash_storage_branch(&pair[0], &pair[0]));
                }
            }
            current_layer = next_layer;
            idx /= 2;
        }

        Ok(proof)
    }
}

/// Bukti Ketersediaan & Keutuhan Potongan Penyimpanan (Proof of Retrievability).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofOfRetrievability {
    pub chunk_index: u32,
    pub chunk_id: ChunkId,
    pub merkle_path: Vec<[u8; 32]>,
}

impl ProofOfRetrievability {
    /// Memverifikasi keabsahan bukti Proof of Retrievability terhadap storage_root.
    pub fn verify(&self, expected_storage_root: &[u8; 32]) -> bool {
        let mut current = self.chunk_id;
        let mut idx = self.chunk_index as usize;

        for sibling in &self.merkle_path {
            if idx.is_multiple_of(2) {
                current = hash_storage_branch(&current, sibling);
            } else {
                current = hash_storage_branch(sibling, &current);
            }
            idx /= 2;
        }

        current == *expected_storage_root
    }
}

/// Pengelola Alokasi & Audit Penyimpanan Terdistribusi L5.
#[derive(Debug, Default)]
pub struct StorageGrid {
    manifests: BTreeMap<[u8; 32], StorageManifest>,
    allocations: BTreeMap<ChunkId, Vec<InfrastructureNodeId>>,
}

impl StorageGrid {
    pub fn new() -> Self {
        Self {
            manifests: BTreeMap::new(),
            allocations: BTreeMap::new(),
        }
    }

    /// Mendaftarkan manifest berkas baru ke grid penyimpanan.
    pub fn register_manifest(&mut self, manifest: StorageManifest) {
        self.manifests.insert(manifest.content_id, manifest);
    }

    /// Mengalokasikan replika chunk ke node penjaga (StorageKeeper).
    pub fn allocate_replica(&mut self, chunk_id: ChunkId, keeper_id: InfrastructureNodeId) {
        let entry = self.allocations.entry(chunk_id).or_default();
        if !entry.contains(&keeper_id) {
            entry.push(keeper_id);
        }
    }

    /// Menghitung tantangan acak audit untuk keeper pada slot tertentu.
    pub fn generate_audit_challenge(seed: &[u8; 32], chunk_count: u32) -> u32 {
        if chunk_count == 0 {
            return 0;
        }
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-STORAGE-POR-CHALLENGE-V1");
        hasher.update(seed);
        let digest = hasher.finalize();
        let bytes: [u8; 4] = digest.as_bytes()[0..4].try_into().unwrap();
        (u32::from_be_bytes(bytes)) % chunk_count
    }

    pub fn get_manifest(&self, content_id: &[u8; 32]) -> Option<&StorageManifest> {
        self.manifests.get(content_id)
    }

    pub fn get_replicas(&self, chunk_id: &ChunkId) -> &[InfrastructureNodeId] {
        self.allocations
            .get(chunk_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_chunking_and_por_verification() {
        // Data berukuran 150 KB (menghasilkan 3 chunk)
        let data = vec![0xAB; 150_000];
        let owner = [0x01; 32];
        let rent = Quantum::new(100);

        let (manifest, chunks) = StorageManifest::create(&data, owner, rent);
        assert_eq!(manifest.chunk_count, 3);
        assert_eq!(chunks.len(), 3);

        // Verifikasi Por untuk setiap chunk
        for (i, chunk) in chunks.iter().enumerate() {
            let cid = compute_chunk_id(chunk);
            assert_eq!(manifest.chunk_ids[i], cid);

            let proof_path = manifest
                .generate_merkle_proof(i)
                .expect("Proof must be generated");
            let por = ProofOfRetrievability {
                chunk_index: i as u32,
                chunk_id: cid,
                merkle_path: proof_path,
            };

            assert!(por.verify(&manifest.storage_root));
        }
    }

    #[test]
    fn test_storage_por_tampered_rejected() {
        let data = vec![0x12; 200_000];
        let (manifest, _chunks) = StorageManifest::create(&data, [0x02; 32], Quantum::new(50));

        let proof_path = manifest.generate_merkle_proof(0).unwrap();
        let mut tampered_path = proof_path.clone();
        tampered_path[0][0] ^= 0xFF;

        let por = ProofOfRetrievability {
            chunk_index: 0,
            chunk_id: manifest.chunk_ids[0],
            merkle_path: tampered_path,
        };

        assert!(!por.verify(&manifest.storage_root));
    }
}
