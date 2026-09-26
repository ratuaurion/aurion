//! Uji integrasi Smart Contract Visualization untuk Explorer Aurion.
//!
//! Cakupan:
//! 1. Parser AVM Call Frame (payload bukan ABI encoding biasa).
//! 2. Dekode argumen terhadap metadata ABI dari registry off-chain.
//! 3. Penanganan metadata hilang / selector asing (Unknown Method).
//! 4. `ContractDeploy` -> alamat kontrak hasil deploy.
//! 5. Integrasi API `/explorer/tx/:hash` dan `/api/v1/transactions/{hash}`.
//! 6. Truncation payload besar agar UI tidak crash.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::sync::Arc;

use aurion::contract::calldata::encode_call_payload;
use aurion::contract::metadata::{AbiParam, AbiType, AbiValue, MethodAbi};
use aurion::contract::ContractMetadata;
use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::crypto::blake3_hash;
use aurion::gateway::contract_decode::{
    describe_contract_interaction, parse_call_frame, DecodeStatus, TxStatus, MAX_RAW_PAYLOAD_CHARS,
};
use aurion::gateway::rpc::methods::RpcContext;
use aurion::state::account::Account;
use aurion::transaction::types::{Transaction, TxType};
use aurion::vm::opcode::Opcode;

/// Runtime AVM: `PUSH1 0x2A; PUSH1 0; MSTORE; PUSH1 0x20; PUSH1 0; RETURN`.
fn echo_runtime() -> Vec<u8> {
    vec![0x60, 0x2A, 0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xF3]
}

fn transfer_method() -> MethodAbi {
    MethodAbi::new(
        "transfer",
        "transfer(u64,address)",
        vec![
            AbiParam {
                name: "amount".to_string(),
                ty: AbiType::U64,
            },
            AbiParam {
                name: "to".to_string(),
                ty: AbiType::Address,
            },
        ],
        vec![],
        true,
    )
    .expect("method")
}

fn contract_ctx() -> (Arc<RpcContext>, Address, Hash256) {
    let ctx = RpcContext::new(1001);
    let contract = Address::from_bytes([0xC0; 32]);
    let code_hash = blake3_hash(&echo_runtime());
    let mut accounts = HashMap::new();
    accounts.insert(
        contract,
        Account::new_contract(Quantum::ZERO, 0, code_hash, Hash256::ZERO),
    );
    accounts.insert(
        Address::from_bytes([0x11; 32]),
        Account::new(Quantum::new(1_000_000), 0),
    );
    *ctx.accounts.lock().expect("accounts") = accounts;
    (Arc::new(ctx), contract, code_hash)
}

fn register_metadata(
    ctx: &RpcContext,
    contract: Address,
    code_hash: &Hash256,
    methods: Vec<MethodAbi>,
) {
    let bech = aurion::crypto::encode_address_bech32m(&contract, "aur").expect("bech32m");
    let meta = ContractMetadata::new("Echo", 1001, &bech, code_hash, &echo_runtime(), methods)
        .expect("metadata");
    ctx.contract_metadata
        .register(*code_hash, meta.to_json().expect("json"));
}

