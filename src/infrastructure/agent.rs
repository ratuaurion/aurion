#![forbid(unsafe_code)]

//! Mesin Mandat Kriptografis & Pendelegasian Otonom Agen AI (REQ-L5-09).
//! Invariant: AUR-L5-SEC-002 (Cryptographic Agent Mandate with Spending Cap & Expiry).

use crate::primitives::core::{Address, Quantum};
use blake3::Hasher;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::collections::BTreeMap;

/// Pengenal Unik Mandat Agen AI.
pub type MandateId = [u8; 32];

/// Mandat Otorisasi Kriptografis untuk Agen AI / Perangkat Otonom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMandate {
    pub mandate_id: MandateId,
    pub principal_address: Address,
    pub principal_pubkey: [u8; 32],
    pub agent_pubkey: [u8; 32],
    pub allowed_operations: Vec<String>,
    pub spending_cap_quanta: Quantum,
    pub cumulative_spent_quanta: Quantum,
    pub valid_until_slot: u64,
    pub principal_signature: [u8; 64],
}

impl AgentMandate {
    /// Menghasilkan komitmen intisari mandat yang harus disahkan oleh prinsipal.
    pub fn compute_mandate_digest(
        principal_address: &Address,
        agent_pubkey: &[u8; 32],
        allowed_operations: &[String],
        spending_cap: Quantum,
        valid_until_slot: u64,
    ) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-AGENT-MANDATE-V1");
        hasher.update(principal_address.as_bytes());
        hasher.update(agent_pubkey);
        for op in allowed_operations {
            hasher.update(op.as_bytes());
        }
        hasher.update(&spending_cap.as_u128().to_be_bytes());
        hasher.update(&valid_until_slot.to_be_bytes());
        *hasher.finalize().as_bytes()
    }

    /// Memverifikasi keabsahan tanda tangan prinsipal pada mandat.
    pub fn verify_principal_signature(&self) -> bool {
        let digest = Self::compute_mandate_digest(
            &self.principal_address,
            &self.agent_pubkey,
            &self.allowed_operations,
            self.spending_cap_quanta,
            self.valid_until_slot,
        );

        let vk = match VerifyingKey::from_bytes(&self.principal_pubkey) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let sig = Signature::from_bytes(&self.principal_signature);

        vk.verify(&digest, &sig).is_ok()
    }
}

/// Tindakan Eksekusi yang Didelegasikan oleh Agen AI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegatedAction {
    pub mandate_id: MandateId,
    pub operation: String,
    pub action_payload: Vec<u8>,
    pub cost_quanta: Quantum,
    pub action_nonce: u64,
    pub agent_signature: [u8; 64],
}

impl DelegatedAction {
    pub fn compute_action_digest(
        mandate_id: &[u8; 32],
        operation: &str,
        payload: &[u8],
        cost: Quantum,
        nonce: u64,
    ) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-DELEGATED-ACTION-V1");
        hasher.update(mandate_id);
        hasher.update(operation.as_bytes());
        hasher.update(payload);
        hasher.update(&cost.as_u128().to_be_bytes());
        hasher.update(&nonce.to_be_bytes());
        *hasher.finalize().as_bytes()
    }
}

/// Mesin Eksekutif Pengendali Otorisasi & Pendelegasian Agen AI.
#[derive(Debug, Default)]
pub struct AgentExecutive {
    mandates: BTreeMap<MandateId, AgentMandate>,
    used_action_nonces: BTreeMap<(MandateId, u64), bool>,
}

impl AgentExecutive {
    pub fn new() -> Self {
        Self {
            mandates: BTreeMap::new(),
            used_action_nonces: BTreeMap::new(),
        }
    }

    /// Mendaftarkan mandat baru yang telah diverifikasi tanda tangan prinsipalnya.
    pub fn register_mandate(&mut self, mandate: AgentMandate) -> Result<MandateId, &'static str> {
        if !mandate.verify_principal_signature() {
            return Err("Invalid principal signature on agent mandate");
        }

