//! Definisi dan Serializer/Deserializer ABI Kontrak L2SettlementBridge di Layer-1.
//! Sesuai Dokumen Spesifikasi: docs/Application-Rules-Layer/application/aurion-l2-scaling/01-L2-SETTLEMENT-BRIDGE-ABI-SPECIFICATION.md
//! Mematuhi Invariant: L2-SETTLE-001..005, AUR-ARCH-011 (#![forbid(unsafe_code)]), AUR-ARCH-012 (Zero-Float Quantum).

use crate::core::{Address, Hash256, Quantum};
use crate::crypto::blake3_hash;
use thiserror::Error;

/// Selector fungsi 4-byte kanonikal
pub const SELECTOR_DEPOSIT: [u8; 4] = [0x5D, 0x43, 0x7F, 0x01];
pub const SELECTOR_VERIFY_STATE_TRANSITION: [u8; 4] = [0x8A, 0x2C, 0x19, 0xE4];
pub const SELECTOR_WITHDRAW: [u8; 4] = [0x3B, 0x79, 0xF4, 0x18];
pub const SELECTOR_ENQUEUE_FORCED_TX: [u8; 4] = [0x1F, 0x80, 0xE7, 0x4C];
pub const SELECTOR_ESCAPE_HATCH_CLAIM: [u8; 4] = [0x7C, 0x92, 0x46, 0xD3];

