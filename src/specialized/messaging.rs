//! Protokol Perpesanan Hierarkis & Relayer L1 <-> L2 <-> L3 (`src/specialized/messaging.rs`).
//! Mengelola transfer aset, perpesanan lintas layer/domain, nullifier registry anti-replay,
//! dan cross-domain event routing melalui L2 hub.
//! Mematuhi Invariant AUR-ARCH-011 (#![forbid(unsafe_code)]), AUR-ARCH-012 (Zero-Float Quantum u128),
//! AUR-L3-MSG-001 (Strict Replay Protection), dan AUR-L3-MSG-002 (Ordered Delivery Guarantee).

use crate::core::{Address, Hash256, Quantum};
use crate::crypto::blake3_hash;
use crate::specialized::state::L3AccountProof;
use crate::specialized::types::DomainId;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Domain Separation Tag untuk Nullifier L3
pub const DST_L3_NULLIFIER: &str = "AURION-L3-NULLIFIER-V1";

/// Kesalahan Perpesanan & Relayer L3
#[derive(Debug, Error, PartialEq, Eq)]
pub enum L3MessagingError {
    #[error(
        "Pesan sudah dieksekusi sebelumnya (Replay Attack dicegah oleh Nullifier Registry: {0})"
    )]
    DuplicateNullifier(Hash256),

    #[error("Nonce perpesanan tidak berurutan: diharapkan {expected}, aktual {actual}")]
    OutOfOrderNonce { expected: u64, actual: u64 },

    #[error("Domain tujuan tidak cocok: diharapkan {expected}, aktual {actual}")]
    DestinationMismatch {
        expected: DomainId,
        actual: DomainId,
    },

    #[error("Bukti Merkle inklusi pesan tidak valid")]
    InvalidMerkleProof,

    #[error("Jumlah transfer bernilai nol atau overflow")]
    InvalidTransferAmount,

    #[error("Operasi state internal gagal: {0}")]
    StateError(String),
}

/// Format Pesan Kanonikal 7-Elemen L1 <-> L2 <-> L3 (Rule 18 §4.3)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossLayerMessage {
    pub message_id: Hash256,
    pub source_domain: DomainId,
    pub destination_domain: DomainId,
    pub nonce: u64,
    pub payload: Vec<u8>,
    pub proof: Vec<u8>,
    pub nullifier: Hash256,
}

impl CrossLayerMessage {
    /// Membuat pesan baru dengan kalkulasi nullifier deterministik
    #[must_use]
    pub fn new(
        source_domain: DomainId,
        destination_domain: DomainId,
        nonce: u64,
        payload: Vec<u8>,
        proof: Vec<u8>,
    ) -> Self {
        // Hitung message_id berbasis Blake3
        let mut msg_data = Vec::with_capacity(32 + 32 + 8 + payload.len());
        msg_data.extend_from_slice(source_domain.as_bytes());
        msg_data.extend_from_slice(destination_domain.as_bytes());
        msg_data.extend_from_slice(&nonce.to_be_bytes());
        msg_data.extend_from_slice(&payload);
        let message_id = blake3_hash(&msg_data);

        // Hitung nullifier anti-replay unik (AUR-L3-MSG-001)
        let mut null_data = Vec::with_capacity(32 + 32 + 8);
        null_data.extend_from_slice(DST_L3_NULLIFIER.as_bytes());
        null_data.extend_from_slice(message_id.as_bytes());
        null_data.extend_from_slice(&nonce.to_be_bytes());
        let nullifier = blake3_hash(&null_data);

        Self {
            message_id,
            source_domain,
            destination_domain,
            nonce,
            payload,
            proof,
            nullifier,
        }
    }
}

/// Registry Nullifier Anti-Replay Lintas Layer (L3-TSK-402)
#[derive(Debug, Default, Clone)]
pub struct NullifierRegistry {
    spent_nullifiers: BTreeSet<Hash256>,
    channel_nonces: BTreeMap<(DomainId, DomainId), u64>,
}

impl NullifierRegistry {
    /// Membuat registry nullifier baru
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Memeriksa apakah nullifier sudah pernah digunakan
    #[must_use]
    pub fn is_spent(&self, nullifier: &Hash256) -> bool {
        self.spent_nullifiers.contains(nullifier)
    }

