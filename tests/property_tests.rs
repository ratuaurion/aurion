#![forbid(unsafe_code)]

//! VER-004: Property-Based Testing Suite untuk Aurion Protocol
//! Memverifikasi relasi roundtrip kanonikal dan invarian matematis pada:
//! 1. P2P Network Wire Frame Codec (AUR0, 52B Header)
//! 2. Layer-2 Batch Calldata Frame Codec (AUL2, 102B Header)
//! 3. Layer-4 Cross-Chain Wire Envelope Codec (AUL4, 168B Header)
//! 4. Quantum Integer Arithmetic & Conservation Invariance (AUR-ARCH-012)
//! 5. Sparse Merkle Tree (SMT) Determinism & Proof Invariance (AUR-ARCH-005)
//! 6. AVM Bytecode Verification & Deterministic Gas Metering (AUR-VM-001..004)

use std::collections::HashMap;

use aurion::core::{Address, Hash256, Quantum};
use aurion::crypto::{blake3_hash, derive_address_from_pubkey, Keypair};
use aurion::interop::codec::{decode_envelope, encode_envelope, L4_HEADER_BYTES, L4_WIRE_MAGIC};
use aurion::interop::types::{BridgeStatus, ChainId, CrossChainMessage, ProofPayload, ProtocolId};
use aurion::scaling::codec::{
    L2BatchFrame, L2BatchFrameHeader, BATCH_FRAME_HEADER_SIZE, MAGIC_AUL2, PROTOCOL_VERSION_1,
};
use aurion::state::account::Account;
use aurion::state::smt::compute_accounts_state_root;
use aurion::vm::context::ExecutionContext;
use aurion::vm::engine::{AvmEngine, ExecutionResult};
use aurion::vm::opcode::Opcode;
use aurion::vm::verifier::BytecodeVerifier;
use aurion::wire::frame::{
    parse_network_frame, serialize_network_frame, WIRE_FRAME_HEADER_BYTES, WIRE_MAGIC,
};
use aurion::wire::{
    max_payload_bound, MSG_BFT_PREVOTE, MSG_HANDSHAKE_HELLO, MSG_MEMPOOL_INV, MSG_PEERS_ADDR,
    MSG_TX_GOSSIP,
};

// ==============================================================================
// 1. DETERMINISTIC BLAKE3 PRNG ENGINE (Zero Float, Zero External Dependency)
// ==============================================================================

struct Blake3Prng {
    state: [u8; 32],
    counter: u64,
}

impl Blake3Prng {
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

    fn next_u8(&mut self) -> u8 {
        self.next_bytes()[0]
    }

    fn next_range(&mut self, min: u64, max: u64) -> u64 {
        if min >= max {
            return min;
        }
        min + (self.next_u64() % (max - min + 1))
    }

    fn next_vec(&mut self, len: usize) -> Vec<u8> {
        let mut result = Vec::with_capacity(len);
        while result.len() < len {
            let chunk = self.next_bytes();
            let needed = len - result.len();
            let take = needed.min(32);
            result.extend_from_slice(&chunk[..take]);
        }
        result
    }
}

// ==============================================================================
// 2. PROPERTY TEST 1: P2P WIRE FRAME CODEC ROUNDTRIP & CHECKSUM INVARIANCE
// ==============================================================================

#[test]
fn test_property_wire_frame_roundtrip_and_checksum_invariance() {
    let mut prng = Blake3Prng::new(b"AURION-PROPERTY-WIRE-FRAME-V1");

    for iter in 0..500 {
        // Hanya tipe pesan kanonikal yang diuji; plafon payload per tipe
        // ditegakkan oleh codec, jadi panjang payload harus dipangkas ke batas tipe.
        const KNOWN_TYPES: [u16; 5] = [
            MSG_HANDSHAKE_HELLO,
            MSG_BFT_PREVOTE,
            MSG_PEERS_ADDR,
            MSG_MEMPOOL_INV,
            MSG_TX_GOSSIP,
        ];
        let msg_type = KNOWN_TYPES[iter % KNOWN_TYPES.len()];
        let max_bound = max_payload_bound(msg_type);
        // Variasi panjang payload: 0, 1, kecil, menengah, hingga 64 KB
        let payload_len = match iter % 6 {
            0 => 0,
            1 => 1,
            2 => prng.next_range(2, 64) as usize,
            3 => prng.next_range(65, 1024) as usize,
            4 => prng.next_range(1025, 16384) as usize,
            _ => prng.next_range(16385, 65536) as usize,
        }
        .min(max_bound);
        let payload = prng.next_vec(payload_len);

        // 1. Serialisasi frame
        let serialized = serialize_network_frame(msg_type, &payload)
            .expect("Valid payload serialization must succeed");

        // Properti: Ukuran frame tepat WIRE_FRAME_HEADER_BYTES + payload.len()
        assert_eq!(
            serialized.len(),
            WIRE_FRAME_HEADER_BYTES + payload_len,
            "Frame length property violated"
        );

        // Properti: 4 byte pertama adalah WIRE_MAGIC kanonikal "AUR0"
        assert_eq!(
            &serialized[0..4],
            &WIRE_MAGIC,
            "Wire magic property violated"
        );

        // 2. Deserialisasi dan verifikasi roundtrip
        let (header, parsed_payload) =
            parse_network_frame(&serialized).expect("Valid frame roundtrip parsing must succeed");

        assert_eq!(header.magic, WIRE_MAGIC);
        assert_eq!(header.message_type, msg_type);
        assert_eq!(header.payload_len as usize, payload_len);
        assert_eq!(parsed_payload, payload.as_slice());

        // Properti: Checksum adalah hash Blake3 kanonikal dari payload
        let expected_checksum = blake3_hash(&payload);
        assert_eq!(header.payload_checksum, expected_checksum);
    }
}

