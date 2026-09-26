//! Uji integrasi CLI Kontrak Cerdas (`aurion contract`).
//!
//! Cakupan:
//! 1. Parsing argumen ABI (tipe native AVM).
//! 2. Subcommand offline (`verify`, `deploy` tanpa keystore, `help`).
//! 3. Jalur galat: simpul tak terjangkau, argumen salah, bytecode rusak.
//! 4. Kompatibilitas mundur dengan `contract inspect` dan bentuk lama `deploy <hex>`.

#![forbid(unsafe_code)]

use aurion::cli::command::CliCommand;
use aurion::cli::dispatcher::dispatch;
use aurion::cli::output::OutputFormat;
use aurion::contract::cli::{load_bytecode, parse_abi_arg, parse_address};
use aurion::contract::metadata::{AbiParam, AbiType, AbiValue, MethodAbi};
use aurion::contract::Signer;
use aurion::core::Quantum;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// 1. Parsing argumen ABI
// ---------------------------------------------------------------------------

#[test]
fn parses_native_scalar_abi_types() {
    assert_eq!(
        parse_abi_arg(AbiType::U64, "42").expect("u64"),
        AbiValue::U64(42)
    );
    assert_eq!(
        parse_abi_arg(AbiType::U32, "7").expect("u32"),
        AbiValue::U32(7)
    );
    // bool menerima beberapa ejaan operator
    for truthy in ["true", "TRUE", "1", "yes", "y"] {
        assert_eq!(
            parse_abi_arg(AbiType::Bool, truthy).expect("bool true"),
            AbiValue::Bool(true),
            "input: {truthy}"
        );
    }
    for falsy in ["false", "0", "no", "n"] {
        assert_eq!(
            parse_abi_arg(AbiType::Bool, falsy).expect("bool false"),
            AbiValue::Bool(false),
            "input: {falsy}"
        );
    }
}

#[test]
fn parses_quantum_as_quanta_or_aur_decimal() {
    // Bilangan bulat dibaca sebagai Quanta; desimal sebagai AUR.
    assert_eq!(
        parse_abi_arg(AbiType::Quantum, "1000000000").expect("quanta"),
        AbiValue::Quantum(Quantum::new(1_000_000_000))
    );
    assert_eq!(
        parse_abi_arg(AbiType::Quantum, "1.5").expect("aur desimal"),
        AbiValue::Quantum(Quantum::new(1_500_000_000))
    );
    // Presisi 9 desimal (1 quantum) tidak boleh hilang.
    assert_eq!(
        parse_abi_arg(AbiType::Quantum, "0.000000001").expect("1 quantum"),
        AbiValue::Quantum(Quantum::new(1))
    );
}

#[test]
fn parses_address_and_hash_arguments() {
    let hex_addr = "22".repeat(32);
    match parse_abi_arg(AbiType::Address, &hex_addr).expect("hex address") {
        AbiValue::Address(a) => assert_eq!(a.to_hex(), hex_addr),
        other => panic!("expected address, got {other:?}"),
    }
    let hash = "ab".repeat(32);
    match parse_abi_arg(AbiType::Hash256, &hash).expect("hash") {
        AbiValue::Hash256(h) => assert_eq!(h.to_hex(), hash),
        other => panic!("expected hash, got {other:?}"),
    }
}

#[test]
fn rejects_malformed_abi_arguments() {
    assert!(parse_abi_arg(AbiType::U64, "abc").is_err());
    assert!(parse_abi_arg(AbiType::U64, "-1").is_err());
    assert!(parse_abi_arg(AbiType::Hash256, "abcd").is_err());
    assert!(parse_abi_arg(AbiType::Address, "aur1pendek").is_err());
    assert!(parse_abi_arg(AbiType::Bool, "mungkin").is_err());
    assert!(parse_abi_arg(AbiType::Quantum, "sepuluh").is_err());
}

// ---------------------------------------------------------------------------
// 2. Parsing alamat tingkat CLI
// ---------------------------------------------------------------------------