    /// Memvalidasi dan mendaftarkan nullifier serta memastikan pengurutan strictly monotonic (AUR-L3-MSG-002)
    pub fn verify_and_consume(&mut self, msg: &CrossLayerMessage) -> Result<(), L3MessagingError> {
        // 1. Cek duplikasi nullifier
        if self.is_spent(&msg.nullifier) {
            return Err(L3MessagingError::DuplicateNullifier(msg.nullifier));
        }

        // 2. Cek pengurutan nonce strictly monotonic per saluran domain
        let channel_key = (msg.source_domain, msg.destination_domain);
        let current_nonce = self.channel_nonces.get(&channel_key).copied().unwrap_or(0);
        let expected_nonce = current_nonce + 1;

        if msg.nonce != expected_nonce {
            return Err(L3MessagingError::OutOfOrderNonce {
                expected: expected_nonce,
                actual: msg.nonce,
            });
        }

        // 3. Catat konsumsi nullifier dan perbarui sequence nonce
        self.spent_nullifiers.insert(msg.nullifier);
        self.channel_nonces.insert(channel_key, expected_nonce);

        Ok(())
    }
}

/// Bukti Penarikan Dana L3 ke L2 (Withdrawal Proof)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L3WithdrawalProof {
    pub account_proof: L3AccountProof,
    pub recipient_l2: Address,
    pub amount: Quantum,
}

/// Relayer Dua Arah L2 <-> L3 (L3-TSK-401)
#[derive(Debug)]
pub struct L2L3TwoWayRelayer {
    pub domain_id: DomainId,
    pub nullifier_registry: NullifierRegistry,
    pub deposit_vault_l2: Quantum,
    channel_nonce_outbound: u64,
}

impl L2L3TwoWayRelayer {
    /// Membuat relayer baru untuk domain L3
    #[must_use]
    pub fn new(domain_id: DomainId) -> Self {
        Self {
            domain_id,
            nullifier_registry: NullifierRegistry::new(),
            deposit_vault_l2: Quantum::ZERO,
            channel_nonce_outbound: 0,
        }
    }

    /// Memproses deposit dari L2 ke L3 (Lock di L2 vault, siapkan paket pesan)
    pub fn create_deposit_message(
        &mut self,
        l2_domain: DomainId,
        recipient_l3: Address,
        amount: Quantum,
    ) -> Result<CrossLayerMessage, L3MessagingError> {
        if amount == Quantum::ZERO {
            return Err(L3MessagingError::InvalidTransferAmount);
        }

        self.deposit_vault_l2 = self
            .deposit_vault_l2
            .checked_add(amount)
            .map_err(|_| L3MessagingError::StateError("Overflow vault deposit L2".to_string()))?;

        self.channel_nonce_outbound += 1;

        let mut payload = Vec::with_capacity(32 + 16);
        payload.extend_from_slice(recipient_l3.as_bytes());
        payload.extend_from_slice(&amount.as_u128().to_be_bytes());

        let msg = CrossLayerMessage::new(
            l2_domain,
            self.domain_id,
            self.channel_nonce_outbound,
            payload,
            vec![],
        );

        Ok(msg)
    }

    /// Memproses penarikan dari L3 kembali ke L2 (Bakar di L3, Buka kunci vault L2)
    pub fn verify_withdrawal_and_unlock(
        &mut self,
        msg: &CrossLayerMessage,
        expected_state_root: Hash256,
        proof: &L3WithdrawalProof,
    ) -> Result<(Address, Quantum), L3MessagingError> {
        // 1. Verifikasi nullifier anti-replay
        self.nullifier_registry.verify_and_consume(msg)?;

        // 2. Verifikasi bukti inklusi akun terhadap state root L3 yang diselesaikan di L2
        if proof.account_proof.root != expected_state_root || !proof.account_proof.verify() {
            return Err(L3MessagingError::InvalidMerkleProof);
        }

        // 3. Buka kunci saldo dari deposit vault L2 (Konservasi nilai aset)
        self.deposit_vault_l2 = self
            .deposit_vault_l2
            .checked_sub(proof.amount)
            .map_err(|_| {
                L3MessagingError::StateError("Saldo deposit vault L2 tidak mencukupi".to_string())
            })?;

        Ok((proof.recipient_l2, proof.amount))
    }
}

/// Cross-Domain Event Router untuk komunikasi antar domain L3 independen (L3-TSK-403)
#[derive(Debug, Default)]
pub struct CrossDomainEventRouter {
    pub routed_messages: Vec<CrossLayerMessage>,
}

impl CrossDomainEventRouter {
    /// Membuat router baru
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Meneruskan pesan antar domain L3 melalui L2 Hub
    pub fn route_message(&mut self, message: CrossLayerMessage) {
        self.routed_messages.push(message);
    }

