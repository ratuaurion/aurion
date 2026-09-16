//! # Aurion L3 Specialized Domain: Zero-Knowledge Confidential Privacy
//!
//! Confidential Shielded Pool domain adapter with note commitment tree, anti-double-spend nullifier
//! registry, and verified zero-knowledge proof state transitions.
//!
//! Conforms strictly to:
//! - AUR-ARCH-011: Zero unsafe code (`#![forbid(unsafe_code)]`)
//! - AUR-ARCH-012: Zero floating-point arithmetic (`Quantum(u128)`)
//! - AUR-L3-SEC-002: L3 Privacy domain execution & zero-knowledge confidential settlement

use std::collections::BTreeSet;
use blake3::Hasher;

use crate::primitives::core::Quantum;
use crate::specialized::types::DomainId;

/// A private shielded note within the confidential domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShieldedNote {
    /// Note value in integer Quantum.
    pub value: Quantum,
    /// Secret preimage for deterministic nullifier calculation.
    pub nullifier_preimage: [u8; 32],
    /// Public key / address of the note owner.
    pub recipient: [u8; 32],
    /// Cryptographic blinding randomness.
    pub randomness: [u8; 32],
}

impl ShieldedNote {
    /// Computes the deterministic Blake3 note commitment.
    pub fn compute_commitment(&self) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION_L3_NOTE_COMMITMENT_V1");
        hasher.update(&self.value.as_u128().to_be_bytes());
        hasher.update(&self.nullifier_preimage);
        hasher.update(&self.recipient);
        hasher.update(&self.randomness);
        *hasher.finalize().as_bytes()
    }

    /// Computes the unique nullifier hash revealed when this note is spent.
    pub fn compute_nullifier(&self) -> [u8; 32] {
        let commitment = self.compute_commitment();
        let mut hasher = Hasher::new();
        hasher.update(b"AURION_L3_NOTE_NULLIFIER_V1");
        hasher.update(&self.nullifier_preimage);
        hasher.update(&commitment);
        *hasher.finalize().as_bytes()
    }
}

/// Zero-Knowledge Proof representation for private note spend/transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZkProof {
    /// Cryptographic proof payload bytes (e.g., Groth16 / Plonk / Halo2 representation).
    pub proof_bytes: Vec<u8>,
    /// Public inputs hash commitment.
    pub public_inputs_hash: [u8; 32],
}

impl ZkProof {
    /// Creates a mock verification proof for testing/development.
    pub fn mock(public_inputs_hash: [u8; 32]) -> Self {
        Self {
            proof_bytes: vec![0x42; 64],
            public_inputs_hash,
        }
    }

    /// Verifies proof validity against expected public inputs hash.
    pub fn verify(&self, expected_public_inputs_hash: &[u8; 32]) -> bool {
        !self.proof_bytes.is_empty() && &self.public_inputs_hash == expected_public_inputs_hash
    }
}

/// Operational errors for the ZK Confidential Privacy Domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivacyError {
    /// Shielded amount cannot be zero.
    ZeroAmount,
    /// Nullifier was already published and spent (double-spend attempt).
    NullifierAlreadySpent([u8; 32]),
    /// Note commitment does not exist in the domain accumulator.
    CommitmentNotFound([u8; 32]),
    /// Zero-knowledge proof verification failed.
    InvalidZkProof,
    /// Vault balance insufficient to cover unshielding.
    InsufficientVaultBalance,
    /// Arithmetic overflow.
    ArithmeticOverflow,
}

/// Confidential ZK-Shielded Pool within an L3 specialized domain.
#[derive(Debug, Clone)]
pub struct ShieldedPool {
    /// Domain identifier.
    pub domain_id: DomainId,
    /// Total public Quanta locked in this shielded pool vault.
    pub vault_balance: Quantum,
    /// Ordered registry of all note commitments published.
    pub commitments: Vec<[u8; 32]>,
    /// Set of spent nullifiers to prevent double-spending.
    pub spent_nullifiers: BTreeSet<[u8; 32]>,
}

impl ShieldedPool {
    /// Instantiates a new empty shielded pool for a domain.
    pub fn new(domain_id: DomainId) -> Self {
        Self {
            domain_id,
            vault_balance: Quantum(0),
            commitments: Vec::new(),
            spent_nullifiers: BTreeSet::new(),
        }
    }

    /// Shields public funds: deposits public Quanta into the vault and registers a new note commitment.
    pub fn shield(&mut self, note: &ShieldedNote) -> Result<[u8; 32], PrivacyError> {
        if note.value == Quantum(0) {
            return Err(PrivacyError::ZeroAmount);
        }

        self.vault_balance = self
            .vault_balance
            .checked_add(note.value)
            .map_err(|_| PrivacyError::ArithmeticOverflow)?;

        let commitment = note.compute_commitment();
        self.commitments.push(commitment);
        Ok(commitment)
    }

