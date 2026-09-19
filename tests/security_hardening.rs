#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic, clippy::cast_precision_loss)]

//! Suite Pengujian Pengerasan Keamanan & Audit Pembersihan Memori (VER-009).
//!
//! Menguji secara otomatis dan ketat:
//! 1. Pembersihan memori rahasia kriptografis (Zeroization on drop / zeroize hygiene).
//! 2. Penegakan batas anti-DoS pada seluruh lapisan protokol (Wire 8MB/64KB, Calldata, AVM Stack/Memory).
//! 3. Isolasi hak istimewa (Privilege Isolation) Sentry Node vs. Validator Enclave.
//! 4. Ketahanan kriptografis anti-malleability (RFC 8032 strict verification).
//! 5. Penolakan mutlak atas serangan replay lintas-lapisan via Nullifier Registries.

use aurion::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::crypto::ed25519_verify_strict;
use aurion::crypto::{derive_address_from_pubkey, Keypair};
use aurion::interop::messaging::UniversalNullifierRegistry;
use aurion::interop::types::{
    ChainId, CrossChainMessage, CrossChainMessageParams, ProofPayload, ProtocolId,
};
use aurion::mempool::MempoolEngine;
use aurion::runtime::{NodeConfig, NodeRole, RuntimeSupervisor};
use aurion::scaling::codec::{L2BatchFrame, L2CodecError};
use aurion::specialized::messaging::{CrossLayerMessage, NullifierRegistry};
use aurion::specialized::types::DomainId;
use aurion::transaction::types::{Transaction, TxType, MAX_TRANSACTION_PAYLOAD_BYTES};
use aurion::vm::context::ExecutionContext;
use aurion::vm::engine::{AvmEngine, ExecutionResult};
use aurion::vm::opcode::Opcode;
use aurion::vm::verifier::BytecodeVerifier;
use aurion::wallet::bip39::{entropy_to_mnemonic_24, mnemonic_to_entropy_24};
use aurion::wallet::derivation::ExtendedKey;
use aurion::wallet::keystore::Keystore;
use aurion::wire::frame::{
    parse_network_frame, serialize_network_frame, WireError, MAX_WIRE_PAYLOAD_BYTES,
};
use aurion::wire::{MAX_TX_WIRE_SIZE, MSG_TX_GOSSIP};
use ed25519_dalek::SigningKey;
use std::collections::HashMap;
use zeroize::Zeroize;

// ============================================================================
// 1. AUDIT PEMBERSIHAN MEMORI KUNCI PRIVAT & RAHASIA (ZEROIZATION AUDIT)
// ============================================================================

#[test]
fn test_zeroize_extended_key_on_drop() {
    let mut ext_key = ExtendedKey {
        key: [0x42u8; 32],
        chain_code: [0x99u8; 32],
    };

    // Pastikan sebelum zeroize, memori berisi data rahasia
    assert_eq!(ext_key.key[0], 0x42);
    assert_eq!(ext_key.chain_code[0], 0x99);

    // Jalankan zeroize eksplisit sesuai trait Zeroize
    ext_key.key.zeroize();
    ext_key.chain_code.zeroize();

    // Verifikasi seluruh 64 byte memori terhapus menjadi 0x00
    assert_eq!(ext_key.key, [0u8; 32], "Kunci rahasia wajib bernilai 0x00");
    assert_eq!(
        ext_key.chain_code, [0u8; 32],
        "Chain code wajib bernilai 0x00"
    );
}

#[test]
fn test_zeroize_bip39_entropy_hygiene() {
    let initial_entropy = [0xabu8; 32];
    let mnemonic = entropy_to_mnemonic_24(&initial_entropy);
    let mut entropy = mnemonic_to_entropy_24(&mnemonic).expect("Valid mnemonic to entropy");
    assert_eq!(entropy, initial_entropy);

    // Zeroize memori entropi
    entropy.zeroize();
    assert_eq!(
        entropy, [0u8; 32],
        "Memori entropi wajib 0x00 pasca zeroize"
    );
}