        let mid = mandate.mandate_id;
        self.mandates.insert(mid, mandate);
        Ok(mid)
    }

    /// Memvalidasi dan mengeksekusi tindakan terdelegasi agen dengan verifikasi spending cap dan expiry.
    pub fn execute_delegated_action(
        &mut self,
        action: &DelegatedAction,
        current_slot: u64,
    ) -> Result<Quantum, &'static str> {
        let mandate = self
            .mandates
            .get_mut(&action.mandate_id)
            .ok_or("Agent mandate not found")?;

        // 1. Periksa batas waktu kedaluwarsa
        if current_slot > mandate.valid_until_slot {
            return Err("Agent mandate has expired");
        }

        // 2. Periksa apakah operasi diizinkan
        if !mandate.allowed_operations.contains(&action.operation) {
            return Err("Requested operation is not permitted by mandate");
        }

        // 3. Periksa pencegahan replay nonce
        let nonce_key = (action.mandate_id, action.action_nonce);
        if self.used_action_nonces.contains_key(&nonce_key) {
            return Err("Action nonce already used for this mandate");
        }

        // 4. Periksa batas spending cap kumulatif
        let new_cumulative = mandate
            .cumulative_spent_quanta
            .checked_add(action.cost_quanta)
            .map_err(|_| "Arithmetic overflow in cumulative spending")?;

        if new_cumulative.as_u128() > mandate.spending_cap_quanta.as_u128() {
            return Err("Action cost exceeds remaining mandate spending cap");
        }

        // 5. Verifikasi tanda tangan agen
        let digest = DelegatedAction::compute_action_digest(
            &action.mandate_id,
            &action.operation,
            &action.action_payload,
            action.cost_quanta,
            action.action_nonce,
        );

        let vk = VerifyingKey::from_bytes(&mandate.agent_pubkey)
            .map_err(|_| "Invalid agent public key")?;
        let sig = Signature::from_bytes(&action.agent_signature);

        vk.verify(&digest, &sig)
            .map_err(|_| "Agent signature verification failed")?;

        // 6. Terapkan pemutakhiran state
        mandate.cumulative_spent_quanta = new_cumulative;
        self.used_action_nonces.insert(nonce_key, true);

        Ok(new_cumulative)
    }

    pub fn get_mandate(&self, mandate_id: &MandateId) -> Option<&AgentMandate> {
        self.mandates.get(mandate_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn test_agent_mandate_lifecycle_and_spending_cap() {
        let principal_sk = SigningKey::from_bytes(&[0x11; 32]);
        let principal_pk = principal_sk.verifying_key().to_bytes();
        let principal_addr = Address::from_bytes(principal_pk);

        let agent_sk = SigningKey::from_bytes(&[0x22; 32]);
        let agent_pk = agent_sk.verifying_key().to_bytes();

        let allowed_ops = vec!["QueryData".to_string(), "SubmitTask".to_string()];
        let spending_cap = Quantum::new(10_000);
        let valid_until = 5_000;

        let digest = AgentMandate::compute_mandate_digest(
            &principal_addr,
            &agent_pk,
            &allowed_ops,
            spending_cap,
            valid_until,
        );
        let principal_sig = principal_sk.sign(&digest).to_bytes();

        let mandate = AgentMandate {
            mandate_id: [0x55; 32],
            principal_address: principal_addr,
            principal_pubkey: principal_pk,
            agent_pubkey: agent_pk,
            allowed_operations: allowed_ops,
            spending_cap_quanta: spending_cap,
            cumulative_spent_quanta: Quantum::new(0),
            valid_until_slot: valid_until,
            principal_signature: principal_sig,
        };

        let mut exec = AgentExecutive::new();
        let mid = exec
            .register_mandate(mandate)
            .expect("Registration should succeed");

        // Action 1: Cost 4,000 Quanta
        let cost1 = Quantum::new(4_000);
        let action1_digest =
            DelegatedAction::compute_action_digest(&mid, "QueryData", b"query_params", cost1, 1);
        let action1 = DelegatedAction {
            mandate_id: mid,
            operation: "QueryData".to_string(),
            action_payload: b"query_params".to_vec(),
            cost_quanta: cost1,
            action_nonce: 1,
            agent_signature: agent_sk.sign(&action1_digest).to_bytes(),
        };

        assert!(exec.execute_delegated_action(&action1, 1_000).is_ok());
        assert_eq!(
            exec.get_mandate(&mid).unwrap().cumulative_spent_quanta,
            Quantum::new(4_000)
        );

        // Action 2: Cost 7,000 Quanta (Exceeds remaining 6,000 cap)
        let cost2 = Quantum::new(7_000);
        let action2_digest =
            DelegatedAction::compute_action_digest(&mid, "SubmitTask", b"task_params", cost2, 2);
        let action2 = DelegatedAction {
            mandate_id: mid,
            operation: "SubmitTask".to_string(),
            action_payload: b"task_params".to_vec(),
            cost_quanta: cost2,
            action_nonce: 2,
            agent_signature: agent_sk.sign(&action2_digest).to_bytes(),
        };

        assert!(exec.execute_delegated_action(&action2, 1_000).is_err());
    }
}