fn call_tx(sender: Address, contract: Address, payload: Vec<u8>) -> Transaction {
    Transaction {
        version: 1,
        chain_id: 1001,
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

// ---------------------------------------------------------------------------
// 1. Parser AVM Call Frame
// ---------------------------------------------------------------------------

#[test]
fn parses_call_frame_selector_and_arguments() {
    let method = transfer_method();
    let args = [
        AbiValue::U64(1_000_000),
        AbiValue::Address(Address::from_bytes([0xEE; 32])),
    ];
    let payload = encode_call_payload(&method, &args, &echo_runtime()).expect("payload");

    let frame = parse_call_frame(&payload, 2)
        .expect("no error")
        .expect("frame must match arity 2");

    assert_eq!(frame.selector, method.selector, "selector harus sama");
    assert_eq!(frame.words.len(), 2);
    // Argumen didorong terbalik: word pertama di stream = argumen terakhir.
    assert_eq!(frame.words[0], [0xEE; 32]);
    let amount_word = frame.words[1];
    assert_eq!(
        u64::from_be_bytes(amount_word[24..32].try_into().unwrap()),
        1_000_000
    );
    assert_eq!(frame.runtime_bytes, echo_runtime().len());
}

#[test]
fn arity_mismatch_does_not_match() {
    let method = transfer_method();
    let args = [
        AbiValue::U64(7),
        AbiValue::Address(Address::from_bytes([0x11; 32])),
    ];
    let payload = encode_call_payload(&method, &args, &echo_runtime()).expect("payload");

    // Arity berbeda tidak boleh menghasilkan frame (mencegah salah tebak).
    assert!(parse_call_frame(&payload, 0).expect("e0").is_none());
    assert!(parse_call_frame(&payload, 1).expect("e1").is_none());
    // Arity benar matched.
    assert!(parse_call_frame(&payload, 2).expect("e2").is_some());
}

#[test]
fn truncated_payload_returns_error_not_panic() {
    // Selector terpotong -> Err, bukan panic.
    let short = vec![Opcode::Push4 as u8, 0x00, 0x01];
    assert!(parse_call_frame(&short, 0).is_err());
    // Argumen PUSH32 terpotong -> Err.
    let short_arg = vec![Opcode::Push32 as u8, 0x00, 0x01];
    assert!(parse_call_frame(&short_arg, 1).is_err());
    // Payload kosong tidak panic.
    assert!(parse_call_frame(&[], 0).expect("empty ok").is_none());
}

// ---------------------------------------------------------------------------
// 2. Dekode penuh dengan metadata
// ---------------------------------------------------------------------------

#[test]
fn decodes_method_and_arguments_with_metadata() {
    let (ctx, contract, code_hash) = contract_ctx();
    register_metadata(&ctx, contract, &code_hash, vec![transfer_method()]);

    let method = transfer_method();
    let args = [
        AbiValue::U64(42),
        AbiValue::Address(Address::from_bytes([0xAB; 32])),
    ];
    let payload = encode_call_payload(&method, &args, &echo_runtime()).expect("payload");
    let tx = call_tx(Address::from_bytes([0x11; 32]), contract, payload);

    let d = describe_contract_interaction(&ctx, &tx, TxStatus::Finalized);

    assert_eq!(d.kind, "call");
    assert_eq!(d.decode_status, DecodeStatus::Decoded);
    assert_eq!(d.method.as_deref(), Some("transfer(u64,address)"));
    assert_eq!(d.method_name.as_deref(), Some("transfer"));
    assert_eq!(d.contract_name.as_deref(), Some("Echo"));
    let expected_sel = format!("0x{}", hex::encode(method.selector));
    assert_eq!(d.selector.as_deref(), Some(expected_sel.as_str()));
    assert_eq!(d.arguments.len(), 2);

    // Urutan argumen mengikuti deklarasi metadata, bukan urutan push.
    assert_eq!(d.arguments[0].name, "amount");
    assert_eq!(d.arguments[0].abi_type, "U64");
    assert!(
        d.arguments[0].value.contains("42"),
        "got {}",
        d.arguments[0].value
    );
    assert_eq!(d.arguments[1].name, "to");
    assert_eq!(d.arguments[1].abi_type, "Address");
    assert!(
        d.arguments[1].value.contains("aur1"),
        "got {}",
        d.arguments[1].value
    );
}

// ---------------------------------------------------------------------------
// 3. Metadata hilang / selector asing
// ---------------------------------------------------------------------------

#[test]
fn unknown_method_when_metadata_missing() {
    let (ctx, contract, _code_hash) = contract_ctx();
    // Sengaja TIDAK mendaftarkan metadata.
    let method = transfer_method();
    let args = [AbiValue::U64(1), AbiValue::Address(Address::ZERO)];
    let payload = encode_call_payload(&method, &args, &echo_runtime()).expect("payload");
    let tx = call_tx(Address::from_bytes([0x11; 32]), contract, payload);

    let d = describe_contract_interaction(&ctx, &tx, TxStatus::Pending);

    assert_eq!(d.kind, "call");
    assert_eq!(d.decode_status, DecodeStatus::MetadataMissing);
    assert!(d.method.is_none(), "tidak boleh mengarang nama metode");
    // Selector tetap ditampilkan sebagai petunjuk audit.
    assert!(d.selector.is_some(), "selector harus tetap exposes");
    assert!(d.reason.is_some());
    assert!(!d.raw_payload.is_empty());
}

#[test]
fn unknown_method_when_selector_not_in_metadata() {
    let (ctx, contract, code_hash) = contract_ctx();
    // Metadata ada, tapi hanya berisi metode `ping` tanpa argumen.
    let ping = MethodAbi::new("ping", "ping()", vec![], vec![], false).expect("method");
    register_metadata(&ctx, contract, &code_hash, vec![ping]);

    let method = transfer_method();
    let args = [AbiValue::U64(1), AbiValue::Address(Address::ZERO)];
    let payload = encode_call_payload(&method, &args, &echo_runtime()).expect("payload");
    let tx = call_tx(Address::from_bytes([0x11; 32]), contract, payload);

    let d = describe_contract_interaction(&ctx, &tx, TxStatus::Finalized);

    assert_eq!(d.decode_status, DecodeStatus::UnknownMethod);
    assert!(d.method.is_none());
    assert!(d.reason.is_some());
}

// ---------------------------------------------------------------------------
// 4. ContractDeploy
// ---------------------------------------------------------------------------

#[test]
fn deploy_reports_deployed_contract_address() {
    let ctx = RpcContext::new(1001);
    let sender = Address::from_bytes([0x22; 32]);
    let payload = echo_runtime();
    let tx = Transaction {
        version: 1,
        chain_id: 1001,
        tx_type: TxType::ContractDeploy,
        flags: 0,
        sender,
        recipient: Address::ZERO,
        nonce: 3,
        amount: Quantum::ZERO,
        fee: Quantum::new(52_000),
        valid_until: 0,
        payload: payload.clone(),
        signature: Signature::ZERO,
    };

    let d = describe_contract_interaction(&ctx, &tx, TxStatus::Finalized);

    assert_eq!(d.kind, "deploy");
    assert_eq!(d.decode_status, DecodeStatus::Decoded);
    // Alamat harus sama dengan derivasi STF (sender, nonce).
    let expected = aurion::state::stf::derive_contract_address(&sender, 3);
    assert_eq!(
        d.contract_address,
        aurion::crypto::encode_address_bech32m(&expected, "aur").unwrap()
    );
    let expected_hash = blake3_hash(&payload).to_hex();
    assert_eq!(d.code_hash.as_deref(), Some(expected_hash.as_str()));
    assert_eq!(d.bytecode_bytes, payload.len());
    assert!(d.method.is_none(), "deploy tidak punya method call");
    assert!(d.arguments.is_empty());
}

// ---------------------------------------------------------------------------
// 5. Non-kontrak & truncation payload besar
// ---------------------------------------------------------------------------

#[test]
fn plain_transfer_yields_kind_none() {
    let (ctx, _, _) = contract_ctx();
    let tx = Transaction {
        version: 1,
        chain_id: 1001,
        tx_type: TxType::Transfer,
        flags: 0,
        sender: Address::from_bytes([0x11; 32]),
        recipient: Address::from_bytes([0x33; 32]),
        nonce: 0,
        amount: Quantum::new(1_000),
        fee: Quantum::new(10_000),
        valid_until: 0,
        payload: Vec::new(),
        signature: Signature::ZERO,
    };
    let d = describe_contract_interaction(&ctx, &tx, TxStatus::Finalized);
    assert_eq!(d.kind, "none");
    assert_eq!(d.decode_status, DecodeStatus::NotAContract);
}

#[test]
fn large_calldata_is_truncated_not_crashed() {
    let (ctx, contract, code_hash) = contract_ctx();
    register_metadata(&ctx, contract, &code_hash, vec![transfer_method()]);

    // Payload jauh melebihi batas tampilan.
    let mut payload = encode_call_payload(
        &transfer_method(),
        &[AbiValue::U64(1), AbiValue::Address(Address::ZERO)],
        &echo_runtime(),
    )
    .expect("payload");
    payload.extend(std::iter::repeat_n(0x00u8, 20_000));

    let tx = call_tx(Address::from_bytes([0x11; 32]), contract, payload.clone());
    let d = describe_contract_interaction(&ctx, &tx, TxStatus::Finalized);

    assert!(d.raw_payload_truncated, "payload besar harus dipotong");
    assert!(d.raw_payload.len() <= MAX_RAW_PAYLOAD_CHARS + 1);
    // Panjang penuh tetap dilaporkan agar UI bisa memberi tahu.
    assert_eq!(d.raw_payload_bytes, payload.len());
    // Argumen tetap ter-decode dari frame di depan payload besar.
    assert_eq!(d.decode_status, DecodeStatus::Decoded);
    assert_eq!(d.arguments.len(), 2);
}

// ---------------------------------------------------------------------------
// 6. Integrasi API explorer
// ---------------------------------------------------------------------------

#[test]
fn explorer_tx_detail_includes_decoded_contract_interaction() {
    let (ctx, contract, code_hash) = contract_ctx();
    register_metadata(&ctx, contract, &code_hash, vec![transfer_method()]);

    let method = transfer_method();
    let payload = encode_call_payload(
        &method,
        &[
            AbiValue::U64(99),
            AbiValue::Address(Address::from_bytes([0x77; 32])),
        ],
        &echo_runtime(),
    )
    .expect("payload");
    let tx = call_tx(Address::from_bytes([0x11; 32]), contract, payload);
    let tx_id = tx.compute_tx_id();

    // Masukkan ke recent_transactions agar berstatus FINALIZED.
    ctx.recent_transactions.lock().expect("recent").push_back(
        aurion::gateway::rpc::methods::CommittedTxSummary {
            height: 12,
            tx_id,
            tx,
            received_at: 1_700_000_000,
        },
    );

    let json =
        aurion::gateway::explorer::render_tx_by_hash(&ctx, &tx_id.to_hex()).expect("ditemukan");
    let v: serde_json::Value = serde_json::from_str(&json).expect("json valid");

    assert_eq!(v["status"], "FINALIZED");
    assert_eq!(v["tx_type"], "contract_call");
    assert_eq!(v["block_height"], 12);
    assert!(v["raw_payload"].as_str().unwrap().starts_with("0x"));

    let ci = &v["contract_interaction"];
    assert_eq!(ci["kind"], "call");
    assert_eq!(ci["decode_status"], "decoded");
    assert_eq!(ci["method"], "transfer(u64,address)");
    assert_eq!(ci["contract_name"], "Echo");
    assert_eq!(ci["arguments"][0]["name"], "amount");
    assert!(
        ci["arguments"][0]["value"].as_str().unwrap().contains("99"),
        "argumen tidak ter-decode: {ci}"
    );
    // Status eksekusi harus terekspos ke UI.
    assert_eq!(ci["status"], "finalized");
}

#[test]
fn explorer_tx_detail_handles_unknown_hash() {
    let (ctx, _, _) = contract_ctx();
    assert!(aurion::gateway::explorer::render_tx_by_hash(&ctx, "deadbeef").is_none());
}

/// Membuktikan bentuk JSON yang dilihat UI: transaksi call ter-decode penuh.
#[test]
fn documents_decoded_response_shape() {
    let (ctx, contract, code_hash) = contract_ctx();
    register_metadata(&ctx, contract, &code_hash, vec![transfer_method()]);

    let method = transfer_method();
    let payload = encode_call_payload(
        &method,
        &[
            AbiValue::U64(42),
            AbiValue::Address(Address::from_bytes([0xAB; 32])),
        ],
        &echo_runtime(),
    )
    .expect("payload");
    let tx = call_tx(Address::from_bytes([0x11; 32]), contract, payload);
    let tx_id = tx.compute_tx_id();
    ctx.recent_transactions.lock().expect("recent").push_back(
        aurion::gateway::rpc::methods::CommittedTxSummary {
            height: 42,
            tx_id,
            tx,
            received_at: 1_700_000_000,
        },
    );

    let json =
        aurion::gateway::explorer::render_tx_by_hash(&ctx, &tx_id.to_hex()).expect("ditemukan");
    println!("DECODED_SHAPE={json}");
}