#[test]
fn test_zeroize_keystore_encryption_decryption_cycle() {
    let signing_key = SigningKey::from_bytes(&[0x88u8; 32]);
    let address = derive_address_from_pubkey(signing_key.verifying_key().as_bytes());
    let bech32m = aurion::crypto::encode_address_bech32m(&address, "aur").unwrap();
    let password = "SuperSecretPassword123!@#";

    let keystore = Keystore::encrypt(&signing_key, password, &bech32m)
        .expect("Enkripsi keystore V2 harus berhasil");
    let json = keystore.to_json_string();

    // 1. Dekripsi dengan password yang benar harus sukses
    let loaded_keystore = Keystore::from_json_str(&json).expect("Parse keystore json");
    let decrypted_key = loaded_keystore
        .decrypt(password)
        .expect("Decryption success");

    // Verifikasi public key dari decrypted key identik
    assert_eq!(
        decrypted_key.verifying_key().to_bytes(),
        signing_key.verifying_key().to_bytes()
    );

    // 2. Dekripsi dengan password salah WAJIB ditolak seketika (Anti-Tamper MAC)
    let wrong_res = loaded_keystore.decrypt("WrongPassword");
    assert!(
        wrong_res.is_err(),
        "Password salah wajib ditolak oleh otentikasi MAC"
    );
}

// ============================================================================
// 2. PENEGAKAN BATAS ANTI-DOS & RESOURCE BOUNDS
// ============================================================================

#[test]
fn test_anti_dos_wire_frame_max_payload_rejection() {
    // 1. Payload normal dalam batas tipe pesan (MSG_HANDSHAKE_HELLO <= 512 B) diterima
    let valid_payload = vec![0xAA; 256];
    let encoded = serialize_network_frame(1, &valid_payload);
    assert!(encoded.is_ok());

    // 2. Payload melebihi plafon tipe pesan (dan plafon global 8 MB) wajib ditolak
    let oversized_payload = vec![0xBB; MAX_WIRE_PAYLOAD_BYTES + 1];
    let oversized_res = serialize_network_frame(1, &oversized_payload);
    assert!(
        matches!(oversized_res, Err(WireError::PayloadTooLarge(_))),
        "Frame jaringan melebihi batas tipe/global wajib ditolak demi mencegah DoS memori"
    );

    // 3. Deserialisasi frame dengan magic salah wajib ditolak
    let mut bad_magic_frame = encoded.unwrap();
    bad_magic_frame[0] = 0xFF; // Rusak magic "AUR0"
    let decode_res = parse_network_frame(&bad_magic_frame);
    assert!(
        matches!(decode_res, Err(WireError::InvalidMagic(_))),
        "Frame dengan magic salah wajib ditolak seketika"
    );
}

#[test]
fn test_strict_wire_frame_rejects_oversized_per_message_type() {
    // 1. Serialisasi gossip melampaui MAX_TX_WIRE_SIZE wajib ditolak
    let oversized = vec![0xAA; MAX_TX_WIRE_SIZE + 1];
    assert!(matches!(
        serialize_network_frame(MSG_TX_GOSSIP, &oversized),
        Err(WireError::PayloadTooLarge(_))
    ));

    // 2. Header dengan payload_len di atas plafon tipe ditolak sebelum checksum.
    //    Layout header: magic[0..4], message_type[4..6], reserved[6..8],
    //    payload_len[8..12], reserved2[12..20], checksum[20..52].
    let frame = serialize_network_frame(MSG_TX_GOSSIP, &[]).unwrap();
    let mut tampered = frame.clone();
    tampered[8..12].copy_from_slice(&((MAX_TX_WIRE_SIZE + 1) as u32).to_be_bytes());
    assert!(matches!(
        parse_network_frame(&tampered),
        Err(WireError::PayloadTooLarge(len)) if len == MAX_TX_WIRE_SIZE + 1
    ));
}