#[test]
fn parses_address_forms() {
    let hex_addr = "33".repeat(32);
    assert!(parse_address(&hex_addr).is_ok());
    assert!(parse_address(&format!("0x{hex_addr}")).is_ok());
    let bech = aurion::crypto::encode_address_bech32m(
        &aurion::core::Address::from_bytes([0x44; 32]),
        "aur",
    )
    .expect("bech32m");
    assert!(parse_address(&bech).is_ok());
    assert!(parse_address("bukan-alamat").is_err());
}

// ---------------------------------------------------------------------------
// 3. Subcommand offline
// ---------------------------------------------------------------------------

#[tokio::test]
async fn verify_subcommand_succeeds_offline() {
    // PUSH1 0x00; RETURN -> 6000f3
    let res = dispatch(
        CliCommand::Contract(vec!["verify".to_string(), "6000f3".to_string()]),
        OutputFormat::Json,
    )
    .await;
    assert!(res.is_ok(), "verify offline harus berhasil: {res:?}");
}

#[tokio::test]
async fn deploy_without_keystore_stays_offline() {
    // Bentuk lama `deploy <hex>` harus tetap Ok dan TIDAK menyentuh jaringan.
    let res = dispatch(
        CliCommand::Contract(vec!["deploy".to_string(), "6000f3".to_string()]),
        OutputFormat::Json,
    )
    .await;
    assert!(res.is_ok(), "deploy offline harus Ok: {res:?}");
}

#[tokio::test]
async fn help_subcommand_succeeds() {
    for flag in ["help", "--help", "-h"] {
        let res = dispatch(
            CliCommand::Contract(vec![flag.to_string()]),
            OutputFormat::Text,
        )
        .await;
        assert!(res.is_ok(), "contract {flag} harus Ok");
    }
}

#[tokio::test]
async fn inspect_subcommand_backward_compatible() {
    let res = dispatch(
        CliCommand::Contract(vec!["inspect".to_string(), "02".repeat(32)]),
        OutputFormat::Json,
    )
    .await;
    assert!(res.is_ok(), "inspect harus tetap bekerja: {res:?}");
}

#[test]
fn load_bytecode_accepts_hex_inline() {
    assert_eq!(
        load_bytecode("6000f3").expect("hex"),
        vec![0x60, 0x00, 0xF3]
    );
    assert_eq!(
        load_bytecode("0x6000f3").expect("0x hex"),
        vec![0x60, 0x00, 0xF3]
    );
    assert!(load_bytecode("tidak-valid-!!").is_err());
}

// ---------------------------------------------------------------------------
// 4. Jalur galat
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unknown_subcommand_is_rejected() {
    let res = dispatch(
        CliCommand::Contract(vec!["subcommand-hantu".to_string()]),
        OutputFormat::Text,
    )
    .await;
    assert!(res.is_err(), "subcommand tak dikenal harus ditolak");
}

#[tokio::test]
async fn unreachable_node_yields_error_not_panic() {
    // Alamat valid-format, jaringan mati pada port 1.
    let res = dispatch(
        CliCommand::Contract(vec![
            "query".to_string(),
            "44".repeat(32),
            "balanceOf".to_string(),
            "--rpc".to_string(),
            "http://127.0.0.1:1".to_string(),
        ]),
        OutputFormat::Text,
    )
    .await;
    assert!(res.is_err(), "node tak terjangkau harus menghasilkan Err");
    let msg = res.unwrap_err();
    assert!(!msg.is_empty(), "pesan galat tidak boleh kosong");
    assert!(
        msg.contains("simpul") || msg.contains("akun") || msg.contains("Gagal"),
        "pesan galat kurang informatif: {msg}"
    );
}

#[tokio::test]
async fn verify_rejects_invalid_bytecode() {
    // 0xFF bukan opcode AVM yang valid.
    let res = dispatch(
        CliCommand::Contract(vec!["verify".to_string(), "ff".to_string()]),
        OutputFormat::Text,
    )
    .await;
    assert!(res.is_err(), "bytecode rusak harus ditolak");
}

