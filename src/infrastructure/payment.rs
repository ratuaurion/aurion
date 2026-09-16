#![forbid(unsafe_code)]

//! Kanal Pembayaran Streaming Mikro (State Channels) L5 (REQ-L5-07).
//! Invariant: AUR-L5-PREC-001 (Zero-Float Quantum), AUR-L5-PREC-002 (Exact Balance Conservation).

use std::collections::BTreeMap;
use blake3::Hasher;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use crate::primitives::core::{Address, Quantum};

/// Pengenal Unik Kanal Pembayaran Streaming.
pub type PaymentChannelId = [u8; 32];

/// Status Operasional Kanal Pembayaran Streaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelStatus {
    Open,
    Disputed,
    Closed,
}

/// Bukti Saldo Kumulatif Off-Chain Ditandatangani Pengirim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OffChainBalanceProof {
    pub channel_id: PaymentChannelId,
    pub cumulative_amount_quanta: Quantum,
    pub nonce: u64,
    pub signature: [u8; 64],
}

impl OffChainBalanceProof {
    pub fn compute_digest(
        channel_id: &[u8; 32],
        cumulative_amount: Quantum,
        nonce: u64,
    ) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-STREAMING-PAYMENT-PROOF-V1");
        hasher.update(channel_id);
        hasher.update(&cumulative_amount.as_u128().to_be_bytes());
        hasher.update(&nonce.to_be_bytes());
        *hasher.finalize().as_bytes()
    }

    pub fn verify_signature(&self, sender_pubkey: &[u8; 32]) -> bool {
        let digest = Self::compute_digest(
            &self.channel_id,
            self.cumulative_amount_quanta,
            self.nonce,
        );

        let vk = match VerifyingKey::from_bytes(sender_pubkey) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let sig = Signature::from_bytes(&self.signature);

        vk.verify(&digest, &sig).is_ok()
    }
}

/// Struktur Data Kanal Pembayaran Streaming Terbuka.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamingChannel {
    pub channel_id: PaymentChannelId,
    pub sender: Address,
    pub sender_pubkey: [u8; 32],
    pub recipient: Address,
    pub total_deposit_quanta: Quantum,
    pub current_transferred_quanta: Quantum,
    pub latest_nonce: u64,
    pub status: ChannelStatus,
    pub dispute_deadline_slot: Option<u64>,
}

/// Mesin Orkestrator Pembayaran Streaming Sub-Penny L5.
#[derive(Debug, Default)]
pub struct StreamingPaymentEngine {
    channels: BTreeMap<PaymentChannelId, StreamingChannel>,
    total_locked_deposit: Quantum,
}

impl StreamingPaymentEngine {
    pub fn new() -> Self {
        Self {
            channels: BTreeMap::new(),
            total_locked_deposit: Quantum::new(0),
        }
    }

    /// Membuka kanal pembayaran streaming baru dengan mengunci deposit jaminan Quantum.
    pub fn open_channel(
        &mut self,
        sender: Address,
        sender_pubkey: [u8; 32],
        recipient: Address,
        deposit: Quantum,
    ) -> Result<PaymentChannelId, &'static str> {
        if deposit.as_u128() == 0 {
            return Err("Channel deposit must be greater than zero");
        }

        let mut hasher = Hasher::new();
        hasher.update(b"AURION-L5-STREAMING-CHANNEL-ID-V1");
        hasher.update(sender.as_bytes());
        hasher.update(recipient.as_bytes());
        hasher.update(&deposit.as_u128().to_be_bytes());
        hasher.update(&(self.channels.len() as u64).to_be_bytes());
        let channel_id = *hasher.finalize().as_bytes();

        let channel = StreamingChannel {
            channel_id,
            sender,
            sender_pubkey,
            recipient,
            total_deposit_quanta: deposit,
            current_transferred_quanta: Quantum::new(0),
            latest_nonce: 0,
            status: ChannelStatus::Open,
            dispute_deadline_slot: None,
        };

        self.total_locked_deposit = self
            .total_locked_deposit
            .checked_add(deposit)
            .map_err(|_| "Overflow in total locked deposit")?;