/// Kesalahan decoding ABI calldata
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AbiError {
    #[error("Calldata terlalu pendek (diharapkan minimal {expected_at_least} byte, diterima {actual} byte)")]
    CalldataTooShort {
        expected_at_least: usize,
        actual: usize,
    },

    #[error("Selector fungsi tidak dikenal: {0:02X?}")]
    UnknownSelector([u8; 4]),

    #[error("Payload calldata tidak valid: {0}")]
    MalformedPayload(&'static str),

    #[error("Terdapat sisa byte tidak terduga pada calldata")]
    TrailingBytes,
}

/// Representasi panggilan metode resmi ke kontrak L2SettlementBridge
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeCall {
    Deposit {
        recipient_l2: Address,
        amount: Quantum,
    },
    VerifyStateTransition {
        batch_index: u64,
        prev_state_root: Hash256,
        new_state_root: Hash256,
        start_block: u64,
        end_block: u64,
        calldata_hash: Hash256,
    },
    Withdraw {
        recipient_l1: Address,
        amount: Quantum,
        leaf_index: u32,
        merkle_branch: Vec<Hash256>,
    },
    EnqueueForcedTx {
        payload: Vec<u8>,
    },
    EscapeHatchClaim {
        recipient_l1: Address,
        amount: Quantum,
        proof: Vec<u8>,
    },
}

impl BridgeCall {
    /// Mengodekan pemanggilan metode ke dalam format biner calldata kanonikal
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::Deposit {
                recipient_l2,
                amount,
            } => {
                let mut buf = Vec::with_capacity(4 + 32 + 16);
                buf.extend_from_slice(&SELECTOR_DEPOSIT);
                buf.extend_from_slice(recipient_l2.as_bytes());
                buf.extend_from_slice(&amount.as_u128().to_be_bytes());
                buf
            }
            Self::VerifyStateTransition {
                batch_index,
                prev_state_root,
                new_state_root,
                start_block,
                end_block,
                calldata_hash,
            } => {
                let mut buf = Vec::with_capacity(4 + 8 + 32 + 32 + 8 + 8 + 32);
                buf.extend_from_slice(&SELECTOR_VERIFY_STATE_TRANSITION);
                buf.extend_from_slice(&batch_index.to_be_bytes());
                buf.extend_from_slice(prev_state_root.as_bytes());
                buf.extend_from_slice(new_state_root.as_bytes());
                buf.extend_from_slice(&start_block.to_be_bytes());
                buf.extend_from_slice(&end_block.to_be_bytes());
                buf.extend_from_slice(calldata_hash.as_bytes());
                buf
            }
            Self::Withdraw {
                recipient_l1,
                amount,
                leaf_index,
                merkle_branch,
            } => {
                let mut buf = Vec::with_capacity(4 + 32 + 16 + 4 + 2 + merkle_branch.len() * 32);
                buf.extend_from_slice(&SELECTOR_WITHDRAW);
                buf.extend_from_slice(recipient_l1.as_bytes());
                buf.extend_from_slice(&amount.as_u128().to_be_bytes());
                buf.extend_from_slice(&leaf_index.to_be_bytes());
                let branch_len = u16::try_from(merkle_branch.len()).unwrap_or(u16::MAX);
                buf.extend_from_slice(&branch_len.to_be_bytes());
                for sibling in merkle_branch {
                    buf.extend_from_slice(sibling.as_bytes());
                }
                buf
            }
            Self::EnqueueForcedTx { payload } => {
                let mut buf = Vec::with_capacity(4 + 4 + payload.len());
                buf.extend_from_slice(&SELECTOR_ENQUEUE_FORCED_TX);
                let payload_len = u32::try_from(payload.len()).unwrap_or(u32::MAX);
                buf.extend_from_slice(&payload_len.to_be_bytes());
                buf.extend_from_slice(payload);
                buf
            }
            Self::EscapeHatchClaim {
                recipient_l1,
                amount,
                proof,
            } => {
                let mut buf = Vec::with_capacity(4 + 32 + 16 + 4 + proof.len());
                buf.extend_from_slice(&SELECTOR_ESCAPE_HATCH_CLAIM);
                buf.extend_from_slice(recipient_l1.as_bytes());
                buf.extend_from_slice(&amount.as_u128().to_be_bytes());
                let proof_len = u32::try_from(proof.len()).unwrap_or(u32::MAX);
                buf.extend_from_slice(&proof_len.to_be_bytes());
                buf.extend_from_slice(proof);
                buf
            }
        }
    }

    /// Mendekode calldata biner menjadi objek BridgeCall resmi
    pub fn decode(calldata: &[u8]) -> Result<Self, AbiError> {
        if calldata.len() < 4 {
            return Err(AbiError::CalldataTooShort {
                expected_at_least: 4,
                actual: calldata.len(),
            });
        }

        let selector: [u8; 4] = [calldata[0], calldata[1], calldata[2], calldata[3]];
        let body = &calldata[4..];

        match selector {
            SELECTOR_DEPOSIT => {
                if body.len() != 48 {
                    return Err(AbiError::CalldataTooShort {
                        expected_at_least: 52,
                        actual: calldata.len(),
                    });
                }
                let mut addr_bytes = [0u8; 32];
                addr_bytes.copy_from_slice(&body[0..32]);
                let recipient_l2 = Address::from_bytes(addr_bytes);

                let mut amount_bytes = [0u8; 16];
                amount_bytes.copy_from_slice(&body[32..48]);
                let amount = Quantum::new(u128::from_be_bytes(amount_bytes));

                Ok(Self::Deposit {
                    recipient_l2,
                    amount,
                })
            }
            SELECTOR_VERIFY_STATE_TRANSITION => {
                if body.len() != 120 {
                    return Err(AbiError::CalldataTooShort {
                        expected_at_least: 124,
                        actual: calldata.len(),
                    });
                }
                let mut batch_idx_bytes = [0u8; 8];
                batch_idx_bytes.copy_from_slice(&body[0..8]);
                let batch_index = u64::from_be_bytes(batch_idx_bytes);

                let mut prev_root_bytes = [0u8; 32];
                prev_root_bytes.copy_from_slice(&body[8..40]);
                let prev_state_root = Hash256::from_bytes(prev_root_bytes);

                let mut new_root_bytes = [0u8; 32];
                new_root_bytes.copy_from_slice(&body[40..72]);
                let new_state_root = Hash256::from_bytes(new_root_bytes);

                let mut start_blk_bytes = [0u8; 8];
                start_blk_bytes.copy_from_slice(&body[72..80]);
                let start_block = u64::from_be_bytes(start_blk_bytes);

                let mut end_blk_bytes = [0u8; 8];
                end_blk_bytes.copy_from_slice(&body[80..88]);
                let end_block = u64::from_be_bytes(end_blk_bytes);

                let mut calldata_h_bytes = [0u8; 32];
                calldata_h_bytes.copy_from_slice(&body[88..120]);
                let calldata_hash = Hash256::from_bytes(calldata_h_bytes);

                Ok(Self::VerifyStateTransition {
                    batch_index,
                    prev_state_root,
                    new_state_root,
                    start_block,
                    end_block,
                    calldata_hash,
                })
            }
            SELECTOR_WITHDRAW => {
                if body.len() < 54 {
                    return Err(AbiError::CalldataTooShort {
                        expected_at_least: 58,
                        actual: calldata.len(),
                    });
                }
                let mut addr_bytes = [0u8; 32];
                addr_bytes.copy_from_slice(&body[0..32]);
                let recipient_l1 = Address::from_bytes(addr_bytes);

                let mut amount_bytes = [0u8; 16];
                amount_bytes.copy_from_slice(&body[32..48]);
                let amount = Quantum::new(u128::from_be_bytes(amount_bytes));

                let mut leaf_idx_bytes = [0u8; 4];
                leaf_idx_bytes.copy_from_slice(&body[48..52]);
                let leaf_index = u32::from_be_bytes(leaf_idx_bytes);

                let mut branch_len_bytes = [0u8; 2];
                branch_len_bytes.copy_from_slice(&body[52..54]);
                let branch_len = usize::from(u16::from_be_bytes(branch_len_bytes));

                let expected_total_len = 54 + branch_len * 32;
                if body.len() != expected_total_len {
                    return Err(AbiError::MalformedPayload(
                        "Panjang cabang Merkle tidak cocok dengan isi body calldata",
                    ));
                }

                let mut merkle_branch = Vec::with_capacity(branch_len);
                let mut offset = 54;
                for _ in 0..branch_len {
                    let mut sibling_bytes = [0u8; 32];
                    sibling_bytes.copy_from_slice(&body[offset..offset + 32]);
                    merkle_branch.push(Hash256::from_bytes(sibling_bytes));
                    offset += 32;
                }

                Ok(Self::Withdraw {
                    recipient_l1,
                    amount,
                    leaf_index,
                    merkle_branch,
                })
            }
            SELECTOR_ENQUEUE_FORCED_TX => {
                if body.len() < 4 {
                    return Err(AbiError::CalldataTooShort {
                        expected_at_least: 8,
                        actual: calldata.len(),
                    });
                }
                let mut len_bytes = [0u8; 4];
                len_bytes.copy_from_slice(&body[0..4]);
                let payload_len = usize::try_from(u32::from_be_bytes(len_bytes)).unwrap_or(0);

                if body.len() != 4 + payload_len {
                    return Err(AbiError::MalformedPayload(
                        "Panjang payload transaksi paksa tidak konsisten dengan calldata",
                    ));
                }

                let payload = body[4..4 + payload_len].to_vec();
                Ok(Self::EnqueueForcedTx { payload })
            }
            SELECTOR_ESCAPE_HATCH_CLAIM => {
                if body.len() < 52 {
                    return Err(AbiError::CalldataTooShort {
                        expected_at_least: 56,
                        actual: calldata.len(),
                    });
                }
                let mut addr_bytes = [0u8; 32];
                addr_bytes.copy_from_slice(&body[0..32]);
                let recipient_l1 = Address::from_bytes(addr_bytes);

                let mut amount_bytes = [0u8; 16];
                amount_bytes.copy_from_slice(&body[32..48]);
                let amount = Quantum::new(u128::from_be_bytes(amount_bytes));

                let mut proof_len_bytes = [0u8; 4];
                proof_len_bytes.copy_from_slice(&body[48..52]);
                let proof_len = usize::try_from(u32::from_be_bytes(proof_len_bytes)).unwrap_or(0);

                if body.len() != 52 + proof_len {
                    return Err(AbiError::MalformedPayload(
                        "Panjang bukti escape hatch tidak konsisten dengan calldata",
                    ));
                }

                let proof = body[52..52 + proof_len].to_vec();
                Ok(Self::EscapeHatchClaim {
                    recipient_l1,
                    amount,
                    proof,
                })
            }
            unknown => Err(AbiError::UnknownSelector(unknown)),
        }
    }
}