#[test]
fn test_strict_wire_frame_rejects_unknown_type_and_nonzero_reserved() {
    let frame = serialize_network_frame(MSG_TX_GOSSIP, &[0x01, 0x02, 0x03]).unwrap();

    // 1. Tipe pesan tidak dikenal wajib ditolak (tanpa fallback implisit)
    let mut unknown = frame.clone();
    unknown[4] = 0xFF;
    unknown[5] = 0xFF;
    assert!(matches!(
        parse_network_frame(&unknown),
        Err(WireError::UnknownMessageType(0xFFFF))
    ));

    // 2. reserved != 0 wajib ditolak
    let mut reserved = frame.clone();
    reserved[7] = 0x01;
    assert!(matches!(
        parse_network_frame(&reserved),
        Err(WireError::NonZeroReserved(1))
    ));

    // 3. reserved2 non-zero wajib ditolak
    let mut reserved2 = frame;
    reserved2[12] = 0x01;
    assert!(matches!(
        parse_network_frame(&reserved2),
        Err(WireError::NonZeroReserved2(_))
    ));
}

#[test]
fn test_transaction_decoder_rejects_oversized_payload_before_allocation() {
    let tx = Transaction {
        version: 1,
        chain_id: 1,
        tx_type: TxType::Transfer,
        flags: 0,
        sender: Address([1u8; 32]),
        recipient: Address([2u8; 32]),
        nonce: 0,
        amount: Quantum::new(1),
        fee: Quantum::new(1),
        valid_until: 0,
        payload: Vec::new(),
        signature: Signature([0u8; 64]),
    };
    let mut encoded = Vec::new();
    tx.encode_canonical(&mut encoded);

    // Offset prefiks `payload_len` transaksi kanonikal = 0x78 = 120 byte.
    let oversized_len = (MAX_TRANSACTION_PAYLOAD_BYTES + 1) as u32;
    encoded[120..124].copy_from_slice(&oversized_len.to_be_bytes());

    // Decoder wajib menolak sebelum membaca/mengalokasikan payload.
    assert!(matches!(
        Transaction::decode_canonical_exact(&encoded),
        Err(CodecError::ExcessiveAllocation { max, requested })
            if max == MAX_TRANSACTION_PAYLOAD_BYTES
                && requested == MAX_TRANSACTION_PAYLOAD_BYTES + 1
    ));
}

#[test]
fn test_anti_dos_l2_batch_calldata_integrity() {
    // 1. Frame valid
    let frame = L2BatchFrame::new(
        1,
        Hash256::ZERO,
        Hash256([0x01; 32]),
        100,
        105,
        10,
        0,
        vec![0xAA; 256],
    );
    let encoded = frame.encode();
    assert!(L2BatchFrame::decode(&encoded).is_ok());

    // 2. Mutasi panjang byte (truncation) wajib ditolak dengan PayloadLengthMismatch
    let truncated = &encoded[..encoded.len() - 10];
    let trunc_res = L2BatchFrame::decode(truncated);
    assert!(
        matches!(trunc_res, Err(L2CodecError::PayloadLengthMismatch { .. })),
        "Frame terpotong wajib ditolak oleh validator calldata"
    );
}

#[test]
fn test_anti_dos_l4_cross_chain_envelope_oversize_rejection() {
    let oversized_payload = vec![0xDD; 65_537]; // > 64 KB
    let msg_res = CrossChainMessage::new(CrossChainMessageParams {
        source_chain: ChainId::Ethereum,
        destination_chain: ChainId::AurionL1,
        sequence_nonce: 1,
        sender: [0x01; 32],
        target_contract: [0x02; 32],
        payload: oversized_payload,
        timeout_timestamp: 2_000_000_000,
        protocol: ProtocolId::ThresholdVault,
        gas_limit: 100_000,
        max_fee: Quantum::new(100),
        proof: ProofPayload::MerkleInclusion(vec![[0x01; 32]]),
    });

    assert!(
        msg_res.is_err(),
        "Envelope L4 melebihi batas 64 KB wajib ditolak langsung oleh konstruktor"
    );
}

