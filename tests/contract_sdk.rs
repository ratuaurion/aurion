#![forbid(unsafe_code)]

//! Test Suite Contract SDK — Unifikasi Wallet & Smart Contract VM Aurion.
//!
//! Membuktikan pipeline penuh:
//! `metadata -> auto tx builder -> local VM simulation (dry-run STF) ->
//! wallet clear signing -> auto broadcast` tanpa menyentuh konsensus.

use std::cell::RefCell;
use std::rc::Rc;

use aurion::contract::{
    AbiType, AbiValue, ApprovalMode, CallOptions, ContractError, ContractIntent, ContractInstance,
    ContractMetadata, DeployRequest, DryRunReport, IntentAction, KeystoreSigner, MemoryProvider,
    MethodAbi, Provider, Signer, METADATA_SCHEMA,
};
use aurion::core::{Address, Quantum, Signature};
use aurion::crypto::{blake3_hash, derive_address_from_pubkey, encode_address_bech32m};
use aurion::genesis::builder::GENESIS_CHAIN_ID;
use aurion::platform::gateway::rpc::methods::RpcContext;
use aurion::platform::gateway::rpc::types::{JsonRpcId, JsonRpcRequest};
use aurion::state::account::Account;
use aurion::transaction::types::{Transaction, TxType};
use aurion::vm::Opcode;
use aurion::wallet::client::parse_account_value;

// ---------------------------------------------------------------------------
// Fixture: kontrak "Echo" (konstruktor menyimpan 100 di slot 0; runtime
// mengembalikan argumen pertama dari call frame).
// ---------------------------------------------------------------------------

/// Konstruktor: PUSH1 100; PUSH1 0; SSTORE; STOP
fn echo_constructor() -> Vec<u8> {
    vec![
        Opcode::Push1 as u8,
        100,
        Opcode::Push1 as u8,
        0,
        Opcode::SStore as u8,
        Opcode::Stop as u8,
    ]
}

