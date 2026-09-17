#![forbid(unsafe_code)]

//! VER-004: Fuzz Robustness & Zero-Panic Testing Suite untuk Aurion Protocol
//! Melakukan ratusan ribu mutasi data ekstrem (bit-flips, truncations, boundary corruptions,
//! dan arbitrary byte stream injection) untuk membuktikan garansi ZERO PANIC pada:
//! 1. P2P Wire Frame Parser (`parse_network_frame`)
//! 2. Layer-2 Batch Calldata Frame Decoder (`L2BatchFrame::decode`, `L2BatchFrameHeader::decode`)
//! 3. Layer-4 Cross-Chain Wire Envelope Decoder (`decode_envelope`)
//! 4. Aurion Virtual Machine (AVM) Bytecode Verifier & Engine (`BytecodeVerifier`, `AvmEngine`)
//! 5. Stateless Transaction Validator (`validate_transaction_stateless`)

use std::collections::HashMap;

use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::crypto::blake3_hash;
use aurion::interop::codec::{decode_envelope, encode_envelope};
use aurion::interop::types::{
    BridgeStatus, ChainId, CrossChainMessage, CrossChainMessageParams, ProofPayload, ProtocolId,
};
use aurion::scaling::codec::{L2BatchFrame, L2BatchFrameHeader};
use aurion::transaction::types::{Transaction, TxType};
use aurion::transaction::validator::validate_transaction_stateless;
use aurion::vm::context::ExecutionContext;
use aurion::vm::engine::AvmEngine;
use aurion::vm::verifier::BytecodeVerifier;
use aurion::wire::frame::{parse_network_frame, serialize_network_frame};

// ==============================================================================
// 1. DETERMINISTIC MUTATION ENGINE (Zero Float, Blake3 PRNG)
// ==============================================================================

struct FuzzMutationEngine {
    state: [u8; 32],
    counter: u64,
}

impl FuzzMutationEngine {
    fn new(seed: &[u8]) -> Self {
        let digest = blake3_hash(seed);
        Self {
            state: digest.0,
            counter: 0,
        }
    }

    fn next_bytes(&mut self) -> [u8; 32] {
        self.counter += 1;
        let mut input = Vec::with_capacity(40);
        input.extend_from_slice(&self.state);
        input.extend_from_slice(&self.counter.to_be_bytes());
        let digest = blake3_hash(&input);
        self.state = digest.0;
        digest.0
    }

    fn next_u64(&mut self) -> u64 {
        let bytes = self.next_bytes();
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&bytes[..8]);
        u64::from_be_bytes(buf)
    }

    fn next_u32(&mut self) -> u32 {
        let bytes = self.next_bytes();
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&bytes[..4]);
        u32::from_be_bytes(buf)
    }

    fn next_u8(&mut self) -> u8 {
        self.next_bytes()[0]
    }

    fn next_range(&mut self, min: u64, max: u64) -> u64 {
        if min >= max {
            return min;
        }
        min + (self.next_u64() % (max - min + 1))
    }

    fn random_bytes(&mut self, len: usize) -> Vec<u8> {
        let mut result = Vec::with_capacity(len);
        while result.len() < len {
            let chunk = self.next_bytes();
            let needed = len - result.len();
            let take = needed.min(32);
            result.extend_from_slice(&chunk[..take]);
        }
        result
    }

    /// Terapkan strategi mutasi acak pada buffer byte valid
    fn mutate(&mut self, data: &mut Vec<u8>) {
        if data.is_empty() {
            data.push(self.next_u8());
            return;
        }

        match self.next_range(0, 4) {
            // Strategi 0: Bit-flip 1-8 bit acak
            0 => {
                let flips = self.next_range(1, 8);
                for _ in 0..flips {
                    let byte_idx = self.next_range(0, data.len() as u64 - 1) as usize;
                    let bit_idx = self.next_range(0, 7) as u8;
                    data[byte_idx] ^= 1 << bit_idx;
                }
            }
            // Strategi 1: Pemangkasan (truncation)
            1 => {
                let cut = self.next_range(0, data.len() as u64 - 1) as usize;
                data.truncate(cut);
            }
            // Strategi 2: Penyisipan byte sampah (splicing)
            2 => {
                let insert_len = self.next_range(1, 32) as usize;
                let insert_pos = self.next_range(0, data.len() as u64) as usize;
                let junk = self.random_bytes(insert_len);
                data.splice(insert_pos..insert_pos, junk);
            }
            // Strategi 3: Penimpaan nilai batas ekstrem (0x00, 0xFF, 0x7F, 0x80)
            3 => {
                let pos = self.next_range(0, data.len() as u64 - 1) as usize;
                let extreme_val = match self.next_range(0, 3) {
                    0 => 0x00,
                    1 => 0xFF,
                    2 => 0x7F,
                    _ => 0x80,
                };
                data[pos] = extreme_val;
            }
            // Strategi 4: Timpa slice dengan data acak
            _ => {
                let pos = self.next_range(0, data.len() as u64 - 1) as usize;
                let max_len = (data.len() - pos).min(16);
                let overwrite_len = self.next_range(1, max_len as u64) as usize;
                let junk = self.random_bytes(overwrite_len);
                data[pos..pos + overwrite_len].copy_from_slice(&junk);
            }
        }
    }
}

