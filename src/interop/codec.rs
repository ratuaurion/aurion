#![forbid(unsafe_code)]

//! Aurion Layer-4 (L4) Canonical Wire Envelope Codec.
//!
//! Complies strictly with:
//! - AUR-ARCH-005: Deterministic Canonical Big-Endian Serialization.
//! - AUR-ARCH-011: Absolute Zero Unsafe Code.
//! - AUR-ARCH-012: Absolute Zero Float Arithmetic (Quantum u128).
//! - AUR-L4-ARCH-002: Universal Cross-Domain Envelope Standard.

use crate::interop::types::{
    BridgeStatus, ChainId, CrossChainMessage, ProofPayload, ProtocolId, RouteDescriptor,
    MAX_L4_PAYLOAD_BYTES,
};
use crate::primitives::core::Quantum;

/// Canonical 4-byte wire frame magic for Layer-4 envelopes ("AUL4").
pub const L4_WIRE_MAGIC: [u8; 4] = [0x41, 0x55, 0x4C, 0x34];

/// Protocol version byte.
pub const L4_CODEC_VERSION: u8 = 0x01;

/// Fixed-size header length for the L4 envelope (168 bytes).
pub const L4_HEADER_BYTES: usize = 168;

/// Encodes an L4 cross-chain message envelope into canonical big-endian wire bytes.
pub fn encode_envelope(
    msg: &CrossChainMessage,
    status: BridgeStatus,
) -> Result<Vec<u8>, &'static str> {
    if msg.payload.len() > MAX_L4_PAYLOAD_BYTES {
        return Err("Payload exceeds maximum allowable size (64 KB)");
    }

    let encoded_proof = encode_proof_payload(&msg.proof);
    if encoded_proof.len() > 8192 {
        return Err("Proof exceeds maximum allowable size (8 KB)");
    }

    let mut buf = Vec::with_capacity(L4_HEADER_BYTES + msg.payload.len() + encoded_proof.len());

    // 1. Magic & Version
    buf.extend_from_slice(&L4_WIRE_MAGIC);
    buf.push(L4_CODEC_VERSION);

    // 2. Protocol, HopCount, BridgeStatus
    buf.push(msg.route.protocol as u8);
    buf.push(msg.route.hop_count);
    buf.push(status as u8);

    // 3. Routing & Chains
    buf.extend_from_slice(&msg.source_chain.to_u64().to_be_bytes());
    buf.extend_from_slice(&msg.destination_chain.to_u64().to_be_bytes());
    buf.extend_from_slice(&msg.sequence_nonce.to_be_bytes());
    buf.extend_from_slice(&msg.timeout_timestamp.to_be_bytes());

    // 4. Gas & Quantum Fee (u128)
    buf.extend_from_slice(&msg.route.gas_limit.to_be_bytes());
    buf.extend_from_slice(&msg.route.max_fee.as_u128().to_be_bytes());

    // 5. Identifiers (Sender, Target, PacketId)
    buf.extend_from_slice(&msg.sender);
    buf.extend_from_slice(&msg.target_contract);
    buf.extend_from_slice(&msg.packet_id);

    // 6. Body Lengths
    buf.extend_from_slice(&(msg.payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(&(encoded_proof.len() as u32).to_be_bytes());

    // 7. Body Payloads
    buf.extend_from_slice(&msg.payload);
    buf.extend_from_slice(&encoded_proof);

    Ok(buf)
}

/// Decodes an L4 wire envelope from canonical big-endian bytes.
pub fn decode_envelope(bytes: &[u8]) -> Result<(CrossChainMessage, BridgeStatus), &'static str> {
    if bytes.len() < L4_HEADER_BYTES {
        return Err("Envelope buffer is smaller than L4_HEADER_BYTES (168 bytes)");
    }

    if bytes[0..4] != L4_WIRE_MAGIC {
        return Err("Invalid L4 envelope magic (expected 'AUL4')");
    }

    if bytes[4] != L4_CODEC_VERSION {
        return Err("Unsupported L4 codec version");
    }

    let protocol = ProtocolId::from_u8(bytes[5]);
    let hop_count = bytes[6];
    let status = BridgeStatus::from_u8(bytes[7]);

    let source_chain = ChainId::from_u64(u64::from_be_bytes(bytes[8..16].try_into().unwrap()));
    let destination_chain =
        ChainId::from_u64(u64::from_be_bytes(bytes[16..24].try_into().unwrap()));
    let sequence_nonce = u64::from_be_bytes(bytes[24..32].try_into().unwrap());
    let timeout_timestamp = u64::from_be_bytes(bytes[32..40].try_into().unwrap());

    let gas_limit = u64::from_be_bytes(bytes[40..48].try_into().unwrap());
    let max_fee_raw = u128::from_be_bytes(bytes[48..64].try_into().unwrap());
    let max_fee = Quantum::new(max_fee_raw);

    let sender: [u8; 32] = bytes[64..96].try_into().unwrap();
    let target_contract: [u8; 32] = bytes[96..128].try_into().unwrap();
    let packet_id: [u8; 32] = bytes[128..160].try_into().unwrap();

    let payload_len = u32::from_be_bytes(bytes[160..164].try_into().unwrap()) as usize;
    let proof_len = u32::from_be_bytes(bytes[164..168].try_into().unwrap()) as usize;

    if payload_len > MAX_L4_PAYLOAD_BYTES {
        return Err("Payload length in header exceeds MAX_L4_PAYLOAD_BYTES");
    }

    let expected_total = L4_HEADER_BYTES
        .checked_add(payload_len)
        .and_then(|t| t.checked_add(proof_len))
        .ok_or("Arithmetic overflow in envelope length")?;

    if bytes.len() != expected_total {
        return Err("Envelope length does not match payload + proof body length");
    }

    let payload = bytes[L4_HEADER_BYTES..L4_HEADER_BYTES + payload_len].to_vec();
    let proof_bytes = &bytes[L4_HEADER_BYTES + payload_len..];
    let proof = decode_proof_payload(proof_bytes)?;

    let route = RouteDescriptor {
        source_chain,
        destination_chain,
        protocol,
        hop_count,
        gas_limit,
        max_fee,
    };

    let msg = CrossChainMessage {
        packet_id,
        source_chain,
        destination_chain,
        sequence_nonce,
        sender,
        target_contract,
        payload,
        timeout_timestamp,
        route,
        proof,
    };

    if !msg.verify_integrity() {
        return Err("Message packet_id does not match computed Blake3 digest");
    }

    Ok((msg, status))
}