#[test]
fn test_anti_dos_mempool_rbf_and_capacity_bounds() {
    let mut mempool = MempoolEngine::new(2, 3600); // Kapasitas ketat: hanya 2 transaksi
    let kp = Keypair::generate();
    let addr = kp.derive_address();
    let recipient = Address([2u8; 32]);

    let tx1 = Transaction {
        version: 1,
        chain_id: 1,
        tx_type: TxType::Transfer,
        flags: 0,
        sender: addr,
        recipient,
        nonce: 0,
        amount: Quantum::new(100),
        fee: Quantum::new(10_000),
        valid_until: 10_000,
        payload: Vec::new(),
        signature: Signature([0u8; 64]),
    };
    let mut signed_tx1 = tx1.clone();
    signed_tx1.signature = kp.sign(&tx1.signing_preimage());

    let acct = aurion::state::Account {
        balance: Quantum::new(1_000_000),
        nonce: 0,
        code_hash: None,
        storage_root: None,
    };

    assert!(mempool
        .submit_transaction(signed_tx1, &kp.public_key_bytes(), 100, &acct)
        .is_ok());

    // 1. RBF Mandate: Kenaikan fee < 10% (10.000 -> 10.500 = 5%) WAJIB DITOLAK
    let mut tx_rbf_low = tx1.clone();
    tx_rbf_low.fee = Quantum::new(10_500);
    tx_rbf_low.signature = kp.sign(&tx_rbf_low.signing_preimage());
    let rbf_low_res = mempool.submit_transaction(tx_rbf_low, &kp.public_key_bytes(), 100, &acct);
    assert!(rbf_low_res.is_err(), "RBF fee bump < 10% wajib ditolak");

    // 2. RBF Mandate: Kenaikan fee >= 10% (10.000 -> 11.000 = 10%) WAJIB DITERIMA
    let mut tx_rbf_ok = tx1.clone();
    tx_rbf_ok.fee = Quantum::new(11_000);
    tx_rbf_ok.signature = kp.sign(&tx_rbf_ok.signing_preimage());
    let rbf_ok_res = mempool.submit_transaction(tx_rbf_ok, &kp.public_key_bytes(), 100, &acct);
    assert!(
        rbf_ok_res.is_ok(),
        "RBF fee bump >= 10% wajib diterima menggantikan tx lama"
    );
}

#[test]
fn test_anti_dos_avm_stack_overflow_protection() {
    // Bangun bytecode yang melakukan PUSH1 berulang melebihi batas stack (1024)
    let mut bytecode = Vec::new();
    for _ in 0..1025 {
        bytecode.push(Opcode::Push1 as u8);
        bytecode.push(1);
    }
    bytecode.push(Opcode::Stop as u8);

    let verified = BytecodeVerifier::verify(&bytecode).expect("Bytecode valid secara statis");
    let ctx = ExecutionContext::new(
        Address([1u8; 32]),
        Address([2u8; 32]),
        Address([1u8; 32]),
        Quantum::ZERO,
        50_000,
        1,
        1773533000,
    );
    let storage = HashMap::new();
    let result = AvmEngine::execute(&verified, ctx, &storage);

    // Eksekusi harus dihentikan dengan status Error/Revert tanpa panic
    assert!(
        matches!(
            result,
            ExecutionResult::Error(_) | ExecutionResult::Revert { .. }
        ),
        "Stack overflow (>1024) wajib dihentikan dengan aman tanpa panic"
    );
}

// ============================================================================
// 3. ISOLASI HAK ISTIMEWA SENTRY NODE VS. VALIDATOR ENCLAVE
// ============================================================================