// ==============================================================================
// 2. FUZZ TARGET 1: P2P WIRE FRAME PARSER ZERO-PANIC ROBUSTNESS
// ==============================================================================

#[test]
fn test_fuzz_wire_frame_parser_zero_panic() {
    let mut fuzzer = FuzzMutationEngine::new(b"AURION-FUZZ-WIRE-FRAME-V1");

    for iter in 0..10_000 {
        let fuzz_bytes = if iter % 2 == 0 {
            // Kasus A: Mutasi dari frame yang valid
            let p_len = fuzzer.next_range(0, 512) as usize;
            let valid_payload = fuzzer.random_bytes(p_len);
            let mut valid_frame = serialize_network_frame(1, &valid_payload).unwrap();
            fuzzer.mutate(&mut valid_frame);
            valid_frame
        } else {
            // Kasus B: Data byte acak murni (0 s/d 1024 bytes)
            let len = fuzzer.next_range(0, 1024) as usize;
            fuzzer.random_bytes(len)
        };

        // Asersi Invariant Kritis: parse_network_frame TIDAK PERNAH PANIC
        let parse_result = parse_network_frame(&fuzz_bytes);
        match parse_result {
            Ok((header, payload)) => {
                // Jika lolos, verifikasi bahwa checksum-nya memang benar cocok
                assert_eq!(blake3_hash(payload), header.payload_checksum);
            }
            Err(_) => {
                // Penolakan elegan via Result::Err (ekspektasi kanonikal)
            }
        }
    }
}

// ==============================================================================
// 3. FUZZ TARGET 2: L2 BATCH CALLDATA CODEC ZERO-PANIC ROBUSTNESS
// ==============================================================================

#[test]
fn test_fuzz_l2_batch_calldata_codec_zero_panic() {
    let mut fuzzer = FuzzMutationEngine::new(b"AURION-FUZZ-L2-BATCH-V1");

    for iter in 0..10_000 {
        let fuzz_bytes = if iter % 2 == 0 {
            // Kasus A: Mutasi dari batch frame yang valid
            let p_len = fuzzer.next_range(0, 256) as usize;
            let p_bytes = fuzzer.random_bytes(p_len);
            let frame = L2BatchFrame::new(
                1,
                Hash256::ZERO,
                Hash256::ZERO,
                10,
                20,
                5,
                0x00,
                p_bytes,
            );
            let mut encoded = frame.encode();
            fuzzer.mutate(&mut encoded);
            encoded
        } else {
            // Kasus B: Data byte acak murni
            let len = fuzzer.next_range(0, 512) as usize;
            fuzzer.random_bytes(len)
        };

        // Asersi Invariant Kritis: decode L2 TIDAK PERNAH PANIC
        let _ = L2BatchFrame::decode(&fuzz_bytes);
        let _ = L2BatchFrameHeader::decode(&fuzz_bytes);
    }
}

// ==============================================================================
// 4. FUZZ TARGET 3: L4 CROSS-CHAIN WIRE ENVELOPE ZERO-PANIC ROBUSTNESS
// ==============================================================================