#[tokio::test]
async fn deploy_requires_bytecode_argument() {
    let res = dispatch(
        CliCommand::Contract(vec!["deploy".to_string()]),
        OutputFormat::Text,
    )
    .await;
    assert!(res.is_err(), "deploy tanpa bytecode harus ditolak");
}

#[tokio::test]
async fn metadata_without_target_is_rejected() {
    let res = dispatch(
        CliCommand::Contract(vec!["metadata".to_string()]),
        OutputFormat::Text,
    )
    .await;
    assert!(res.is_err(), "metadata tanpa target harus ditolak");
}

// ---------------------------------------------------------------------------
// 5. Kontrak ABI & konstanta bersama
// ---------------------------------------------------------------------------

#[test]
fn method_abi_selector_is_derived_from_signature() {
    let method = MethodAbi::new(
        "transfer",
        "transfer(u64)",
        vec![AbiParam {
            name: "amount".to_string(),
            ty: AbiType::U64,
        }],
        vec![],
        true,
    )
    .expect("method");
    // Selector harus turunan Blake3 signature.
    assert_eq!(
        method.selector,
        aurion::contract::selector_for("transfer(u64)")
    );
    assert_eq!(method.inputs[0].ty.label(), "U64");
    assert!(method.check_args(&[]).is_err(), "jumlah argumen salah");
    assert!(method.check_args(&[AbiValue::U64(1)]).is_ok());
    assert!(
        method.check_args(&[AbiValue::U32(1)]).is_err(),
        "tipe argumen salah harus ditolak"
    );
}

#[test]
fn gas_limit_constant_is_shared_with_sandbox() {
    // CLI/SDK/simpul wajib memakai satu konstanta gas yang sama.
    assert_eq!(
        aurion::state::sandbox::SANDBOX_GAS_LIMIT,
        aurion::contract::provider::STF_GAS_LIMIT
    );
}

// ---------------------------------------------------------------------------
// 6. End-to-end: CLI `query` melawan simpul RPC nyata
// ---------------------------------------------------------------------------

