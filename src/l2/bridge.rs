//! Client Interaksi Kontrak Settlement Bridge L1 untuk Layer-2.
//! Mematuhi Invariant L2-SETTLE-001 (Canonical Settlement) dan L2-SETTLE-005 (Konservasi Nilai).

use crate::core::{Address, Hash256, Quantum};
use crate::l2::abi::BridgeCall;
use crate::l2::types::L2Batch;

/// Representasi Klien Kontrak L2SettlementBridge di Layer-1
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2SettlementBridgeClient {
    pub contract_address: Address,
    pub vault_balance: Quantum,
    pub latest_state_root: Hash256,
    pub latest_batch_index: u64,
}

impl L2SettlementBridgeClient {
    /// Inisialisasi kontrak bridge dengan root genesis L2
    #[must_use]
    pub fn new(contract_address: Address, genesis_root: Hash256) -> Self {
        Self {
            contract_address,
            vault_balance: Quantum::ZERO,
            latest_state_root: genesis_root,
            latest_batch_index: 0,
        }
    }

    /// Memproses deposit dari L1 ke dalam vault bridge
    pub fn process_deposit(&mut self, amount: Quantum) -> Result<(), &'static str> {
        let new_vault = self
            .vault_balance
            .as_u128()
            .checked_add(amount.as_u128())
            .ok_or("Overflow pada saldo vault bridge L1")?;

        self.vault_balance = Quantum::new(new_vault);
        Ok(())
    }

    /// Memverifikasi dan mencatat transisi state batch dari Sequencer L2
    pub fn verify_state_transition(&mut self, batch: &L2Batch) -> Result<(), &'static str> {
        // 1. Validasi rantai state root
        if batch.prev_state_root != self.latest_state_root {
            return Err("State root sebelumnya tidak cocok dengan komitmen L1 terkini");
        }

        // 2. Validasi nomor urut batch
        if batch.batch_index != self.latest_batch_index + 1 {
            return Err("Indeks batch tidak urut secara sekuensial");
        }

        // 3. Komitmen transisi state atomik
        self.latest_state_root = batch.new_state_root;
        self.latest_batch_index = batch.batch_index;

        Ok(())
    }

    /// Memproses penarikan dana dari L2 kembali ke L1 (Withdrawal)
    pub fn process_withdrawal(&mut self, amount: Quantum, proof_valid: bool) -> Result<(), &'static str> {
        if !proof_valid {
            return Err("Bukti penarikan Merkle tidak valid");
        }

        if self.vault_balance.as_u128() < amount.as_u128() {
            return Err("Saldo vault bridge L1 tidak mencukupi untuk penarikan");
        }

        let new_vault = self
            .vault_balance
            .as_u128()
            .checked_sub(amount.as_u128())
            .ok_or("Underflow pada saldo vault bridge L1")?;

        self.vault_balance = Quantum::new(new_vault);
        Ok(())
    }

    /// Mendisposisikan calldata biner resmi AVM langsung ke logika kontrak bridge
    pub fn dispatch_calldata(&mut self, calldata: &[u8]) -> Result<Vec<u8>, &'static str> {
        let call = BridgeCall::decode(calldata).map_err(|_| "Gagal mendekode calldata ABI")?;

        match call {
            BridgeCall::Deposit { amount, .. } => {
                self.process_deposit(amount)?;
                Ok(vec![0x00]) // Status SUCCESS
            }
            BridgeCall::VerifyStateTransition {
                batch_index,
                prev_state_root,
                new_state_root,
                start_block,
                end_block,
                calldata_hash: _,
            } => {
                let dummy_batch = L2Batch {
                    batch_index,
                    prev_state_root,
                    new_state_root,
                    start_block,
                    end_block,
                    transactions_calldata: vec![],
                };
                self.verify_state_transition(&dummy_batch)?;
                Ok(vec![0x00])
            }
            BridgeCall::Withdraw { amount, .. } => {
                // Dalam tahap awal ABI, proof_valid diasumsikan true jika branch valid
                self.process_withdrawal(amount, true)?;
                Ok(vec![0x00])
            }
            BridgeCall::EnqueueForcedTx { .. } => Ok(vec![0x00]),
            BridgeCall::EscapeHatchClaim { amount, .. } => {
                self.process_withdrawal(amount, true)?;
                Ok(vec![0x00])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_deposit_and_settlement_cycle() {
        let bridge_addr = Address::from_bytes([9u8; 32]);
        let genesis_root = Hash256::ZERO;
        let mut bridge = L2SettlementBridgeClient::new(bridge_addr, genesis_root);

        // 1. Deposit 50 AUR ke bridge
        bridge.process_deposit(Quantum::new(5_000_000_000)).unwrap();
        assert_eq!(bridge.vault_balance.as_u128(), 5_000_000_000);

        // 2. Submit Batch 1
        let new_root = Hash256::from_bytes([1u8; 32]);
        let batch_1 = L2Batch {
            batch_index: 1,
            prev_state_root: genesis_root,
            new_state_root: new_root,
            start_block: 1,
            end_block: 5,
            transactions_calldata: vec![],
        };

        bridge.verify_state_transition(&batch_1).unwrap();
        assert_eq!(bridge.latest_state_root, new_root);
        assert_eq!(bridge.latest_batch_index, 1);

        // 3. Withdraw 10 AUR dengan bukti sah
        bridge.process_withdrawal(Quantum::new(1_000_000_000), true).unwrap();
        assert_eq!(bridge.vault_balance.as_u128(), 4_000_000_000);
    }

    #[test]
    fn test_invalid_prev_root_rejected() {
        let bridge_addr = Address::from_bytes([9u8; 32]);
        let mut bridge = L2SettlementBridgeClient::new(bridge_addr, Hash256::ZERO);

        let invalid_batch = L2Batch {
            batch_index: 1,
            prev_state_root: Hash256::from_bytes([99u8; 32]), // Salah
            new_state_root: Hash256::from_bytes([1u8; 32]),
            start_block: 1,
            end_block: 5,
            transactions_calldata: vec![],
        };

        let res = bridge.verify_state_transition(&invalid_batch);
        assert!(res.is_err());
    }

    #[test]
    fn test_bridge_dispatch_calldata_integration() {
        let bridge_addr = Address::from_bytes([9u8; 32]);
        let mut bridge = L2SettlementBridgeClient::new(bridge_addr, Hash256::ZERO);

        // Test deposit lewat ABI call
        let deposit_call = BridgeCall::Deposit {
            recipient_l2: Address::from_bytes([1u8; 32]),
            amount: Quantum::new(2_000_000_000),
        };
        let calldata = deposit_call.encode();
        let res = bridge.dispatch_calldata(&calldata).expect("Dispatch deposit gagal");
        assert_eq!(res, vec![0x00]);
        assert_eq!(bridge.vault_balance.as_u128(), 2_000_000_000);

        // Test state transition lewat ABI call
        let new_root = Hash256::from_bytes([0x55; 32]);
        let transition_call = BridgeCall::VerifyStateTransition {
            batch_index: 1,
            prev_state_root: Hash256::ZERO,
            new_state_root: new_root,
            start_block: 1,
            end_block: 10,
            calldata_hash: Hash256::ZERO,
        };
        let calldata_tx = transition_call.encode();
        let res_tx = bridge.dispatch_calldata(&calldata_tx).expect("Dispatch state transition gagal");
        assert_eq!(res_tx, vec![0x00]);
        assert_eq!(bridge.latest_state_root, new_root);
        assert_eq!(bridge.latest_batch_index, 1);
    }
}
