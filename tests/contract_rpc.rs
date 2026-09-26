//! Integrasi Endpoint Kontrak Cerdas `aur_*` (bridge Contract SDK <-> Node).
//!
//! Menguji persis apa yang dibutuhkan `RpcProvider` di `src/platform/contract/`:
//! `aur_call`, `aur_estimateGas`, `aur_getContractMetadata`, `aur_getCode`, dan
//! `aur_sendRawTransaction` beserta validasi intent-nya.
//!
//! # Invariant yang dijaga
//! - `aur_call` **tidak pernah** memutasi state on-chain (AUR-ARCH-005).
//! - Simulasi lokal (SDK) dan `aur_call` (simpul) memakai `state::sandbox` yang
//!   sama sehingga hasilnya identik.
//! - Kuota DoS token bucket ditegakkan (`-32004`).
//! - Intent mismatch ditolak sebelum masuk mempool (`-32001`).

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use aurion::codec::CanonicalEncode;
use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::crypto::{blake3_hash, derive_address_from_pubkey, Keypair};
use aurion::gateway::rpc::contract_api::CallRateLimiter;
use aurion::gateway::rpc::errors::*;
use aurion::gateway::rpc::methods::RpcContext;
use aurion::gateway::rpc::types::{JsonRpcId, JsonRpcRequest};
use aurion::state::account::Account;
use aurion::state::sandbox::SANDBOX_GAS_LIMIT;
use aurion::transaction::types::{Transaction, TxType};
use aurion::vm::opcode::Opcode;

const CHAIN_ID: u32 = 1001;
const SENDER_SEED: [u8; 32] = [0x21; 32];

/// Runtime AVM: `MSTORE(0, 42); RETURN(0, 32)` -> kembalian word 0x..2A.
fn echo_runtime() -> Vec<u8> {
    vec![
        Opcode::Push1 as u8,
        0x2A,
        Opcode::Push1 as u8,
        0x00,
        Opcode::MStore as u8,
        Opcode::Push1 as u8,
        0x20,
        Opcode::Push1 as u8,
        0x00,
        Opcode::Return as u8,
    ]
}

fn hex_raw(tx: &Transaction) -> String {
    let mut buf = Vec::new();
    tx.encode_canonical(&mut buf);
    hex::encode(buf)
}

fn request(method: &str, params: Vec<String>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: JsonRpcId::Number(1),
        method: method.to_string(),
        params,
    }
}

fn sender_key() -> Keypair {
    Keypair::from_seed(&SENDER_SEED)
}

fn unsigned_call(sender: Address, contract: Address, payload: Vec<u8>) -> Transaction {
    Transaction {
        version: 1,
        chain_id: CHAIN_ID,
        tx_type: TxType::ContractCall,
        flags: 0,
        sender,
        recipient: contract,
        nonce: 0,
        amount: Quantum::ZERO,
        fee: Quantum::new(10_000),
        valid_until: 0,
        payload,
        signature: Signature::ZERO,
    }
}

/// Konteks uji: satu kontrak Echo on-chain + satu pengirim berdompet.
fn context_with_contract() -> (Arc<RpcContext>, Address, Address) {
    let ctx = Arc::new(RpcContext::new(CHAIN_ID));
    let sender = derive_address_from_pubkey(&sender_key().public_key_bytes());
    let contract = Address::from_bytes([0xC0; 32]);

    let mut accounts = HashMap::new();
    accounts.insert(sender, Account::new(Quantum::new(100_000_000_000), 0));
    accounts.insert(
        contract,
        Account::new_contract(
            Quantum::ZERO,
            0,
            blake3_hash(&echo_runtime()),
            Hash256::ZERO,
        ),
    );
    *ctx.accounts.lock().expect("accounts") = accounts;
    (ctx, sender, contract)
}

fn sign(mut tx: Transaction) -> Transaction {
    tx.signature = sender_key().sign(&tx.signing_preimage());
    tx
}