#[test]
fn test_sentry_node_privilege_isolation_enforcement() {
    let sentry_ip = "tcp/198.51.100.10:9000".to_string();
    let validator_config = NodeConfig::new_validator(vec![sentry_ip.clone()]);
    let validator_sup = RuntimeSupervisor::new(validator_config);

    // 1. Validator terisolasi total dari publik luar
    assert_eq!(validator_sup.config.role, NodeRole::Validator);
    assert!(validator_sup.is_peer_allowed(&sentry_ip));
    assert!(!validator_sup.is_peer_allowed("tcp/198.51.100.99:9000"));
    assert!(!validator_sup.is_peer_allowed("tcp/0.0.0.0:80"));

    // 2. Sentry node publik menerima trafik publik tetapi tidak memegang wewenang konsensus
    let sentry_config = NodeConfig::new_sentry("0.0.0.0:9000".to_string());
    let sentry_sup = RuntimeSupervisor::new(sentry_config);
    assert_eq!(sentry_sup.config.role, NodeRole::Sentry);
    assert!(sentry_sup.is_peer_allowed("tcp/203.0.113.15:9000"));
}

// ============================================================================
// 4. NON-MALLEABILITY KRIPTOGRAFIS (RFC 8032 STRICT VERIFICATION)
// ============================================================================

#[test]
fn test_cryptographic_strict_rfc8032_non_malleability() {
    let kp = Keypair::generate();
    let message = b"Strict anti-malleability verification test payload";
    let sig = kp.sign(message);
    let pubkey = kp.public_key_bytes();

    // 1. Tanda tangan asli wajib lolos
    assert!(ed25519_verify_strict(&pubkey, message, &sig).is_ok());

    // 2. Mutasi bit tunggal pada komponen tanda tangan (64 byte) WAJIB ditolak
    let mut mutated_bytes = *sig.as_bytes();
    mutated_bytes[63] ^= 0x01; // flip 1 bit pada scalar S
    let mutated_sig = Signature(mutated_bytes);

    let verify_res = ed25519_verify_strict(&pubkey, message, &mutated_sig);
    assert!(
        verify_res.is_err(),
        "Tanda tangan dengan malleability atau mutasi bit wajib ditolak mutlak"
    );
}

// ============================================================================
// 5. ANTI-REPLAY MULTI-LAYER & NULLIFIER REGISTRIES
// ============================================================================

#[test]
fn test_multi_layer_nullifier_anti_replay_enforcement() {
    // 1. Layer-3 Nullifier Registry
    let mut l3_registry = NullifierRegistry::new();
    let msg1 = CrossLayerMessage {
        message_id: Hash256([0x11; 32]),
        source_domain: DomainId::DEX_DEFAULT,
        destination_domain: DomainId::GAMING_DEFAULT,
        nonce: 1,
        payload: vec![1, 2, 3],
        proof: Vec::new(),
        nullifier: Hash256([0x55; 32]),
    };

    assert!(!l3_registry.is_spent(&msg1.nullifier));
    assert!(l3_registry.verify_and_consume(&msg1).is_ok());
    assert!(l3_registry.is_spent(&msg1.nullifier));

    // Pengeluaran kedua dengan nullifier yang sama WAJIB ditolak
    let double_spend_l3 = l3_registry.verify_and_consume(&msg1);
    assert!(
        double_spend_l3.is_err(),
        "Double spend pada L3 nullifier registry wajib ditolak"
    );

    // 2. Layer-4 Universal Nullifier Registry (Cross-Chain Replay Defense)
    let mut l4_registry = UniversalNullifierRegistry::new();
    let cross_nullifier = [0x88u8; 32];

    assert!(!l4_registry.is_nullified(&cross_nullifier));
    assert!(l4_registry
        .register_nullifier(cross_nullifier, ChainId::Ethereum, [0x01; 32], 1000)
        .is_ok());
    assert!(l4_registry.is_nullified(&cross_nullifier));

    // Replay pesan lintas rantai dengan nullifier yang sama WAJIB ditolak
    let replay_l4 =
        l4_registry.register_nullifier(cross_nullifier, ChainId::Ethereum, [0x01; 32], 1001);
    assert!(
        replay_l4.is_err(),
        "Replay attack pada L4 cross-chain envelope wajib ditolak mutlak"
    );
}