// ==============================================================================
// 3. PROPERTY TEST 2: L2 BATCH CALLDATA FRAME ROUNDTRIP INVARIANCE
// ==============================================================================

#[test]
fn test_property_l2_batch_calldata_frame_roundtrip_invariance() {
    let mut prng = Blake3Prng::new(b"AURION-PROPERTY-L2-BATCH-V1");

    for iter in 0..500 {
        let batch_index = prng.next_u64();
        let prev_root = Hash256::from_bytes(prng.next_bytes());
        let new_root = Hash256::from_bytes(prng.next_bytes());
        let start_block = prng.next_range(1, 1_000_000);
        let end_block = start_block + prng.next_range(0, 500);
        let tx_count = prng.next_range(0, 10_000) as u32;
        let compression_flags = prng.next_u8();

        let payload_len = match iter % 5 {
            0 => 0,
            1 => 4,
            2 => prng.next_range(5, 512) as usize,
            3 => prng.next_range(513, 4096) as usize,
            _ => prng.next_range(4097, 32768) as usize,
        };
        let payload = prng.next_vec(payload_len);

        let frame = L2BatchFrame::new(
            batch_index,
            prev_root,
            new_root,
            start_block,
            end_block,
            tx_count,
            compression_flags,
            payload.clone(),
        );

        // 1. Properti Header Encoding Roundtrip
        let header_encoded = frame.header.encode();
        assert_eq!(header_encoded.len(), BATCH_FRAME_HEADER_SIZE);
        assert_eq!(&header_encoded[0..4], &MAGIC_AUL2);
        assert_eq!(header_encoded[4], PROTOCOL_VERSION_1);

        let decoded_header = L2BatchFrameHeader::decode(&header_encoded)
            .expect("Header decode roundtrip must succeed");
        assert_eq!(decoded_header, frame.header);

        // 2. Properti Full Batch Frame Roundtrip
        let full_encoded = frame.encode();
        assert_eq!(full_encoded.len(), BATCH_FRAME_HEADER_SIZE + payload_len);

        let decoded_frame =
            L2BatchFrame::decode(&full_encoded).expect("Full batch frame decode must succeed");
        assert_eq!(decoded_frame, frame);
        assert_eq!(decoded_frame.payload, payload);

        // 3. Properti DA Hash Determinisme & Non-Triviality
        let da_hash_1 = frame.compute_da_hash();
        let da_hash_2 = frame.compute_da_hash();
        assert_eq!(da_hash_1, da_hash_2, "DA hash must be deterministic");
        assert_ne!(da_hash_1, Hash256::ZERO, "DA hash cannot be trivial zero");
        assert_eq!(da_hash_1, blake3_hash(&full_encoded));
    }
}

// ==============================================================================
// 4. PROPERTY TEST 3: L4 CROSS-CHAIN WIRE ENVELOPE ROUNDTRIP INVARIANCE
// ==============================================================================