#[test]
fn test_aur_call_executes_on_sandbox_and_returns_gas_and_return_data() {
    let (ctx, sender, contract) = context_with_contract();
    let tx = unsigned_call(sender, contract, echo_runtime());
    let before = ctx.accounts.lock().expect("accounts").clone();

    let resp = ctx.dispatch(&request("aur_call", vec![hex_raw(&tx)]), 1_000);
    assert!(resp.error.is_none(), "error: {:?}", resp.error);
    let result = resp.result.expect("result");
    let value: serde_json::Value = serde_json::from_str(&result).expect("json");

    assert_eq!(value["success"], serde_json::Value::Bool(true), "{result}");
    assert!(value["gas_used"].as_u64().expect("gas") > 0, "{result}");
    assert_eq!(value["gas_limit"].as_u64(), Some(SANDBOX_GAS_LIMIT));
    assert_eq!(value["reason"], serde_json::Value::Null);
    assert_eq!(value["deployed_contract"], serde_json::Value::Null);

    let return_hex = value["return_data"].as_str().expect("return_data");
    let decoded = hex::decode(return_hex).expect("hex");
    assert_eq!(decoded.len(), 32, "RETURN 32 byte: {result}");
    assert_eq!(decoded[31], 0x2A, "nilai 42 dikembalikan: {result}");

    // KRITIS: state on-chain wajib identik sebelum & sesudah.
    assert_eq!(
        ctx.accounts.lock().expect("accounts").clone(),
        before,
        "aur_call tidak boleh memutasi state"
    );
}

#[test]
fn test_aur_call_rejects_contract_call_to_non_contract() {
    let ctx = RpcContext::new(CHAIN_ID);
    let sender = Address::from_bytes([0x01; 32]);
    let plain = Address::from_bytes([0x02; 32]);
    ctx.accounts
        .lock()
        .expect("accounts")
        .insert(sender, Account::new(Quantum::new(1_000_000), 0));

    let tx = unsigned_call(sender, plain, echo_runtime());
    let resp = ctx.dispatch(&request("aur_call", vec![hex_raw(&tx)]), 1_000);

    assert!(resp.error.is_none());
    let value: serde_json::Value =
        serde_json::from_str(&resp.result.expect("result")).expect("json");
    assert_eq!(value["success"], serde_json::Value::Bool(false));
    assert!(value["reason"].as_str().is_some(), "harus ada alasan");
}

#[test]
fn test_aur_call_rejects_bad_params_and_excessive_gas_limit() {
    let ctx = RpcContext::new(CHAIN_ID);
    let sender = Address::from_bytes([0x03; 32]);
    let contract = Address::from_bytes([0x04; 32]);
    let tx = unsigned_call(sender, contract, echo_runtime());

    // 1. Parameter kosong.
    let err = ctx
        .dispatch(&request("aur_call", vec![]), 1_000)
        .error
        .expect("err");
    assert_eq!(err.code, ERR_INVALID_PARAMS);

    // 2. Hex rusak.
    let err = ctx
        .dispatch(&request("aur_call", vec!["zzz".to_string()]), 1_000)
        .error
        .expect("err");
    assert_eq!(err.code, ERR_INVALID_PARAMS);

    // 3. Gas limit melebihi batas kanonik (anti-DoS).
    let err = ctx
        .dispatch(
            &request(
                "aur_call",
                vec![hex_raw(&tx), (SANDBOX_GAS_LIMIT + 1).to_string()],
            ),
            1_000,
        )
        .error
        .expect("err");
    assert_eq!(err.code, ERR_INVALID_PARAMS);

    // 4. Chain ID salah.
    let mut wrong = tx.clone();
    wrong.chain_id = 9999;
    let err = ctx
        .dispatch(&request("aur_call", vec![hex_raw(&wrong)]), 1_000)
        .error
        .expect("err");
    assert_eq!(err.code, ERR_INVALID_PARAMS);
}