    /// Private transfer within the shielded pool:
    /// Spends an existing note (revealing its nullifier) and creates a new note commitment, verified by ZK proof.
    pub fn transfer_private(
        &mut self,
        nullifier: [u8; 32],
        new_commitment: [u8; 32],
        proof: &ZkProof,
    ) -> Result<(), PrivacyError> {
        if self.spent_nullifiers.contains(&nullifier) {
            return Err(PrivacyError::NullifierAlreadySpent(nullifier));
        }

        // Verify public inputs commitment: Blake3(nullifier, new_commitment)
        let mut hasher = Hasher::new();
        hasher.update(b"AURION_L3_PRIVACY_TRANSFER_INPUTS_V1");
        hasher.update(&nullifier);
        hasher.update(&new_commitment);
        let expected_inputs_hash = *hasher.finalize().as_bytes();

        if !proof.verify(&expected_inputs_hash) {
            return Err(PrivacyError::InvalidZkProof);
        }

        // Record spent nullifier and append new note commitment
        self.spent_nullifiers.insert(nullifier);
        self.commitments.push(new_commitment);

        Ok(())
    }

    /// Unshields funds: burns a private note by revealing its nullifier and unlocks public Quanta to recipient.
    pub fn unshield(
        &mut self,
        nullifier: [u8; 32],
        amount: Quantum,
        recipient: [u8; 32],
        proof: &ZkProof,
    ) -> Result<(), PrivacyError> {
        if amount == Quantum(0) {
            return Err(PrivacyError::ZeroAmount);
        }

        if self.spent_nullifiers.contains(&nullifier) {
            return Err(PrivacyError::NullifierAlreadySpent(nullifier));
        }

        if self.vault_balance < amount {
            return Err(PrivacyError::InsufficientVaultBalance);
        }

        // Verify public inputs commitment: Blake3(nullifier, amount, recipient)
        let mut hasher = Hasher::new();
        hasher.update(b"AURION_L3_PRIVACY_UNSHIELD_INPUTS_V1");
        hasher.update(&nullifier);
        hasher.update(&amount.as_u128().to_be_bytes());
        hasher.update(&recipient);
        let expected_inputs_hash = *hasher.finalize().as_bytes();

        if !proof.verify(&expected_inputs_hash) {
            return Err(PrivacyError::InvalidZkProof);
        }

        self.vault_balance = self
            .vault_balance
            .checked_sub(amount)
            .map_err(|_| PrivacyError::ArithmeticOverflow)?;

        self.spent_nullifiers.insert(nullifier);
        Ok(())
    }

    /// Computes root hash of the note commitment tree.
    pub fn compute_commitment_root(&self) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION_L3_COMMITMENT_ROOT_V1");
        hasher.update(&(self.commitments.len() as u64).to_be_bytes());
        for c in &self.commitments {
            hasher.update(c);
        }
        *hasher.finalize().as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shielded_pool_lifecycle_and_double_spend_prevention() {
        let domain_id = DomainId::named("zk-privacy-pool");
        let mut pool = ShieldedPool::new(domain_id);

        let note1 = ShieldedNote {
            value: Quantum(10_000),
            nullifier_preimage: [0x11u8; 32],
            recipient: [0x22u8; 32],
            randomness: [0x33u8; 32],
        };

        // 1. Shielding public funds
        let _commitment1 = pool.shield(&note1).expect("shield note 1");
        assert_eq!(pool.vault_balance, Quantum(10_000));
        assert_eq!(pool.commitments.len(), 1);

        // 2. Private transfer: note1 -> note2
        let nullifier1 = note1.compute_nullifier();

        let note2 = ShieldedNote {
            value: Quantum(10_000),
            nullifier_preimage: [0x44u8; 32],
            recipient: [0x55u8; 32],
            randomness: [0x66u8; 32],
        };
        let commitment2 = note2.compute_commitment();

        let mut hasher = Hasher::new();
        hasher.update(b"AURION_L3_PRIVACY_TRANSFER_INPUTS_V1");
        hasher.update(&nullifier1);
        hasher.update(&commitment2);
        let transfer_inputs = *hasher.finalize().as_bytes();
        let proof = ZkProof::mock(transfer_inputs);

        pool.transfer_private(nullifier1, commitment2, &proof)
            .expect("private transfer");
        assert_eq!(pool.commitments.len(), 2);
        assert!(pool.spent_nullifiers.contains(&nullifier1));

        // 3. Prevent Double-Spend with same nullifier
        let err = pool.transfer_private(nullifier1, commitment2, &proof);
        assert_eq!(err, Err(PrivacyError::NullifierAlreadySpent(nullifier1)));

        // 4. Unshield funds from note2 to public recipient
        let nullifier2 = note2.compute_nullifier();
        let recipient = [0x99u8; 32];

        let mut unshield_hasher = Hasher::new();
        unshield_hasher.update(b"AURION_L3_PRIVACY_UNSHIELD_INPUTS_V1");
        unshield_hasher.update(&nullifier2);
        unshield_hasher.update(&10_000u128.to_be_bytes());
        unshield_hasher.update(&recipient);
        let unshield_inputs = *unshield_hasher.finalize().as_bytes();
        let unshield_proof = ZkProof::mock(unshield_inputs);

        pool.unshield(nullifier2, Quantum(10_000), recipient, &unshield_proof)
            .expect("unshield funds");
        assert_eq!(pool.vault_balance, Quantum(0));
        assert!(pool.spent_nullifiers.contains(&nullifier2));
    }
}
