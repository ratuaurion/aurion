#![forbid(unsafe_code)]

//! Kliring Ekonomi Mesin-ke-Mesin Otonom (M2M Settlement) L5 (REQ-L5-08).
//! Invariant: AUR-L5-PREC-001 (Zero-Float Quantum), AUR-L5-ARCH-001 (Non-Consensus Off-Chain Clearing).

use std::collections::BTreeMap;
use blake3::Hasher;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use crate::primitives::core::Quantum;

/// Pengenal Unik Perangkat Otonom / Sensor / Server (Blake3 Device ID).
pub type DeviceId = [u8; 32];

/// Metrik Satuan Layanan Mesin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ServiceMetric {
    PerByte = 0x01,
    PerSecond = 0x02,
    PerQuery = 0x03,
    PerComputeUnit = 0x04,
}

/// Kontrak Kliring Terprogram Antar Perangkat Mesin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct M2MContract {
    pub contract_id: [u8; 32],
    pub provider_device: DeviceId,
    pub consumer_device: DeviceId,
    pub consumer_pubkey: [u8; 32],
    pub metric: ServiceMetric,
    pub rate_quanta_per_unit: Quantum,
    pub max_budget_quanta: Quantum,
    pub settled_quanta: Quantum,
    pub active: bool,
}

impl M2MContract {
    pub fn new(
        provider_device: DeviceId,
        consumer_device: DeviceId,
        consumer_pubkey: [u8; 32],
        metric: ServiceMetric,
        rate_quanta_per_unit: Quantum,
        max_budget_quanta: Quantum,
    ) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-M2M-CONTRACT-V1");
        hasher.update(&provider_device);
        hasher.update(&consumer_device);
        hasher.update(&[metric as u8]);
        hasher.update(&rate_quanta_per_unit.as_u128().to_be_bytes());
        hasher.update(&max_budget_quanta.as_u128().to_be_bytes());
        let contract_id = *hasher.finalize().as_bytes();

        Self {
            contract_id,
            provider_device,
            consumer_device,
            consumer_pubkey,
            metric,
            rate_quanta_per_unit,
            max_budget_quanta,
            settled_quanta: Quantum::new(0),
            active: true,
        }
    }
}

/// Tanda Terima Penggunaan Layanan Bermeteran (Metered Usage Receipt).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeteredUsageReceipt {
    pub contract_id: [u8; 32],
    pub units_consumed: u64,
    pub total_owed_quanta: Quantum,
    pub nonce: u64,
    pub consumer_signature: [u8; 64],
}

impl MeteredUsageReceipt {
    pub fn compute_digest(
        contract_id: &[u8; 32],
        units: u64,
        total_owed: Quantum,
        nonce: u64,
    ) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-M2M-RECEIPT-V1");
        hasher.update(contract_id);
        hasher.update(&units.to_be_bytes());
        hasher.update(&total_owed.as_u128().to_be_bytes());
        hasher.update(&nonce.to_be_bytes());
        *hasher.finalize().as_bytes()
    }

    pub fn verify_signature(&self, consumer_pubkey: &[u8; 32]) -> bool {
        let digest = Self::compute_digest(
            &self.contract_id,
            self.units_consumed,
            self.total_owed_quanta,
            self.nonce,
        );

        let vk = match VerifyingKey::from_bytes(consumer_pubkey) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let sig = Signature::from_bytes(&self.consumer_signature);

        vk.verify(&digest, &sig).is_ok()
    }
}

/// Balai Kliring M2M Otonom L5 (M2M Clearing House).
#[derive(Debug, Default)]
pub struct M2MClearingHouse {
    contracts: BTreeMap<[u8; 32], M2MContract>,
    latest_nonces: BTreeMap<[u8; 32], u64>,
}

impl M2MClearingHouse {
    pub fn new() -> Self {
        Self {
            contracts: BTreeMap::new(),
            latest_nonces: BTreeMap::new(),
        }
    }

    pub fn register_contract(&mut self, contract: M2MContract) -> [u8; 32] {
        let cid = contract.contract_id;
        self.contracts.insert(cid, contract);
        cid
    }