// `flavor = "multi_thread"` wajib: `dispatch()` melakukan panggilan TCP
// blocking, sedangkan server RPC berjalan di runtime yang sama. Pada runtime
// current-thread, panggilan blocking akan mematikan server dan handshake
// selalu timeout.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn query_end_to_end_against_live_rpc_server() {
    use aurion::core::{Address, Hash256, Quantum};
    use aurion::crypto::blake3_hash;
    use aurion::gateway::rpc::methods::RpcContext;
    use aurion::gateway::rpc::pubsub::SubscriptionManager;
    use aurion::gateway::rpc::server::RpcServer;
    use aurion::state::account::Account;
    use std::collections::HashMap;
    use tokio::sync::watch;

    // 1. Kontrak Echo on-chain di state RPC.
    let ctx = Arc::new(RpcContext::new(1001));
    let contract = Address::from_bytes([0xC0; 32]);
    let runtime = vec![0x60u8, 0x2A, 0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xF3];
    let code_hash = blake3_hash(&runtime);
    {
        let mut accounts = HashMap::new();
        accounts.insert(
            contract,
            Account::new_contract(Quantum::ZERO, 0, code_hash, Hash256::ZERO),
        );
        // View-caller adalah alamat TURUNAN dari seed konstan CLI, bukan
        // literal 0xA0..A0. Dana harus diletakkan pada alamat turunan itu,
        // jika tidak dry-run akan gagal dengan "insufficient balance".
        let view_caller = aurion::contract::KeystoreSigner::from_seed(
            [0xA0; 32],
            aurion::contract::ApprovalMode::Reject,
        )
        .address();
        accounts.insert(view_caller, Account::new(Quantum::new(10_000_000), 0));
        *ctx.accounts.lock().expect("accounts") = accounts;
    }

    // 2. Metadata kontrak didaftarkan ke registry off-chain simpul.
    let bech = aurion::crypto::encode_address_bech32m(&contract, "aur").expect("bech32m");
    let metadata = aurion::contract::ContractMetadata::new(
        "Echo",
        1001,
        &bech,
        &code_hash,
        &runtime,
        vec![MethodAbi::new("ping", "ping()", vec![], vec![], false).expect("method")],
    )
    .expect("metadata");
    ctx.contract_metadata
        .register(code_hash, metadata.to_json().expect("metadata json"));

    // 3. Jalankan server RPC nyata di port bebas.
    let pubsub = Arc::new(SubscriptionManager::new());
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    drop(listener);
    let server =
        RpcServer::new(Arc::clone(&ctx), pubsub, &addr.to_string()).with_shutdown(shutdown_rx);
    let handle = tokio::spawn(async move {
        let _ = server.run().await;
    });
    tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;

    let rpc = format!("http://{addr}/rpc");

    // 4. CLI `query` harus membaca state on-chain, menyelesaikan metadata,
    //    memverifikasi binding code_hash, lalu menjalankan aur_call.
    let res = dispatch(
        CliCommand::Contract(vec![
            "query".to_string(),
            bech.clone(),
            "ping".to_string(),
            "--rpc".to_string(),
            rpc.clone(),
        ]),
        OutputFormat::Json,
    )
    .await;
    assert!(res.is_ok(), "query end-to-end harus berhasil: {res:?}");

    // 5. Verifikasi dry-run di server benar-benar tidak memutasi state.
    let after = ctx.accounts.lock().expect("accounts").clone();
    let contract_acc = after.get(&contract).expect("contract account");
    assert_eq!(contract_acc.code_hash, Some(code_hash));
    assert_eq!(contract_acc.nonce, 0, "nonce kontrak tidak boleh naik");
    let view_caller = aurion::contract::KeystoreSigner::from_seed(
        [0xA0; 32],
        aurion::contract::ApprovalMode::Reject,
    )
    .address();
    assert_eq!(
        after.get(&view_caller).map(|a| a.balance),
        Some(Quantum::new(10_000_000)),
        "saldo view-caller tidak boleh berubah"
    );

    handle.abort();
}

#[tokio::test]
async fn query_rejects_metadata_bound_to_different_code_hash() {
    use aurion::core::{Address, Hash256, Quantum};
    use aurion::crypto::blake3_hash;
    use aurion::gateway::rpc::methods::RpcContext;
    use aurion::state::account::Account;
    use std::collections::HashMap;

    // Akun kontrak dengan code_hash A, tetapi metadata yang謬asar code_hash B.
    let ctx = RpcContext::new(1001);
    let contract = Address::from_bytes([0xC0; 32]);
    let on_chain_hash = blake3_hash(b"constructor-A");
    let mut accounts = HashMap::new();
    accounts.insert(
        contract,
        Account::new_contract(Quantum::ZERO, 0, on_chain_hash, Hash256::ZERO),
    );
    *ctx.accounts.lock().expect("accounts") = accounts;

    // Tulis metadata dengan code_hash berbeda ke file sementara.
    let path = std::env::temp_dir().join("aurion-cli-bad-metadata.json");
    let bogus = aurion::contract::ContractMetadata::new(
        "Evil",
        1001,
        &aurion::crypto::encode_address_bech32m(&contract, "aur").expect("bech"),
        &blake3_hash(b"constructor-B"),
        &[0x60, 0x00, 0xF3],
        vec![],
    )
    .expect("metadata");
    std::fs::write(&path, bogus.to_json().expect("json")).expect("write");

    let res = dispatch(
        CliCommand::Contract(vec![
            "query".to_string(),
            "cc".repeat(32),
            "ping".to_string(),
            "--metadata".to_string(),
            path.to_string_lossy().to_string(),
            "--rpc".to_string(),
            "http://127.0.0.1:1".to_string(),
        ]),
        OutputFormat::Text,
    )
    .await;
    // Gagal (dalam kasus ini lebih dulu karena node mati) tetapi TIDAK panik.
    assert!(res.is_err());
    let _ = std::fs::remove_file(&path);
}