#[test]
fn test_fuzz_l4_cross_chain_envelope_codec_zero_panic() {
    let mut fuzzer = FuzzMutationEngine::new(b"AURION-FUZZ-L4-ENVELOPE-V1");

    for iter in 0..10_000 {
        let fuzz_bytes = if iter % 2 == 0 {
            // Kasus A: Mutasi dari L4 envelope yang valid
            let p_len = fuzzer.next_range(0, 128) as usize;
            let p_bytes = fuzzer.random_bytes(p_len);
            let msg = CrossChainMessage::new(CrossChainMessageParams {
                source_chain: ChainId::Bitcoin,
                destination_chain: ChainId::AurionL1,
                sequence_nonce: 42,
                sender: [1u8; 32],
                target_contract: [2u8; 32],
                payload: p_bytes,
                timeout_timestamp: 1_800_000_000,
                protocol: ProtocolId::SpvBitcoin,
                gas_limit: 100_000,
                max_fee: Quantum::new(50_000),
                proof: ProofPayload::None,
            })
            .unwrap();

            let mut encoded = encode_envelope(&msg, BridgeStatus::Active).unwrap();
            fuzzer.mutate(&mut encoded);
            encoded
        } else {
            // Kasus B: Data byte acak murni
            let len = fuzzer.next_range(0, 512) as usize;
            fuzzer.random_bytes(len)
        };

        // Asersi Invariant Kritis: decode_envelope TIDAK PERNAH PANIC
        let _ = decode_envelope(&fuzz_bytes);
    }
}

// ==============================================================================
// 5. FUZZ TARGET 4: AVM BYTECODE VERIFIER & ENGINE ZERO-PANIC ROBUSTNESS
// ==============================================================================

#[test]
fn test_fuzz_avm_bytecode_verifier_and_engine_zero_panic() {
    let mut fuzzer = FuzzMutationEngine::new(b"AURION-FUZZ-AVM-ENGINE-V1");

    for _ in 0..10_000 {
        // Hasilkan bytecode acak arbitrer (panjang 0 s/d 1024 bytes)
        let bytecode_len = fuzzer.next_range(0, 1024) as usize;
        let raw_bytecode = fuzzer.random_bytes(bytecode_len);

        // 1. Verifikasi bytecode: TIDAK PERNAH PANIC
        let verify_result = BytecodeVerifier::verify(&raw_bytecode);

        // 2. Jika secara kebetulan lolos verifikasi statis, eksekusi di AVM
        if let Ok(verified_contract) = verify_result {
            let ctx = ExecutionContext::new(
                Address::ZERO,
                Address::ZERO,
                Address::ZERO,
                Quantum::ZERO,
                50_000, // Alokasi gas terbatas
                100,
                1773532800,
            );
            let initial_storage = HashMap::new();

            // Eksekusi engine: TIDAK PERNAH PANIC, HANG, ATAU INFINITE LOOP
            let _ = AvmEngine::execute(&verified_contract, ctx, &initial_storage);
        }
    }
}

// ==============================================================================
// 6. FUZZ TARGET 5: STATELESS TRANSACTION VALIDATOR ZERO-PANIC ROBUSTNESS
// ==============================================================================

#[test]
fn test_fuzz_stateless_transaction_validator_zero_panic() {
    let mut fuzzer = FuzzMutationEngine::new(b"AURION-FUZZ-TX-VALIDATOR-V1");

    for _ in 0..10_000 {
        let sender_pubkey = fuzzer.next_bytes();
        let payload_len = fuzzer.next_range(0, 512) as usize;

        let tx = Transaction {
            version: fuzzer.next_range(0, 5) as u16,
            chain_id: fuzzer.next_u32(),
            tx_type: match fuzzer.next_range(0, 2) {
                0 => TxType::Transfer,
                1 => TxType::ContractDeploy,
                _ => TxType::ContractCall,
            },
            flags: fuzzer.next_u8(),
            sender: Address(fuzzer.next_bytes()),
            recipient: Address(fuzzer.next_bytes()),
            nonce: fuzzer.next_u64(),
            amount: Quantum::new(fuzzer.next_u64() as u128),
            fee: Quantum::new(fuzzer.next_u64() as u128),
            valid_until: fuzzer.next_u64(),
            payload: fuzzer.random_bytes(payload_len),
            signature: Signature::from_bytes(fuzzer.random_bytes(64).try_into().unwrap()),
        };

        // Asersi Invariant Kritis: validate_transaction_stateless TIDAK PERNAH PANIC
        let _ = validate_transaction_stateless(&tx, &sender_pubkey);
    }
}