        self.channels.insert(channel_id, channel);
        Ok(channel_id)
    }

    /// Memproses bukti transfer streaming off-chain (sub-penny tick).
    pub fn process_streaming_tick(
        &mut self,
        proof: &OffChainBalanceProof,
    ) -> Result<Quantum, &'static str> {
        let channel = self
            .channels
            .get_mut(&proof.channel_id)
            .ok_or("Channel not found")?;

        if channel.status != ChannelStatus::Open {
            return Err("Channel is not open for streaming");
        }

        if proof.nonce <= channel.latest_nonce {
            return Err("Nonce must be strictly increasing");
        }

        if proof.cumulative_amount_quanta.as_u128() > channel.total_deposit_quanta.as_u128() {
            return Err("Cumulative transferred amount exceeds total deposit");
        }

        if !proof.verify_signature(&channel.sender_pubkey) {
            return Err("Invalid signature on off-chain balance proof");
        }

        channel.current_transferred_quanta = proof.cumulative_amount_quanta;
        channel.latest_nonce = proof.nonce;

        Ok(channel.current_transferred_quanta)
    }

    /// Menyelesaikan dan menutup kanal secara kooperatif (Cooperative Close).
    pub fn cooperative_close(
        &mut self,
        proof: &OffChainBalanceProof,
    ) -> Result<(Quantum, Quantum), &'static str> {
        self.process_streaming_tick(proof)?;

        let channel = self
            .channels
            .get_mut(&proof.channel_id)
            .ok_or("Channel not found")?;

        channel.status = ChannelStatus::Closed;

        let payout_to_recipient = channel.current_transferred_quanta;
        let refund_to_sender = channel
            .total_deposit_quanta
            .checked_sub(payout_to_recipient)
            .map_err(|_| "Underflow in refund calculation")?;

        self.total_locked_deposit = self
            .total_locked_deposit
            .checked_sub(channel.total_deposit_quanta)
            .unwrap_or(Quantum::new(0));

        Ok((payout_to_recipient, refund_to_sender))
    }

    /// Mengaudit invariant konservasi saldo (AUR-L5-PREC-002: Exact Balance Conservation).
    pub fn audit_balance_conservation(&self) -> bool {
        let mut sum_deposits: u128 = 0;
        for c in self.channels.values() {
            if c.status != ChannelStatus::Closed {
                sum_deposits = match sum_deposits.checked_add(c.total_deposit_quanta.as_u128()) {
                    Some(s) => s,
                    None => return false,
                };
            }
        }
        sum_deposits == self.total_locked_deposit.as_u128()
    }

    pub fn get_channel(&self, id: &PaymentChannelId) -> Option<&StreamingChannel> {
        self.channels.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn test_streaming_payment_lifecycle_and_balance_conservation() {
        let sender_sk = SigningKey::from_bytes(&[0x33; 32]);
        let sender_pk = sender_sk.verifying_key().to_bytes();
        let sender_addr = Address::from_bytes(sender_pk);
        let recipient_addr = Address::from_bytes([0x44; 32]);

        let mut engine = StreamingPaymentEngine::new();
        let deposit = Quantum::new(1_000_000); // 1.000.000 Quanta

        let ch_id = engine
            .open_channel(sender_addr, sender_pk, recipient_addr, deposit)
            .expect("Channel opening should succeed");

        assert!(engine.audit_balance_conservation());

        // Stream tick 1: 250,000 Quanta
        let amount1 = Quantum::new(250_000);
        let digest1 = OffChainBalanceProof::compute_digest(&ch_id, amount1, 1);
        let proof1 = OffChainBalanceProof {
            channel_id: ch_id,
            cumulative_amount_quanta: amount1,
            nonce: 1,
            signature: sender_sk.sign(&digest1).to_bytes(),
        };
        assert!(engine.process_streaming_tick(&proof1).is_ok());

        // Stream tick 2: 600,000 Quanta
        let amount2 = Quantum::new(600_000);
        let digest2 = OffChainBalanceProof::compute_digest(&ch_id, amount2, 2);
        let proof2 = OffChainBalanceProof {
            channel_id: ch_id,
            cumulative_amount_quanta: amount2,
            nonce: 2,
            signature: sender_sk.sign(&digest2).to_bytes(),
        };
        let (payout, refund) = engine
            .cooperative_close(&proof2)
            .expect("Cooperative close must succeed");

        assert_eq!(payout, Quantum::new(600_000));
        assert_eq!(refund, Quantum::new(400_000));
        assert_eq!(payout.as_u128() + refund.as_u128(), deposit.as_u128());
        assert!(engine.audit_balance_conservation());
    }

    #[test]
    fn test_streaming_proof_exceeding_deposit_rejected() {
        let sender_sk = SigningKey::from_bytes(&[0x33; 32]);
        let sender_pk = sender_sk.verifying_key().to_bytes();
        let mut engine = StreamingPaymentEngine::new();
        let deposit = Quantum::new(500);

        let ch_id = engine
            .open_channel(
                Address::from_bytes(sender_pk),
                sender_pk,
                Address::from_bytes([0x44; 32]),
                deposit,
            )
            .unwrap();

        let over_amount = Quantum::new(600); // 600 > 500
        let digest = OffChainBalanceProof::compute_digest(&ch_id, over_amount, 1);
        let proof = OffChainBalanceProof {
            channel_id: ch_id,
            cumulative_amount_quanta: over_amount,
            nonce: 1,
            signature: sender_sk.sign(&digest).to_bytes(),
        };

        assert!(engine.process_streaming_tick(&proof).is_err());
    }
}