#[test]
fn test_aur_call_enforces_rate_limit() {
    let mut ctx = RpcContext::new(CHAIN_ID);
    // Bucket kecil supaya kuota habis dalam jumlah permintaan sedikit.
    ctx.call_limiter = Arc::new(CallRateLimiter::new(0, 2));
    let sender = Address::from_bytes([0x05; 32]);
    let contract = Address::from_bytes([0x06; 32]);
    let tx = unsigned_call(sender, contract, echo_runtime());

    for _ in 0..2 {
        let resp = ctx.dispatch(&request("aur_call", vec![hex_raw(&tx)]), 1_000);
        assert!(resp.error.is_none(), "dua permintaan pertama harus lolos");
    }
    let err = ctx
        .dispatch(&request("aur_call", vec![hex_raw(&tx)]), 1_000)
        .error
        .expect("kuota harus habis");
    assert_eq!(err.code, ERR_RATE_LIMIT_EXCEEDED);
}

#[test]
fn test_aur_estimate_gas_matches_local_sandbox_estimate() {
    let (ctx, sender, contract) = context_with_contract();
    let tx = unsigned_call(sender, contract, echo_runtime());

    let resp = ctx.dispatch(&request("aur_estimateGas", vec![hex_raw(&tx)]), 1_000);
    assert!(resp.error.is_none());
    let value: serde_json::Value =
        serde_json::from_str(&resp.result.expect("result")).expect("json");
    let remote = value["gas_used"].as_u64().expect("gas_used");

    // Kode yang sama persis dengan dry-run SDK => hasil wajib identik.
    let local = aurion::state::sandbox::estimate_gas(&tx).expect("local estimate");
    assert_eq!(
        remote, local,
        "aur_estimateGas harus = sandbox::estimate_gas"
    );
    assert_eq!(value["gas_limit"].as_u64(), Some(SANDBOX_GAS_LIMIT));
    assert_eq!(value["success"], serde_json::Value::Bool(true));
}

#[test]
fn test_aur_estimate_gas_reports_failure_in_payload_not_rpc_error() {
    let ctx = RpcContext::new(CHAIN_ID);
    let sender = Address::from_bytes([0x07; 32]);
    let contract = Address::from_bytes([0x08; 32]);
    // 0xFF bukan opcode AVM -> verifikasi gagal.
    let tx = unsigned_call(sender, contract, vec![0xFF]);

    let resp = ctx.dispatch(&request("aur_estimateGas", vec![hex_raw(&tx)]), 1_000);
    assert!(
        resp.error.is_none(),
        "kegagalan kontrak bukan error transport"
    );
    let value: serde_json::Value =
        serde_json::from_str(&resp.result.expect("result")).expect("json");
    assert_eq!(value["success"], serde_json::Value::Bool(false));
    assert!(value["reason"].as_str().is_some());
}

#[test]
fn test_aur_get_code_exposes_code_hash_binding() {
    let (ctx, _sender, contract) = context_with_contract();
    let bech32m = aurion::crypto::encode_address_bech32m(&contract, "aur").expect("bech32m");

    let resp = ctx.dispatch(&request("aur_getCode", vec![bech32m]), 1_000);
    assert!(resp.error.is_none());
    let result = resp.result.expect("result");
    let value: serde_json::Value = serde_json::from_str(&result).expect("json");

    assert_eq!(value["is_contract"], serde_json::Value::Bool(true));
    assert_eq!(
        value["code_hash"].as_str().expect("code_hash"),
        blake3_hash(&echo_runtime()).to_hex()
    );
    // Kode tidak disimpan on-chain; hanya code_hash (AUR-VM-006).
    assert_eq!(value["code_available"], serde_json::Value::Bool(false));
}