fn encode_proof_payload(proof: &ProofPayload) -> Vec<u8> {
    match proof {
        ProofPayload::None => vec![0x00],
        ProofPayload::MerkleInclusion(hashes) => {
            let mut out = Vec::with_capacity(1 + 4 + hashes.len() * 32);
            out.push(0x01);
            out.extend_from_slice(&(hashes.len() as u32).to_be_bytes());
            for h in hashes {
                out.extend_from_slice(h);
            }
            out
        }
        ProofPayload::ZkSnark(zk) => {
            let mut out = Vec::with_capacity(1 + 4 + zk.len());
            out.push(0x02);
            out.extend_from_slice(&(zk.len() as u32).to_be_bytes());
            out.extend_from_slice(zk);
            out
        }
        ProofPayload::ThresholdSignature(sig) => {
            let mut out = Vec::with_capacity(1 + 4 + sig.len());
            out.push(0x03);
            out.extend_from_slice(&(sig.len() as u32).to_be_bytes());
            out.extend_from_slice(sig);
            out
        }
        ProofPayload::MultiProverAttestation {
            light_client_verified,
            zk_proof,
            signatures,
        } => {
            let mut out =
                Vec::with_capacity(1 + 1 + 4 + zk_proof.len() + 4 + signatures.len() * 64);
            out.push(0x04);
            out.push(if *light_client_verified { 1 } else { 0 });
            out.extend_from_slice(&(zk_proof.len() as u32).to_be_bytes());
            out.extend_from_slice(zk_proof);
            out.extend_from_slice(&(signatures.len() as u32).to_be_bytes());
            for sig in signatures {
                out.extend_from_slice(sig);
            }
            out
        }
    }
}