    /// Mengambil seluruh pesan tertunda untuk domain tertentu
    #[must_use]
    pub fn drain_messages_for_domain(&mut self, target_domain: DomainId) -> Vec<CrossLayerMessage> {
        let mut matching = Vec::new();
        let mut remaining = Vec::new();

        for msg in self.routed_messages.drain(..) {
            if msg.destination_domain == target_domain {
                matching.push(msg);
            } else {
                remaining.push(msg);
            }
        }

        self.routed_messages = remaining;
        matching
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_layer_message_and_nullifier_anti_replay() {
        let l2_domain = DomainId::from_bytes([0x02; 32]);
        let l3_domain = DomainId::DEX_DEFAULT;

        let mut registry = NullifierRegistry::new();

        let msg1 =
            CrossLayerMessage::new(l2_domain, l3_domain, 1, b"DEPOSIT_100_AUR".to_vec(), vec![]);

        // Konsumsi pertama harus berhasil
        registry
            .verify_and_consume(&msg1)
            .expect("Konsumsi msg1 pertama harus sukses");
        assert!(registry.is_spent(&msg1.nullifier));

        // Replay attack: konsumsi kedua dengan nullifier yang sama harus ditolak keras!
        let replay_err = registry.verify_and_consume(&msg1);
        assert!(matches!(
            replay_err,
            Err(L3MessagingError::DuplicateNullifier(_))
        ));

        // Pesan kedua dengan nonce out of order (misal loncat ke 3) harus ditolak
        let msg_bad_nonce =
            CrossLayerMessage::new(l2_domain, l3_domain, 3, b"DEPOSIT_200_AUR".to_vec(), vec![]);
        let nonce_err = registry.verify_and_consume(&msg_bad_nonce);
        assert!(matches!(
            nonce_err,
            Err(L3MessagingError::OutOfOrderNonce {
                expected: 2,
                actual: 3
            })
        ));
    }

    #[test]
    fn test_two_way_relayer_deposit_and_withdrawal() {
        let l2_domain = DomainId::from_bytes([0x02; 32]);
        let l3_domain = DomainId::APP_CHAIN_DEFAULT;

        let mut relayer = L2L3TwoWayRelayer::new(l3_domain);
        let alice_l3 = Address::from_bytes([0xaa; 32]);

        // 1. L2 -> L3 Deposit
        let deposit_msg = relayer
            .create_deposit_message(l2_domain, alice_l3, Quantum::new(500_000))
            .unwrap();
        assert_eq!(relayer.deposit_vault_l2, Quantum::new(500_000));
        assert_eq!(deposit_msg.nonce, 1);

        // 2. L3 -> L2 Withdrawal dengan Merkle Proof
        let mut state = crate::specialized::state::L3State::new(l3_domain);
        state.credit(&alice_l3, Quantum::new(500_000)).unwrap();
        let state_root = state.compute_state_root();
        let acct_proof = state.generate_account_proof(&alice_l3).unwrap();

        let withdrawal_proof = L3WithdrawalProof {
            account_proof: acct_proof,
            recipient_l2: Address::from_bytes([0xbb; 32]),
            amount: Quantum::new(200_000),
        };

        let withdrawal_msg = CrossLayerMessage::new(
            l3_domain,
            l2_domain,
            1,
            b"WITHDRAW_200_AUR".to_vec(),
            vec![],
        );

        let (recipient, unlocked) = relayer
            .verify_withdrawal_and_unlock(&withdrawal_msg, state_root, &withdrawal_proof)
            .expect("Verifikasi penarikan dan unlock harus berhasil");

        assert_eq!(recipient, Address::from_bytes([0xbb; 32]));
        assert_eq!(unlocked, Quantum::new(200_000));
        assert_eq!(relayer.deposit_vault_l2, Quantum::new(300_000)); // 500k - 200k = 300k
    }

    #[test]
    fn test_cross_domain_event_router() {
        let dex_domain = DomainId::DEX_DEFAULT;
        let gaming_domain = DomainId::GAMING_DEFAULT;
        let privacy_domain = DomainId::PRIVACY_DEFAULT;

        let mut router = CrossDomainEventRouter::new();

        let msg_to_dex = CrossLayerMessage::new(
            gaming_domain,
            dex_domain,
            1,
            b"BUY_IN_GAME_TOKEN".to_vec(),
            vec![],
        );
        let msg_to_privacy = CrossLayerMessage::new(
            dex_domain,
            privacy_domain,
            1,
            b"SHIELD_FUNDS".to_vec(),
            vec![],
        );

        router.route_message(msg_to_dex);
        router.route_message(msg_to_privacy);

        assert_eq!(router.routed_messages.len(), 2);

        let dex_msgs = router.drain_messages_for_domain(dex_domain);
        assert_eq!(dex_msgs.len(), 1);
        assert_eq!(dex_msgs[0].destination_domain, dex_domain);

        // Hanya tersisa 1 pesan untuk privacy
        assert_eq!(router.routed_messages.len(), 1);
    }
}