#[test]
fn test_aur_get_contract_metadata_not_found_then_registered() {
    let ctx = RpcContext::new(CHAIN_ID);
    let code_hash = blake3_hash(&echo_runtime());

    // 1. Belum terdaftar -> -32002 (dokumentasi asumsi metadata off-chain).
    let err = ctx
        .dispatch(
            &request("aur_getContractMetadata", vec![code_hash.to_hex()]),
            1_000,
        )
        .error
        .expect("harus not found");
    assert_eq!(err.code, ERR_RESOURCE_NOT_FOUND);

    // 2. Daftarkan metadata sah.
    let contract = Address::from_bytes([0xC0; 32]);
    let bech32m = aurion::crypto::encode_address_bech32m(&contract, "aur").expect("bech32m");
    let metadata = aurion::contract::ContractMetadata::new(
        "Echo",
        CHAIN_ID,
        &bech32m,
        &code_hash,
        &echo_runtime(),
        vec![
            aurion::contract::MethodAbi::new("echo", "echo()", vec![], vec![], false)
                .expect("method"),
        ],
    )
    .expect("metadata");
    let json = metadata.to_json().expect("json");

    let resp = ctx.dispatch(
        &request("aur_sendContractMetadata", vec![code_hash.to_hex(), json]),
        1_000,
    );
    assert!(resp.error.is_none(), "error: {:?}", resp.error);

    // 3. Sekarang dapat diambil dan mem-parsing menjadi ContractMetadata.
    let resp = ctx.dispatch(
        &request("aur_getContractMetadata", vec![code_hash.to_hex()]),
        1_000,
    );
    assert!(resp.error.is_none());
    let raw = resp.result.expect("result");
    let loaded = aurion::contract::ContractMetadata::from_json(&raw).expect("parse metadata");
    assert_eq!(loaded.code_hash, code_hash.to_hex());
    assert_eq!(loaded.methods.len(), 1);
    assert_eq!(loaded.methods[0].name, "echo");
}

#[test]
fn test_aur_send_contract_metadata_rejects_code_hash_mismatch() {
    let ctx = RpcContext::new(CHAIN_ID);
    let contract = Address::from_bytes([0xC0; 32]);
    let bech32m = aurion::crypto::encode_address_bech32m(&contract, "aur").expect("bech32m");
    let metadata = aurion::contract::ContractMetadata::new(
        "Echo",
        CHAIN_ID,
        &bech32m,
        &blake3_hash(&echo_runtime()),
        &echo_runtime(),
        vec![],
    )
    .expect("metadata");
    let json = metadata.to_json().expect("json");

    // code_hash yang diminta sengaja salah.
    let err = ctx
        .dispatch(
            &request(
                "aur_sendContractMetadata",
                vec![blake3_hash(b"lain").to_hex(), json],
            ),
            1_000,
        )
        .error
        .expect("harus ditolak");
    assert_eq!(err.code, ERR_INVALID_PARAMS);
}

#[test]
fn test_send_raw_transaction_accepts_keystore_signed_contract_call() {
    let (ctx, sender, contract) = context_with_contract();
    let pubkey = sender_key().public_key_bytes();
    let tx = sign(unsigned_call(sender, contract, echo_runtime()));

    let resp = ctx.dispatch(
        &request(
            "aur_sendRawTransaction",
            vec![hex_raw(&tx), hex::encode(pubkey)],
        ),
        1_000,
    );
    assert!(resp.error.is_none(), "error: {:?}", resp.error);
    let tx_id = resp.result.expect("result");
    let bytes = hex::decode(tx_id.trim_matches('"').trim_start_matches("0x")).expect("hex");
    assert_eq!(bytes.len(), 32);
    assert_eq!(ctx.mempool.lock().expect("mempool").len(), 1);
}