    /// Memproses kliring instan berdasarkan penggunaan bermeteran.
    pub fn clear_metered_usage(
        &mut self,
        receipt: &MeteredUsageReceipt,
    ) -> Result<Quantum, &'static str> {
        let contract = self
            .contracts
            .get_mut(&receipt.contract_id)
            .ok_or("M2M contract not found")?;

        if !contract.active {
            return Err("M2M contract is not active");
        }

        let last_nonce = self.latest_nonces.get(&receipt.contract_id).copied().unwrap_or(0);
        if receipt.nonce <= last_nonce {
            return Err("Nonce must be strictly increasing");
        }

        // Hitung biaya ekspektasi: units * rate
        let expected_cost = (receipt.units_consumed as u128)
            .checked_mul(contract.rate_quanta_per_unit.as_u128())
            .ok_or("Arithmetic overflow in M2M cost computation")?;

        if receipt.total_owed_quanta.as_u128() != expected_cost {
            return Err("Reported owed quanta does not match metered rate calculation");
        }

        // Periksa batas anggaran
        let new_settled = contract
            .settled_quanta
            .checked_add(receipt.total_owed_quanta)
            .map_err(|_| "Overflow in settled quanta")?;

        if new_settled.as_u128() > contract.max_budget_quanta.as_u128() {
            return Err("M2M usage exceeds contract max budget cap");
        }

        // Verifikasi tanda tangan
        if !receipt.verify_signature(&contract.consumer_pubkey) {
            return Err("Invalid consumer signature on M2M usage receipt");
        }

        contract.settled_quanta = new_settled;
        self.latest_nonces.insert(receipt.contract_id, receipt.nonce);

        Ok(contract.settled_quanta)
    }

    pub fn get_contract(&self, contract_id: &[u8; 32]) -> Option<&M2MContract> {
        self.contracts.get(contract_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn test_m2m_clearing_success() {
        let consumer_sk = SigningKey::from_bytes(&[0x77; 32]);
        let consumer_pk = consumer_sk.verifying_key().to_bytes();

        let mut house = M2MClearingHouse::new();
        let rate = Quantum::new(5); // 5 Quanta per Byte
        let budget = Quantum::new(100_000); // 100.000 Quanta max budget

        let contract = M2MContract::new(
            [0x11; 32],
            [0x22; 32],
            consumer_pk,
            ServiceMetric::PerByte,
            rate,
            budget,
        );
        let cid = house.register_contract(contract);

        // Usage: 1,000 bytes -> 5,000 Quanta
        let units = 1_000;
        let total_owed = Quantum::new(5_000);
        let digest = MeteredUsageReceipt::compute_digest(&cid, units, total_owed, 1);
        let sig = consumer_sk.sign(&digest).to_bytes();

        let receipt = MeteredUsageReceipt {
            contract_id: cid,
            units_consumed: units,
            total_owed_quanta: total_owed,
            nonce: 1,
            consumer_signature: sig,
        };

        let settled = house.clear_metered_usage(&receipt).expect("Clearing should succeed");
        assert_eq!(settled, Quantum::new(5_000));
        assert_eq!(house.get_contract(&cid).unwrap().settled_quanta, Quantum::new(5_000));
    }

    #[test]
    fn test_m2m_clearing_budget_exceeded_rejected() {
        let consumer_sk = SigningKey::from_bytes(&[0x77; 32]);
        let consumer_pk = consumer_sk.verifying_key().to_bytes();

        let mut house = M2MClearingHouse::new();
        let rate = Quantum::new(10);
        let budget = Quantum::new(500); // Max 500

        let contract = M2MContract::new(
            [0x11; 32],
            [0x22; 32],
            consumer_pk,
            ServiceMetric::PerQuery,
            rate,
            budget,
        );
        let cid = house.register_contract(contract);

        // Usage: 60 queries -> 600 Quanta (exceeds 500)
        let units = 60;
        let total_owed = Quantum::new(600);
        let digest = MeteredUsageReceipt::compute_digest(&cid, units, total_owed, 1);
        let sig = consumer_sk.sign(&digest).to_bytes();

        let receipt = MeteredUsageReceipt {
            contract_id: cid,
            units_consumed: units,
            total_owed_quanta: total_owed,
            nonce: 1,
            consumer_signature: sig,
        };

        assert!(house.clear_metered_usage(&receipt).is_err());
    }
}