fn decode_proof_payload(bytes: &[u8]) -> Result<ProofPayload, &'static str> {
    if bytes.is_empty() {
        return Ok(ProofPayload::None);
    }

    match bytes[0] {
        0x00 => Ok(ProofPayload::None),
        0x01 => {
            if bytes.len() < 5 {
                return Err("Malformed Merkle proof length");
            }
            let count = u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize;
            let expected_len = 5 + count * 32;
            if bytes.len() != expected_len {
                return Err("Merkle proof length mismatch");
            }
            let mut hashes = Vec::with_capacity(count);
            for i in 0..count {
                let start = 5 + i * 32;
                let h: [u8; 32] = bytes[start..start + 32].try_into().unwrap();
                hashes.push(h);
            }
            Ok(ProofPayload::MerkleInclusion(hashes))
        }
        0x02 => {
            if bytes.len() < 5 {
                return Err("Malformed ZK proof length");
            }
            let len = u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize;
            if bytes.len() != 5 + len {
                return Err("ZK proof byte length mismatch");
            }
            Ok(ProofPayload::ZkSnark(bytes[5..5 + len].to_vec()))
        }
        0x03 => {
            if bytes.len() < 5 {
                return Err("Malformed TSS proof length");
            }
            let len = u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize;
            if bytes.len() != 5 + len {
                return Err("TSS proof byte length mismatch");
            }
            Ok(ProofPayload::ThresholdSignature(bytes[5..5 + len].to_vec()))
        }
        0x04 => {
            if bytes.len() < 6 {
                return Err("Malformed MultiProver payload header");
            }
            let light_client_verified = bytes[1] == 1;
            let zk_len = u32::from_be_bytes(bytes[2..6].try_into().unwrap()) as usize;
            if bytes.len() < 6 + zk_len + 4 {
                return Err("Malformed MultiProver ZK length");
            }
            let zk_proof = bytes[6..6 + zk_len].to_vec();
            let sig_offset = 6 + zk_len;
            let sig_count =
                u32::from_be_bytes(bytes[sig_offset..sig_offset + 4].try_into().unwrap()) as usize;
            let expected = sig_offset + 4 + sig_count * 64;
            if bytes.len() != expected {
                return Err("MultiProver signatures length mismatch");
            }
            let mut signatures = Vec::with_capacity(sig_count);
            for i in 0..sig_count {
                let s_start = sig_offset + 4 + i * 64;
                let sig: [u8; 64] = bytes[s_start..s_start + 64].try_into().unwrap();
                signatures.push(sig);
            }
            Ok(ProofPayload::MultiProverAttestation {
                light_client_verified,
                zk_proof,
                signatures,
            })
        }
        _ => Err("Unknown proof payload discriminator"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interop::types::CrossChainMessageParams;

    #[test]
    fn test_codec_roundtrip_bitcoin_spv_envelope() {
        let params = CrossChainMessageParams {
            source_chain: ChainId::Bitcoin,
            destination_chain: ChainId::AurionL1,
            sequence_nonce: 777,
            sender: [0x11; 32],
            target_contract: [0x22; 32],
            payload: vec![0xCA, 0xFE, 0xBA, 0xBE],
            timeout_timestamp: 1_750_000_000,
            protocol: ProtocolId::SpvBitcoin,
            gas_limit: 21_000,
            max_fee: Quantum::new(50_000),
            proof: ProofPayload::MerkleInclusion(vec![[0xAA; 32], [0xBB; 32]]),
        };
        let msg = CrossChainMessage::new(params).expect("Message should be valid");

        let encoded = encode_envelope(&msg, BridgeStatus::Active).expect("Encoding should succeed");
        assert_eq!(&encoded[0..4], &L4_WIRE_MAGIC);

        let (decoded, status) = decode_envelope(&encoded).expect("Decoding should succeed");
        assert_eq!(decoded.packet_id, msg.packet_id);
        assert_eq!(decoded.source_chain, ChainId::Bitcoin);
        assert_eq!(decoded.destination_chain, ChainId::AurionL1);
        assert_eq!(decoded.sequence_nonce, 777);
        assert_eq!(decoded.payload, vec![0xCA, 0xFE, 0xBA, 0xBE]);
        assert_eq!(status, BridgeStatus::Active);

        if let ProofPayload::MerkleInclusion(hashes) = decoded.proof {
            assert_eq!(hashes.len(), 2);
            assert_eq!(hashes[0], [0xAA; 32]);
            assert_eq!(hashes[1], [0xBB; 32]);
        } else {
            panic!("Expected MerkleInclusion proof payload");
        }
    }

    #[test]
    fn test_codec_roundtrip_evm_envelope_with_zk() {
        let params = CrossChainMessageParams {
            source_chain: ChainId::AurionL2,
            destination_chain: ChainId::Ethereum,
            sequence_nonce: 888,
            sender: [0x33; 32],
            target_contract: [0x44; 32],
            payload: b"EVM call payload".to_vec(),
            timeout_timestamp: 1_760_000_000,
            protocol: ProtocolId::EvmSyncCommittee,
            gas_limit: 100_000,
            max_fee: Quantum::new(200_000_000),
            proof: ProofPayload::ZkSnark(vec![0x01, 0x02, 0x03, 0x04, 0x05]),
        };
        let msg = CrossChainMessage::new(params).expect("Message should be valid");

        let encoded =
            encode_envelope(&msg, BridgeStatus::Rebalancing).expect("Encoding should succeed");
        let (decoded, status) = decode_envelope(&encoded).expect("Decoding should succeed");
        assert_eq!(decoded.packet_id, msg.packet_id);
        assert_eq!(status, BridgeStatus::Rebalancing);
        assert_eq!(decoded.route.max_fee, Quantum::new(200_000_000));
    }

    #[test]
    fn test_codec_roundtrip_cosmos_ibc_envelope() {
        let params = CrossChainMessageParams {
            source_chain: ChainId::CosmosIbc,
            destination_chain: ChainId::AurionL1,
            sequence_nonce: 999,
            sender: [0x55; 32],
            target_contract: [0x66; 32],
            payload: b"IBC ICS-20 transfer".to_vec(),
            timeout_timestamp: 1_770_000_000,
            protocol: ProtocolId::CosmosIbc,
            gas_limit: 50_000,
            max_fee: Quantum::new(100_000),
            proof: ProofPayload::ThresholdSignature(vec![0xFE; 64]),
        };
        let msg = CrossChainMessage::new(params).expect("Message should be valid");

        let encoded = encode_envelope(&msg, BridgeStatus::Active).expect("Encoding should succeed");
        let (decoded, status) = decode_envelope(&encoded).expect("Decoding should succeed");
        assert_eq!(decoded.packet_id, msg.packet_id);
        assert_eq!(status, BridgeStatus::Active);
    }

    #[test]
    fn test_codec_rejects_corrupted_payload() {
        let params = CrossChainMessageParams {
            source_chain: ChainId::AurionL1,
            destination_chain: ChainId::Bitcoin,
            sequence_nonce: 1,
            sender: [0; 32],
            target_contract: [0; 32],
            payload: vec![1, 2, 3],
            timeout_timestamp: 1_800_000_000,
            protocol: ProtocolId::SpvBitcoin,
            gas_limit: 1000,
            max_fee: Quantum::new(10),
            proof: ProofPayload::None,
        };
        let msg = CrossChainMessage::new(params).unwrap();

        let mut encoded = encode_envelope(&msg, BridgeStatus::Active).unwrap();
        // Corrupt payload byte
        let payload_idx = L4_HEADER_BYTES;
        encoded[payload_idx] ^= 0xFF;

        let err = decode_envelope(&encoded);
        assert!(
            err.is_err(),
            "Integrity check must fail when payload is altered"
        );
    }
}