/// Runtime: POP(selector); PUSH1 0; MSTORE(arg1); PUSH1 32; PUSH1 0; RETURN
fn echo_runtime() -> Vec<u8> {
    vec![
        Opcode::Pop as u8,
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

/// Runtime yang selalu revert (untuk menguji gerbang dry-run).
fn reverting_runtime() -> Vec<u8> {
    vec![
        Opcode::Push1 as u8,
        0x00,
        Opcode::Push1 as u8,
        0x00,
        Opcode::Revert as u8,
    ]
}

fn echo_method() -> MethodAbi {
    MethodAbi::new(
        "echo",
        "echo(u64)",
        vec![aurion::contract::AbiParam {
            name: "value".to_string(),
            ty: AbiType::U64,
        }],
        vec![aurion::contract::AbiParam {
            name: "value".to_string(),
            ty: AbiType::U64,
        }],
        false,
    )
    .expect("method ABI")
}

/// Signer yang merekam seluruh prompt clear signing yang diterimanya.
struct RecordingSigner {
    inner: KeystoreSigner,
    prompts: Rc<RefCell<Vec<String>>>,
}

impl RecordingSigner {
    fn new(seed: [u8; 32]) -> Self {
        Self {
            inner: KeystoreSigner::from_seed(seed, ApprovalMode::AutoApprove),
            prompts: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn prompts(&self) -> Vec<String> {
        self.prompts.borrow().clone()
    }
}

impl Signer for RecordingSigner {
    fn address(&self) -> Address {
        self.inner.address()
    }

    fn public_key(&self) -> [u8; 32] {
        self.inner.public_key()
    }

    fn approve(&self, prompt: &str) -> bool {
        self.prompts.borrow_mut().push(prompt.to_string());
        self.inner.approve(prompt)
    }

    fn sign_raw(&self, tx: &aurion::transaction::types::Transaction) -> Result<
        aurion::transaction::types::Transaction,
        ContractError,
    > {
        self.inner.sign_raw(tx)
    }
}


/// Siapkan kontrak Echo ter-deploy beserta instance-nya (fixture pipeline penuh).
fn setup_contract(
    seed: [u8; 32],
    runtime: Vec<u8>,
) -> (
    ContractInstance<MemoryProvider, RecordingSigner>,
    Rc<RefCell<Vec<String>>>,
) {
    let signer = RecordingSigner::new(seed);
    let prompts = signer.prompts.clone();
    let sender = signer.address();
    let provider =
        MemoryProvider::with_account(GENESIS_CHAIN_ID, &sender, Quantum::new(1_000_000_000));
    let request = DeployRequest::new("Echo", echo_constructor(), runtime).with_method(echo_method());
    let (_outcome, instance) =
        ContractInstance::<MemoryProvider, RecordingSigner>::deploy(provider, signer, request)
            .expect("deploy fixture harus berhasil");
    (instance, prompts)
}

#[test]
fn test_metadata_binding_and_selector_derivation() {
    let signer = KeystoreSigner::from_seed([1u8; 32], ApprovalMode::AutoApprove);
    let sender = signer.address();
    let provider =
        MemoryProvider::with_account(GENESIS_CHAIN_ID, &sender, Quantum::new(1_000_000_000));
    let request = DeployRequest::new("Echo", echo_constructor(), echo_runtime())
        .with_method(echo_method());
    let (outcome, instance) =
        ContractInstance::<MemoryProvider, KeystoreSigner>::deploy(provider, signer, request)
            .expect("deploy");

    // code_hash on-chain = blake3(konstruktor) = metadata.
    let account = instance
        .provider()
        .get_account(&instance.address)
        .expect("akun");
    assert_eq!(account.code_hash, Some(outcome.code_hash));
    assert_eq!(outcome.code_hash, blake3_hash(&echo_constructor()));
    instance
        .metadata
        .verify_binding(account.code_hash.as_ref())
        .expect("binding harus valid");

    // Metadata palsu (code_hash berbeda) ditolak.
    let mut fake = instance.metadata.clone();
    fake.code_hash = blake3_hash(b"kode-lain").to_hex();
    let err = fake
        .verify_binding(account.code_hash.as_ref())
        .expect_err("harus ditolak");
    assert!(matches!(err, ContractError::CodeBinding(_)));

    // JSON roundtrip: selector tetap diturunkan dari signature.
    let json = instance.metadata.to_json().expect("json");
    assert!(json.contains(METADATA_SCHEMA));
    let loaded = ContractMetadata::from_json(&json).expect("load");
    assert_eq!(
        loaded.methods[0].selector,
        instance.metadata.methods[0].selector
    );
}

#[test]
fn test_full_pipeline_deploy_and_call_automated() {
    let (instance, prompts) = setup_contract([2u8; 32], echo_runtime());

    // Panggilan otomatis: nonce diambil sendiri, simulasi, lalu clear signing.
    let outcome = instance
        .call("echo", &[AbiValue::U64(42)], &CallOptions::default())
        .expect("call harus berhasil");

    // Hasil simulasi = data kembalian aktual; ter-decode ke ABI.
    assert!(outcome.gas_used > 0, "gas simulasi harus > 0");
    let decoded = outcome.decode_return(AbiType::U64).expect("decode");
    assert_eq!(decoded, AbiValue::U64(42));
    assert!(outcome.simulation.success);
    assert_eq!(outcome.simulation.return_data, outcome.return_data);

    // Ledger berisi 2 transaksi (deploy + call), nonce pengirim otomatis.
    let ledger = instance.provider().ledger();
    assert_eq!(ledger.len(), 2);
    assert_eq!(ledger[1].nonce, 1);
    assert_eq!(
        ledger[1].tx_type,
        TxType::ContractCall
    );
    assert_ne!(ledger[1].signature, Signature::ZERO);

    // Prompt clear signing berbahasa manusia (bukan blind hash).
    let recorded = instance.signer().prompts();
    assert_eq!(recorded.len(), 2, "prompt deploy + prompt call");
    assert_eq!(prompts.borrow().len(), 2, "handle bersama identik");
    let prompt = &recorded[1];
    assert!(prompt.contains("echo(u64)"), "prompt: {prompt}");
    assert!(prompt.contains("value = 42 (U64)"), "prompt: {prompt}");
    assert!(prompt.contains("Dry-Run (STF)    : SUKSES"), "prompt: {prompt}");
    assert!(prompt.contains("Nonce            : 1"), "prompt: {prompt}");
    assert!(prompt.contains("Payload Blake3"), "prompt: {prompt}");

    // View call (read) tidak menambah ledger & tidak meminta persetujuan.
    let report = instance
        .read("echo", &[AbiValue::U64(7)], &CallOptions::default())
        .expect("read");
    assert!(report.success);
    assert_eq!(instance.provider().ledger().len(), 2);
    assert_eq!(instance.signer().prompts().len(), 2);
}

#[test]
fn test_dry_run_gate_blocks_revert_before_signing() {
    let (instance, _prompts) = setup_contract([3u8; 32], reverting_runtime());

    let err = instance
        .call("echo", &[AbiValue::U64(1)], &CallOptions::default())
        .expect_err("revert harus diblokir");
    match err {
        ContractError::SimulationFailed(reason) => {
            assert!(reason.contains("revert"), "alasan: {reason}");
        }
        other => panic!("kesalahan tak terduga: {other:?}"),
    }

    // Tidak ada signing & tidak ada broadcast untuk call yang gagal:
    // hanya prompt deploy (1) yang pernah tercatat.
    assert_eq!(
        instance.signer().prompts().len(),
        1,
        "hanya prompt deploy; call gagal tidak boleh meminta persetujuan"
    );
    assert_eq!(
        instance.provider().ledger().len(),
        1,
        "hanya transaksi deploy yang pernah masuk"
    );
}

#[test]
fn test_dry_run_gate_blocks_bad_nonce_and_invalid_args() {
    let (instance, _prompts) = setup_contract([4u8; 32], echo_runtime());

    let bad_nonce = CallOptions {
        nonce: Some(99),
        ..CallOptions::default()
    };
    let err = instance
        .call("echo", &[AbiValue::U64(1)], &bad_nonce)
        .expect_err("nonce salah harus diblokir");
    assert!(
        matches!(&err, ContractError::SimulationFailed(r) if r.contains("nonce")),
        "err: {err:?}"
    );

    let low_fee = CallOptions {
        fee: Some(Quantum::new(1)),
        ..CallOptions::default()
    };
    let err = instance
        .call("echo", &[AbiValue::U64(1)], &low_fee)
        .expect_err("fee rendah harus ditolak");
    assert!(matches!(err, ContractError::FeeBelowMinimum(1)));

    let unknown = instance
        .call("no_such_method", &[], &CallOptions::default())
        .expect_err("metode tak dikenal harus ditolak");
    assert!(matches!(unknown, ContractError::UnknownMethod(_)));

    let wrong_type = instance
        .call("echo", &[AbiValue::U32(1)], &CallOptions::default())
        .expect_err("tipe argumen salah harus ditolak");
    assert!(matches!(wrong_type, ContractError::Abi(_)));

    assert_eq!(instance.signer().prompts().len(), 1, "hanya prompt deploy");
    assert_eq!(instance.provider().ledger().len(), 1);
}


/// Bangun transaksi contoh (belum ditandatangani) untuk pengujian intent.
fn sample_tx(sender: Address, recipient: Address) -> Transaction {
    Transaction {
        version: 1,
        chain_id: GENESIS_CHAIN_ID,
        tx_type: TxType::ContractCall,
        flags: 0,
        sender,
        recipient,
        nonce: 3,
        amount: Quantum::new(1_000_000_000),
        fee: Quantum::new(10_000),
        valid_until: 9_999_999_999,
        payload: vec![Opcode::Stop as u8],
        signature: Signature::ZERO,
    }
}

/// Intent yang sepenuhnya cocok dengan `sample_tx`.
fn matching_intent(tx: &Transaction) -> ContractIntent {
    ContractIntent {
        action: IntentAction::Call,
        chain_id: tx.chain_id,
        sender: tx.sender,
        sender_bech32m: "aur1q".to_string(),
        tx_recipient: tx.recipient,
        tx_recipient_bech32m: "aur1q".to_string(),
        contract_bech32m: "aur1q".to_string(),
        contract_name: "Echo".to_string(),
        method_name: Some("echo(u64)".to_string()),
        rendered_args: vec!["value = 1 (U64)".to_string()],
        amount: tx.amount,
        fee: tx.fee,
        nonce: tx.nonce,
        valid_until: tx.valid_until,
        payload_hash: blake3_hash(&tx.payload),
        code_hash: blake3_hash(b"kontrak"),
        dry_run: DryRunReport::default(),
    }
}

#[test]
fn test_clear_signing_rejects_tampered_transaction() {
    let signer = RecordingSigner::new([5u8; 32]);
    let sender = signer.address();
    let contract = Address::from_bytes([0x77; 32]);
    let tx = sample_tx(sender, contract);
    let intent = matching_intent(&tx);

    // 1. Transaksi identik lolos verifikasi intent.
    intent.verify_against(&tx).expect("intent harus cocok");

    // 2. Amount dimodifikasi setelah intent dibuat -> ditolak TANPA persetujuan.
    let mut tampered_amount = tx.clone();
    tampered_amount.amount = Quantum::new(66_000_000_000_000_000);
    let err = signer
        .sign(&intent, &tampered_amount)
        .expect_err("tamper amount harus ditolak");
    assert!(matches!(err, ContractError::IntentMismatch(_)), "err: {err:?}");
    assert!(signer.prompts().is_empty(), "tidak boleh ada prompt");

    // 3. Payload dimodifikasi (blind-sign attack) -> hash payload berbeda.
    let mut tampered_payload = tx.clone();
    tampered_payload.payload.push(0xFE);
    let err = signer
        .sign(&intent, &tampered_payload)
        .expect_err("tamper payload harus ditolak");
    assert!(matches!(err, ContractError::IntentMismatch(_)), "err: {err:?}");
    assert!(signer.prompts().is_empty());

    // 4. Nonce dimodifikasi -> ditolak.
    let mut tampered_nonce = tx.clone();
    tampered_nonce.nonce = 9;
    let err = signer
        .sign(&intent, &tampered_nonce)
        .expect_err("tamper nonce harus ditolak");
    assert!(matches!(err, ContractError::IntentMismatch(_)), "err: {err:?}");
    assert!(signer.prompts().is_empty());
}

#[test]
fn test_signer_rejects_foreign_sender() {
    let signer = RecordingSigner::new([6u8; 32]);
    let foreign = Address::from_bytes([0x09; 32]);
    let tx = sample_tx(foreign, Address::from_bytes([0x77; 32]));
    let intent = matching_intent(&tx);

    let err = signer
        .sign(&intent, &tx)
        .expect_err("signer bukan pemilik pengirim");
    assert!(matches!(err, ContractError::SignerMismatch));
    assert!(signer.prompts().is_empty());
}


#[test]
fn test_rpc_get_account_exposes_contract_binding() {
    let ctx = RpcContext::new(GENESIS_CHAIN_ID);
    let code_hash = blake3_hash(b"konstruktor");
    let contract_addr = Address::from_bytes([0x42; 32]);
    let bech32m = encode_address_bech32m(&contract_addr, "aur").expect("bech32m");
    ctx.accounts.lock().expect("lock").insert(
        contract_addr,
        Account {
            balance: Quantum::new(5_000_000),
            nonce: 0,
            code_hash: Some(code_hash),
            storage_root: None,
        },
    );

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: JsonRpcId::Number(1),
        method: "aur_getAccount".to_string(),
        params: vec![bech32m.clone()],
    };
    let response = ctx.dispatch(&request, 1000);
    assert!(response.error.is_none());
    let result = response.result.expect("result");

    // Field binding tersedia untuk Contract SDK.
    assert!(result.contains("\"is_contract\":true"), "result: {result}");
    assert!(
        result.contains(&format!("\"code_hash\":\"{}\"", code_hash.to_hex())),
        "result: {result}"
    );

    // Parser klien memahami hasil ini.
    let value: serde_json::Value = serde_json::from_str(&result).expect("json");
    let parsed = parse_account_value(&value).expect("parse");
    assert_eq!(parsed.balance, 5_000_000);
    assert_eq!(parsed.code_hash, Some(code_hash.0));
    assert!(parsed.is_contract);

    // Tahan terhadap respons simpul lama tanpa field binding.
    let legacy = serde_json::json!({
        "address": bech32m,
        "balance": "10",
        "nonce": 2,
        "consistency": "Finalized"
    });
    let parsed_legacy = parse_account_value(&legacy).expect("parse legacy");
    assert_eq!(parsed_legacy.balance, 10);
    assert_eq!(parsed_legacy.nonce, 2);
    assert_eq!(parsed_legacy.code_hash, None);
    assert!(!parsed_legacy.is_contract);
}

#[test]
fn test_instance_new_validates_chain_and_address() {
    let signer = KeystoreSigner::from_seed([8u8; 32], ApprovalMode::AutoApprove);
    let sender = signer.address();
    let provider =
        MemoryProvider::with_account(GENESIS_CHAIN_ID, &sender, Quantum::new(1_000_000_000));
    let (_outcome, instance) = ContractInstance::<MemoryProvider, KeystoreSigner>::deploy(
        provider,
        signer,
        DeployRequest::new("Echo", echo_constructor(), echo_runtime()).with_method(echo_method()),
    )
    .expect("deploy");

    // 1. Chain ID metadata berbeda dengan Provider -> ditolak.
    let wrong_chain = ContractMetadata {
        chain_id: GENESIS_CHAIN_ID + 1,
        ..instance.metadata.clone()
    };
    let err = ContractInstance::<MemoryProvider, KeystoreSigner>::new(
        &instance.address_bech32m,
        wrong_chain,
        MemoryProvider::new(GENESIS_CHAIN_ID),
        KeystoreSigner::from_seed([8u8; 32], ApprovalMode::AutoApprove),
    )
    .expect_err("chain beda harus ditolak");
    assert!(matches!(
        err,
        ContractError::ChainIdMismatch { metadata, provider }
        if metadata == GENESIS_CHAIN_ID + 1 && provider == GENESIS_CHAIN_ID
    ));

    // 2. Alamat instance berbeda dengan metadata -> ditolak.
    let other_bech32m =
        encode_address_bech32m(&Address::from_bytes([0xEE; 32]), "aur").expect("bech32m");
    let err = ContractInstance::<MemoryProvider, KeystoreSigner>::new(
        &other_bech32m,
        instance.metadata.clone(),
        MemoryProvider::new(GENESIS_CHAIN_ID),
        KeystoreSigner::from_seed([8u8; 32], ApprovalMode::AutoApprove),
    )
    .expect_err("alamat beda harus ditolak");
    assert!(matches!(err, ContractError::InvalidAddress(_)));
}

#[test]
fn test_keystore_signer_unlocks_wallet_keystore() {
    // Kunci dibuat & dienkripsi memakai jalur keystore Wallet yang ada.
    let key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    let address = derive_address_from_pubkey(key.verifying_key().as_bytes());
    let bech32m = encode_address_bech32m(&address, "aur").expect("bech32m");
    let keystore =
        aurion::wallet::keystore::Keystore::encrypt(&key, "rahasia-tes", &bech32m).expect("encrypt");
    let json = keystore.to_json_string();

    let signer =
        KeystoreSigner::from_keystore_json(&json, "rahasia-tes", ApprovalMode::Reject).expect("unlock");
    assert_eq!(signer.address(), address);
    assert_eq!(signer.public_key(), key.verifying_key().to_bytes());
    assert_eq!(signer.approval_mode(), ApprovalMode::Reject);

    // Password salah ditolak.
    let err = KeystoreSigner::from_keystore_json(&json, "salah", ApprovalMode::AutoApprove)
        .expect_err("password salah harus ditolak");
    assert!(matches!(err, ContractError::Provider(_)));
}
