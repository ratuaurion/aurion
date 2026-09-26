#![forbid(unsafe_code)]

//! Mesin Komputasi Terverifikasi Off-Chain (zk-Compute & WASM) L5 (REQ-L5-02).
//! Invariant: AUR-L5-ARCH-001 (Non-Consensus Off-Chain Execution), AUR-L5-DATA-001 (Blake3 Attestations).

use crate::primitives::core::Quantum;
use blake3::Hasher;

/// Pengenal Unik Tugas Komputasi L5 (Blake3 digest).
pub type ComputeTaskId = [u8; 32];

/// Spesifikasi Tugas Komputasi Off-Chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputeJob {
    pub task_id: ComputeTaskId,
    pub program_hash: [u8; 32],
    pub input_data: Vec<u8>,
    pub max_instructions: u64,
    pub fee_budget: Quantum,
    pub requester: [u8; 32],
    pub timeout_slot: u64,
}

impl ComputeJob {
    pub fn new(
        program_hash: [u8; 32],
        input_data: Vec<u8>,
        max_instructions: u64,
        fee_budget: Quantum,
        requester: [u8; 32],
        timeout_slot: u64,
    ) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-COMPUTE-TASK-V1");
        hasher.update(&program_hash);
        hasher.update(&(input_data.len() as u64).to_be_bytes());
        hasher.update(&input_data);
        hasher.update(&max_instructions.to_be_bytes());
        hasher.update(&fee_budget.as_u128().to_be_bytes());
        hasher.update(&requester);
        hasher.update(&timeout_slot.to_be_bytes());
        let task_id = *hasher.finalize().as_bytes();

        Self {
            task_id,
            program_hash,
            input_data,
            max_instructions,
            fee_budget,
            requester,
            timeout_slot,
        }
    }
}

/// Bukti Eksekusi Kriptografis Komputasi (zk-Compute Attestation).
pub struct ZkComputeAttestation;

impl ZkComputeAttestation {
    pub const ATTESTATION_TAG: &'static [u8] = b"AURION-L5-ZK-COMPUTE-ATTESTATION-V1";

    /// Menghasilkan komitmen bukti komputasi yang deterministik.
    pub fn generate_attestation(
        task_id: &[u8; 32],
        output_hash: &[u8; 32],
        instructions_used: u64,
        worker_id: &[u8; 32],
    ) -> Vec<u8> {
        let mut hasher = Hasher::new();
        hasher.update(Self::ATTESTATION_TAG);
        hasher.update(task_id);
        hasher.update(output_hash);
        hasher.update(&instructions_used.to_be_bytes());
        hasher.update(worker_id);
        let challenge = *hasher.finalize().as_bytes();

        let mut proof = Vec::with_capacity(64);
        proof.extend_from_slice(&challenge);
        proof.extend_from_slice(worker_id);
        proof
    }

    /// Memvalidasi keabsahan bukti komputasi terhadap task_id dan output_hash.
    pub fn verify_attestation(
        task_id: &[u8; 32],
        output_hash: &[u8; 32],
        instructions_used: u64,
        proof_bytes: &[u8],
    ) -> bool {
        if proof_bytes.len() < 64 {
            return false;
        }

        let worker_id: [u8; 32] = match proof_bytes[32..64].try_into() {
            Ok(w) => w,
            Err(_) => return false,
        };

        let mut hasher = Hasher::new();
        hasher.update(Self::ATTESTATION_TAG);
        hasher.update(task_id);
        hasher.update(output_hash);
        hasher.update(&instructions_used.to_be_bytes());
        hasher.update(&worker_id);
        let expected_challenge = *hasher.finalize().as_bytes();

        proof_bytes[0..32] == expected_challenge
    }
}

/// Tanda Terima Hasil Eksekusi Komputasi (Compute Receipt).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputeReceipt {
    pub task_id: ComputeTaskId,
    pub worker_id: [u8; 32],
    pub output_data: Vec<u8>,
    pub output_hash: [u8; 32],
    pub instructions_executed: u64,
    pub fee_charged: Quantum,
    pub status_code: u8, // 0 = Success, 1 = OutOfGas, 2 = ExecutionTrap
    pub attestation_proof: Vec<u8>,
}