#[test]
fn test_send_raw_transaction_rejects_intent_payload_hash_mismatch() {
    let (ctx, sender, contract) = context_with_contract();
    let pubkey = sender_key().public_key_bytes();
    let code_hash = blake3_hash(&echo_runtime());
    let tx = sign(unsigned_call(sender, contract, echo_runtime()));

    // payload_hash sengaja diklaim salah (anti-blind-signing guardrail).
    let err = ctx
        .dispatch(
            &request(
                "aur_sendRawTransaction",
                vec![
                    hex_raw(&tx),
                    hex::encode(pubkey),
                    blake3_hash(b"payload-tampered").to_hex(),
                    code_hash.to_hex(),
                ],
            ),
            1_000,
        )
        .error
        .expect("intent mismatch harus ditolak");
    assert_eq!(err.code, ERR_TX_REJECTED);
    assert_eq!(ctx.mempool.lock().expect("mempool").len(), 0);
}

#[test]
fn test_send_raw_transaction_accepts_matching_intent_binding() {
    let (ctx, sender, contract) = context_with_contract();
    let pubkey = sender_key().public_key_bytes();
    let code_hash = blake3_hash(&echo_runtime());
    let payload = echo_runtime();
    let tx = sign(unsigned_call(sender, contract, payload.clone()));

    let resp = ctx.dispatch(
        &request(
            "aur_sendRawTransaction",
            vec![
                hex_raw(&tx),
                hex::encode(pubkey),
                blake3_hash(&payload).to_hex(),
                code_hash.to_hex(),
            ],
        ),
        1_000,
    );
    assert!(resp.error.is_none(), "error: {:?}", resp.error);
    assert_eq!(ctx.mempool.lock().expect("mempool").len(), 1);
}

#[test]
fn test_send_raw_transaction_rejects_foreign_contract_code_hash() {
    let (ctx, sender, contract) = context_with_contract();
    let pubkey = sender_key().public_key_bytes();
    let payload = echo_runtime();
    let tx = sign(unsigned_call(sender, contract, payload.clone()));

    // code_hash yang diklaim bukan code_hash kontrak on-chain.
    let err = ctx
        .dispatch(
            &request(
                "aur_sendRawTransaction",
                vec![
                    hex_raw(&tx),
                    hex::encode(pubkey),
                    blake3_hash(&payload).to_hex(),
                    blake3_hash(b"kontrak-lain").to_hex(),
                ],
            ),
            1_000,
        )
        .error
        .expect("harus ditolak");
    assert_eq!(err.code, ERR_TX_REJECTED);
    assert_eq!(ctx.mempool.lock().expect("mempool").len(), 0);
}

#[test]
fn test_send_raw_transaction_rejects_invalid_contract_bytecode() {
    let (ctx, sender, contract) = context_with_contract();
    let pubkey = sender_key().public_key_bytes();
    // 0xFF bukan opcode AVM -> ditolak sebelum mempool.
    let tx = sign(unsigned_call(sender, contract, vec![0xFF]));

    let err = ctx
        .dispatch(
            &request(
                "aur_sendRawTransaction",
                vec![hex_raw(&tx), hex::encode(pubkey)],
            ),
            1_000,
        )
        .error
        .expect("bytecode rusak harus ditolak");
    assert_eq!(err.code, ERR_TX_REJECTED);
    assert_eq!(ctx.mempool.lock().expect("mempool").len(), 0);
}

#[test]
fn test_backward_compatibility_existing_methods_unchanged() {
    let (ctx, sender, _contract) = context_with_contract();
    ctx.current_height.store(7, Ordering::SeqCst);
    ctx.finalized_height.store(5, Ordering::SeqCst);

    assert_eq!(
        ctx.dispatch(&request("aur_chainId", vec![]), 1_000)
            .result
            .expect("chainId"),
        CHAIN_ID.to_string()
    );
    assert_eq!(
        ctx.dispatch(&request("aur_blockHeight", vec![]), 1_000)
            .result
            .expect("height"),
        "7"
    );
    let bech32m = aurion::crypto::encode_address_bech32m(&sender, "aur").expect("bech32m");
    let result = ctx
        .dispatch(&request("aur_getAccount", vec![bech32m]), 1_000)
        .result
        .expect("account");
    assert!(result.contains("\"balance\""));
    assert!(result.contains("\"nonce\":0"));
    assert!(result.contains("\"code_hash\":null"));
}