/// Menghitung selector 4-byte kanonikal dari signature metode berbasis Blake3
#[must_use]
pub fn compute_method_selector(signature: &str) -> [u8; 4] {
    let hash = blake3_hash(signature.as_bytes());
    let mut selector = [0u8; 4];
    selector.copy_from_slice(&hash.as_bytes()[0..4]);
    selector
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deposit_roundtrip() {
        let recipient = Address::from_bytes([0xAA; 32]);
        let amount = Quantum::new(250_000_000); // 2.5 AUR

        let call = BridgeCall::Deposit {
            recipient_l2: recipient,
            amount,
        };

        let encoded = call.encode();
        assert_eq!(&encoded[0..4], &SELECTOR_DEPOSIT);
        assert_eq!(encoded.len(), 52);

        let decoded = BridgeCall::decode(&encoded).expect("Decode harus berhasil");
        assert_eq!(decoded, call);
    }

    #[test]
    fn test_verify_state_transition_roundtrip() {
        let prev_root = Hash256::from_bytes([0x11; 32]);
        let new_root = Hash256::from_bytes([0x22; 32]);
        let calldata_hash = Hash256::from_bytes([0x33; 32]);

        let call = BridgeCall::VerifyStateTransition {
            batch_index: 42,
            prev_state_root: prev_root,
            new_state_root: new_root,
            start_block: 100,
            end_block: 110,
            calldata_hash,
        };

        let encoded = call.encode();
        assert_eq!(&encoded[0..4], &SELECTOR_VERIFY_STATE_TRANSITION);
        assert_eq!(encoded.len(), 124);

        let decoded = BridgeCall::decode(&encoded).expect("Decode harus berhasil");
        assert_eq!(decoded, call);
    }

    #[test]
    fn test_withdraw_roundtrip() {
        let recipient = Address::from_bytes([0xBB; 32]);
        let amount = Quantum::new(100_000_000); // 1 AUR
        let sibling1 = Hash256::from_bytes([0x01; 32]);
        let sibling2 = Hash256::from_bytes([0x02; 32]);

        let call = BridgeCall::Withdraw {
            recipient_l1: recipient,
            amount,
            leaf_index: 7,
            merkle_branch: vec![sibling1, sibling2],
        };

        let encoded = call.encode();
        assert_eq!(&encoded[0..4], &SELECTOR_WITHDRAW);
        assert_eq!(encoded.len(), 58 + 64);

        let decoded = BridgeCall::decode(&encoded).expect("Decode harus berhasil");
        assert_eq!(decoded, call);
    }

    #[test]
    fn test_enqueue_forced_tx_roundtrip() {
        let payload = vec![0xCA, 0xFE, 0xBA, 0xBE, 0x01, 0x02];
        let call = BridgeCall::EnqueueForcedTx { payload };

        let encoded = call.encode();
        assert_eq!(&encoded[0..4], &SELECTOR_ENQUEUE_FORCED_TX);
        assert_eq!(encoded.len(), 4 + 4 + 6);

        let decoded = BridgeCall::decode(&encoded).expect("Decode harus berhasil");
        assert_eq!(decoded, call);
    }

    #[test]
    fn test_unknown_selector_rejection() {
        let bogus = vec![0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x01];
        let err = BridgeCall::decode(&bogus).expect_err("Harus gagal pada selector tidak dikenal");
        assert_eq!(err, AbiError::UnknownSelector([0xFF, 0xFF, 0xFF, 0xFF]));
    }

    #[test]
    fn test_calldata_too_short() {
        let short = vec![0x5D, 0x43];
        let err = BridgeCall::decode(&short).expect_err("Harus gagal calldata terlalu pendek");
        assert_eq!(
            err,
            AbiError::CalldataTooShort {
                expected_at_least: 4,
                actual: 2
            }
        );
    }
}