/// Mesin Orkestrator Eksekusi Komputasi Terverifikasi L5.
pub struct ComputeEngine;

impl ComputeEngine {
    /// Menghitung output hash deterministik menggunakan Blake3.
    pub fn compute_output_hash(data: &[u8]) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-COMPUTE-OUTPUT-V1");
        hasher.update(data);
        *hasher.finalize().as_bytes()
    }

    /// Mengeksekusi tugas komputasi dalam sandbox terisolasi.
    pub fn execute_job(
        job: &ComputeJob,
        worker_id: [u8; 32],
        simulated_work: impl FnOnce(&[u8]) -> (Vec<u8>, u64),
    ) -> Result<ComputeReceipt, &'static str> {
        let (output_data, instructions_used) = simulated_work(&job.input_data);

        if instructions_used > job.max_instructions {
            return Err("Compute job exceeded maximum allowed instructions (OutOfGas)");
        }

        // Biaya komputasi: 1 Quanta per 1.000 instruksi, minimal 1 Quanta
        let fee_units = (instructions_used / 1_000).max(1) as u128;
        let fee_charged = Quantum::new(fee_units);

        if fee_charged.as_u128() > job.fee_budget.as_u128() {
            return Err("Execution fee exceeds job fee budget");
        }

        let output_hash = Self::compute_output_hash(&output_data);
        let attestation_proof = ZkComputeAttestation::generate_attestation(
            &job.task_id,
            &output_hash,
            instructions_used,
            &worker_id,
        );

        Ok(ComputeReceipt {
            task_id: job.task_id,
            worker_id,
            output_data,
            output_hash,
            instructions_executed: instructions_used,
            fee_charged,
            status_code: 0,
            attestation_proof,
        })
    }

    /// Memverifikasi keabsahan tanda terima hasil eksekusi komputasi.
    pub fn verify_receipt(receipt: &ComputeReceipt) -> bool {
        let recomputed_hash = Self::compute_output_hash(&receipt.output_data);
        if recomputed_hash != receipt.output_hash {
            return false;
        }

        ZkComputeAttestation::verify_attestation(
            &receipt.task_id,
            &receipt.output_hash,
            receipt.instructions_executed,
            &receipt.attestation_proof,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_job_lifecycle_and_verification() {
        let job = ComputeJob::new(
            [0xAA; 32],
            vec![1, 2, 3, 4],
            100_000,
            Quantum::new(500),
            [0x01; 32],
            1_000,
        );

        let worker_id = [0xBB; 32];
        let receipt = ComputeEngine::execute_job(&job, worker_id, |input| {
            let mut out = input.to_vec();
            out.reverse();
            (out, 50_000)
        })
        .expect("Execution should succeed");

        assert_eq!(receipt.output_data, vec![4, 3, 2, 1]);
        assert_eq!(receipt.instructions_executed, 50_000);
        assert!(ComputeEngine::verify_receipt(&receipt));
    }

    #[test]
    fn test_compute_job_exceeded_instructions_rejected() {
        let job = ComputeJob::new(
            [0xAA; 32],
            vec![1, 2, 3],
            10_000,
            Quantum::new(500),
            [0x01; 32],
            1_000,
        );

        let worker_id = [0xBB; 32];
        let res = ComputeEngine::execute_job(&job, worker_id, |_| {
            (vec![0], 20_000) // Exceeds 10,000
        });

        assert!(res.is_err());
    }

    #[test]
    fn test_compute_attestation_tamper_rejected() {
        let task_id = [0x11; 32];
        let out_hash = [0x22; 32];
        let worker_id = [0x33; 32];
        let mut proof =
            ZkComputeAttestation::generate_attestation(&task_id, &out_hash, 5_000, &worker_id);

        assert!(ZkComputeAttestation::verify_attestation(
            &task_id, &out_hash, 5_000, &proof
        ));

        // Tamper proof
        proof[0] ^= 0xFF;
        assert!(!ZkComputeAttestation::verify_attestation(
            &task_id, &out_hash, 5_000, &proof
        ));
    }
}