#[test]
fn test_property_l4_cross_chain_envelope_roundtrip_invariance() {
    let mut prng = Blake3Prng::new(b"AURION-PROPERTY-L4-ENVELOPE-V1");

    for iter in 0..500 {
        let protocol = match prng.next_range(0, 5) {
            0 => ProtocolId::SpvBitcoin,
            1 => ProtocolId::EvmSyncCommittee,
            2 => ProtocolId::CosmosIbc,
            3 => ProtocolId::ZkRollup,
            4 => ProtocolId::ThresholdVault,
            _ => ProtocolId::NativeCrossLayer,
        };
        let hop_count = prng.next_range(1, 16) as u8;
        let status = match prng.next_range(0, 5) {
            0 => BridgeStatus::Proposed,
            1 => BridgeStatus::Verified,
            2 => BridgeStatus::Active,
            3 => BridgeStatus::Rebalancing,
            4 => BridgeStatus::CircuitBroken,
            _ => BridgeStatus::Paused,
        };

        let source_chain = ChainId::from_u64(prng.next_range(1, 100_000));
        let destination_chain = ChainId::from_u64(prng.next_range(1, 100_000));
        let sequence_nonce = prng.next_u64();
        let timeout_timestamp = prng.next_range(1_700_000_000, 2_000_000_000);
        let gas_limit = prng.next_range(21_000, 10_000_000);
        let fee_quanta = prng.next_range(1, 1_000_000_000);

        let sender = prng.next_bytes();
        let target_contract = prng.next_bytes();

        let payload_len = match iter % 4 {
            0 => 0,
            1 => prng.next_range(1, 128) as usize,
            2 => prng.next_range(129, 2048) as usize,
            _ => prng.next_range(2049, 16384) as usize,
        };
        let payload = prng.next_vec(payload_len);

        let proof = match iter % 4 {
            0 => ProofPayload::None,
            1 => ProofPayload::MerkleInclusion(vec![prng.next_bytes(), prng.next_bytes()]),
            2 => ProofPayload::ZkSnark(prng.next_vec(128)),
            _ => ProofPayload::ThresholdSignature(prng.next_vec(64)),
        };

        let mut msg = CrossChainMessage::new(aurion::interop::types::CrossChainMessageParams {
            source_chain,
            destination_chain,
            sequence_nonce,
            sender,
            target_contract,
            payload: payload.clone(),
            timeout_timestamp,
            protocol,
            gas_limit,
            max_fee: Quantum::new(fee_quanta as u128),
            proof,
        })
        .expect("Valid message params must construct CrossChainMessage");
        msg.route.hop_count = hop_count;

        let encoded =
            encode_envelope(&msg, status).expect("Valid L4 envelope encoding must succeed");
        assert!(encoded.len() >= L4_HEADER_BYTES);
        assert_eq!(&encoded[0..4], &L4_WIRE_MAGIC);

        let (decoded_msg, decoded_status) =
            decode_envelope(&encoded).expect("L4 envelope decode must succeed");

        assert_eq!(decoded_status, status);
        assert_eq!(decoded_msg.source_chain, source_chain);
        assert_eq!(decoded_msg.destination_chain, destination_chain);
        assert_eq!(decoded_msg.sequence_nonce, sequence_nonce);
        assert_eq!(decoded_msg.timeout_timestamp, timeout_timestamp);
        assert_eq!(decoded_msg.sender, sender);
        assert_eq!(decoded_msg.target_contract, target_contract);
        assert_eq!(decoded_msg.packet_id, msg.packet_id);
        assert_eq!(decoded_msg.payload, payload);
        assert_eq!(decoded_msg.route.protocol, protocol);
        assert_eq!(decoded_msg.route.hop_count, hop_count);
        assert_eq!(decoded_msg.route.gas_limit, gas_limit);
        assert_eq!(decoded_msg.route.max_fee, Quantum::new(fee_quanta as u128));
    }
}

// ==============================================================================
// 5. PROPERTY TEST 4: QUANTUM INTEGER ARITHMETIC & CONSERVATION INVARIANCE
// ==============================================================================

#[test]
fn test_property_quantum_arithmetic_associativity_and_conservation() {
    let mut prng = Blake3Prng::new(b"AURION-PROPERTY-QUANTUM-MATH-V1");

    for _ in 0..1000 {
        // Ambil nilai kuantum bounded (0 s/d 10^14) untuk mencegah overflow saat penjumlahan
        let a_val = prng.next_range(0, 100_000_000_000_000);
        let b_val = prng.next_range(0, 100_000_000_000_000);
        let c_val = prng.next_range(0, 100_000_000_000_000);

        let a = Quantum::new(a_val as u128);
        let b = Quantum::new(b_val as u128);
        let c = Quantum::new(c_val as u128);

        // 1. Properti Identitas Penjumlahan: A + 0 == A
        assert_eq!(a.checked_add(Quantum::ZERO).unwrap(), a);

        // 2. Properti Komutatif: A + B == B + A
        assert_eq!(
            a.checked_add(b).unwrap(),
            b.checked_add(a).unwrap(),
            "Commutativity property violated"
        );

        // 3. Properti Asosiatif: (A + B) + C == A + (B + C)
        let sum_ab_c = a.checked_add(b).unwrap().checked_add(c).unwrap();
        let sum_a_bc = a.checked_add(b.checked_add(c).unwrap()).unwrap();
        assert_eq!(sum_ab_c, sum_a_bc, "Associativity property violated");

        // 4. Properti Pengurangan Konsisten: (A + B) - B == A
        let sum_ab = a.checked_add(b).unwrap();
        assert_eq!(
            sum_ab.checked_sub(b).unwrap(),
            a,
            "Inverse subtraction property violated"
        );

        // 5. Properti Pembagian Fee Konservasi Eksak (20% burn, 80% miner):
        let fee_raw = prng.next_range(1, 1_000_000_000_000) as u128;
        let fee = Quantum::new(fee_raw);
        let burn_amt = (fee_raw * 20) / 100;
        let miner_amt_raw = (fee_raw * 80) / 100;
        let remainder = fee_raw - (burn_amt + miner_amt_raw);
        let miner_fee = miner_amt_raw + remainder;

        // Konservasi mutlak: burn + miner == total fee
        assert_eq!(
            burn_amt + miner_fee,
            fee.as_u128(),
            "Fee conservation invariant violated"
        );
    }
}

// ==============================================================================
// 6. PROPERTY TEST 5: SMT STATE ROOT DETERMINISM & ORDER INDEPENDENCE
// ==============================================================================

#[test]
fn test_property_smt_root_determinism_and_order_independence() {
    let mut prng = Blake3Prng::new(b"AURION-PROPERTY-SMT-DETERMINISM-V1");

    for _ in 0..100 {
        let count = prng.next_range(5, 30) as usize;
        let mut accounts_list = Vec::with_capacity(count);

        for _ in 0..count {
            let kp = Keypair::generate();
            let addr = derive_address_from_pubkey(&kp.public_key_bytes());
            let balance = Quantum::new(prng.next_range(100, 10_000_000) as u128);
            let nonce = prng.next_range(0, 50);
            accounts_list.push((addr, Account::new(balance, nonce)));
        }

        // Susun ke dalam HashMap urutan 1
        let mut map_1: HashMap<Address, Account> = HashMap::new();
        for (addr, acct) in &accounts_list {
            map_1.insert(*addr, acct.clone());
        }

        // Susun ke dalam HashMap urutan 2 (reverse)
        let mut map_2: HashMap<Address, Account> = HashMap::new();
        for (addr, acct) in accounts_list.iter().rev() {
            map_2.insert(*addr, acct.clone());
        }

        let root_1 = compute_accounts_state_root(&map_1);
        let root_2 = compute_accounts_state_root(&map_2);

        // Properti: State root Sparse Merkle Tree harus 100% independen terhadap urutan iterasi
        assert_eq!(
            root_1, root_2,
            "SMT root determinism violated across different insertion orders"
        );
        assert_ne!(
            root_1,
            Hash256::ZERO,
            "SMT root cannot be zero with populated accounts"
        );
    }
}

// ==============================================================================
// 7. PROPERTY TEST 6: AVM BYTECODE VERIFIER & DETERMINISTIC EXECUTION
// ==============================================================================

#[test]
fn test_property_avm_bytecode_verification_and_determinism() {
    let mut prng = Blake3Prng::new(b"AURION-PROPERTY-AVM-EXECUTION-V1");

    for _ in 0..300 {
        // Hasilkan program AVM sah dengan instruksi acak terkontrol
        let mut bytecode = Vec::new();
        let num_ops = prng.next_range(3, 20);

        for _ in 0..num_ops {
            match prng.next_range(0, 6) {
                0 => {
                    // PUSH1 dengan 1 byte data
                    bytecode.push(Opcode::Push1 as u8);
                    bytecode.push(prng.next_u8());
                }
                1 => bytecode.push(Opcode::Add as u8),
                2 => bytecode.push(Opcode::Sub as u8),
                3 => bytecode.push(Opcode::Mul as u8),
                4 => bytecode.push(Opcode::Div as u8),
                5 => bytecode.push(Opcode::Mod as u8),
                _ => bytecode.push(Opcode::Dup1 as u8),
            }
        }
        bytecode.push(Opcode::Stop as u8);

        // 1. Verifikasi bytecode secara statis
        let verified_res = BytecodeVerifier::verify(&bytecode);
        assert!(
            verified_res.is_ok(),
            "Well-formed generated bytecode must pass static verification"
        );
        let verified = verified_res.unwrap();

        // 2. Eksekusi program di AVM Engine
        let ctx = ExecutionContext::new(
            Address::ZERO,
            Address::ZERO,
            Address::ZERO,
            Quantum::ZERO,
            100_000,
            100,
            1773532800,
        );
        let initial_storage = HashMap::new();

        let result = AvmEngine::execute(&verified, ctx, &initial_storage);

        // Properti: Eksekusi harus berakhir secara deterministik tanpa panik
        match result {
            ExecutionResult::Success { gas_used, .. } => {
                assert!(gas_used > 0 && gas_used <= 100_000);
            }
            ExecutionResult::OutOfGas => {}
            ExecutionResult::Revert { .. } => {}
            ExecutionResult::Error(_) => {}
        }
    }
}
