//! Uji end-to-end alur nyata: `deploy -> call -> Explorer`.
//!
//! # Peran file ini
//!
//! Tidak seperti `explorer_contract_decode.rs` yang memakai call frame sintetis,
//! test di sini menjalankan transaksi melalui `state::stf::apply_transaction`
//! sungguhan sehingga:
//!
//! - akun kontrak benar-benar terbentuk di state lengkap dengan `code_hash`
//!   on-chain, bukan map buatan tangan,
//! - alamat hasil deploy berasal dari derivasi STF, bukan ditebak,
//! - Explorer membaca metadata yang terikat ke `code_hash` state nyata,
//! - regresi di STF maupun di dekoder akan sama-sama terdeteksi.
//!
//! # Batas cakupan (sengaja)
//!
//! Konsensus BFT sesungguhnya tidak diuji di sini: itu memerlukan >=3 validator
//! dengan certificate QC. Yang diuji adalah **state transition** lewat STF,
//! yaitu jalur eksekusi yang sama persis dengan blok produksi. Signature
//! Ed25519 ikut diverifikasi, sehingga transaksi yang salah tanda tangan akan
//! ditolak sebelum menyentuh state.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::sync::Arc;

use aurion::contract::calldata::encode_call_payload;
use aurion::contract::metadata::{AbiParam, AbiType, AbiValue, MethodAbi};
use aurion::contract::ContractMetadata;
use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::crypto::{blake3_hash, Keypair};
use aurion::gateway::explorer::render_tx_by_hash;
use aurion::gateway::rpc::methods::{CommittedTxSummary, RpcContext};
use aurion::state::account::Account;
use aurion::state::monetary::MonetaryState;
use aurion::state::stf::apply_transaction;
use aurion::transaction::types::{Transaction, TxType};
use aurion::vm::opcode::Opcode;

const CHAIN_ID: u32 = 1001;
const SEED: [u8; 32] = [0x21; 32];
const FEE: Quantum = Quantum::new(10_000);

/// Runtime AVM: `MSTORE(0, 42); RETURN(0, 32)`.
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
                name: "recipient".to_string(),
                ty: AbiType::Address,
            },
        ],
        vec![],
        true,
    )
    .expect("method")
}

fn key() -> Keypair {
    Keypair::from_seed(&SEED)
}

/// Chain state nyata + konteks RPC Explorer yang berbagi state tersebut.
struct Chain {
    accounts: HashMap<Address, Account>,
    monetary: MonetaryState,
    proposer: Address,
    ctx: Arc<RpcContext>,
}

/// Bangun rantai dengan satu pengirim berdompet.
fn funded_chain() -> Chain {
    let sender = aurion::crypto::derive_address_from_pubkey(&key().public_key_bytes());
    let mut accounts = HashMap::new();
    accounts.insert(sender, Account::new(Quantum::new(100_000_000_000), 0));
    let monetary = MonetaryState::new(Quantum::new(100_000_000_000), Quantum::ZERO);
    let ctx = Arc::new(RpcContext::new(CHAIN_ID));
    *ctx.accounts.lock().expect("accounts") = accounts.clone();
    Chain {
        accounts,
        monetary,
        proposer: sender,
        ctx,
    }
}

impl Chain {
    /// Terapkan satu transaksi melalui STF, lalu buffer sebagai transaksi
    /// terkonfirmasi agar Explorer bisa menampilkannya (`FINALIZED`).
    ///
    /// Mengembalikan alamat kontrak bila transaksi ini adalah deploy.
    fn apply(&mut self, mut tx: Transaction, height: u64) -> Option<Address> {
        tx.signature = key().sign(&tx.signing_preimage());
        let receipt =
            apply_transaction(&mut self.accounts, &mut self.monetary, &self.proposer, &tx)
                .unwrap_or_else(|e| panic!("STF menolak transaksi: {e}"));

        // State Explorer harus mencerminkan state yang baru.
        *self.ctx.accounts.lock().expect("accounts") = self.accounts.clone();
        self.ctx
            .recent_transactions
            .lock()
            .expect("recent")
            .push_back(CommittedTxSummary {
                height,
                tx_id: tx.compute_tx_id(),
                tx,
                received_at: 1_700_000_000 + height,
            });
        receipt.deployed_contract
    }

    /// Daftarkan metadata kontrak ke registry off-chain seperti yang dilakukan
    /// CLI `contract publish-metadata` / RPC `aur_sendContractMetadata`.
    fn register_metadata(&self, contract: Address, code_hash: &Hash256, methods: Vec<MethodAbi>) {
        let bech = aurion::crypto::encode_address_bech32m(&contract, "aur").expect("bech32m");
        // `runtime` harus bytecode yang sama persis dengan yang di-deploy agar
        // `runtime_hash == code_hash` dan binding metadata valid.
        let meta =
            ContractMetadata::new("Echo", CHAIN_ID, &bech, code_hash, &echo_runtime(), methods)
                .expect("metadata");
        self.ctx
            .contract_metadata
            .register(*code_hash, meta.to_json().expect("metadata json"));
    }
}

fn deploy_tx(sender: Address, nonce: u64) -> Transaction {
    Transaction {
        version: 1,
        chain_id: CHAIN_ID,
        tx_type: TxType::ContractDeploy,
        flags: 0,
        sender,
        recipient: Address::ZERO,
        nonce,
        amount: Quantum::ZERO,
        fee: FEE,
        valid_until: 0,
        payload: echo_runtime(),
        signature: Signature::ZERO,
    }
}

fn call_tx(sender: Address, contract: Address, nonce: u64, payload: Vec<u8>) -> Transaction {
    Transaction {
        version: 1,
        chain_id: CHAIN_ID,
        tx_type: TxType::ContractCall,
        flags: 0,
        sender,
        recipient: contract,
        nonce,
        amount: Quantum::ZERO,
        fee: FEE,
        valid_until: 0,
        payload,
        signature: Signature::ZERO,
    }
}

// ---------------------------------------------------------------------------
// 1. Deploy nyata melalui STF -> Explorer menampilkan "Contract Deployment"
// ---------------------------------------------------------------------------

#[test]
fn deploy_through_stf_is_shown_as_contract_deployment() {
    let mut chain = funded_chain();
    let sender = aurion::crypto::derive_address_from_pubkey(&key().public_key_bytes());

    let deployed = chain.apply(deploy_tx(sender, 0), 1);
    let contract = aurion::state::stf::derive_contract_address(&sender, 0);
    assert_eq!(
        deployed,
        Some(contract),
        "STF harus melaporkan alamat kontrak hasil deploy"
    );

    // STF harus benar-benar membentuk akun kontrak di state.
    let acct = chain
        .accounts
        .get(&contract)
        .expect("akun kontrak harus ada");
    assert!(acct.is_contract(), "akun hasil deploy harus bercode_hash");
    assert_eq!(
        acct.code_hash,
        Some(blake3_hash(&echo_runtime())),
        "code_hash on-chain harus = blake3(payload deploy)"
    );

    // Explorer harus membaca state itu, bukan state buatan.
    let tx_id = chain.ctx.recent_transactions.lock().expect("recent")[0].tx_id;
    let json = render_tx_by_hash(&chain.ctx, &tx_id.to_hex()).expect("deploy harus ditemukan");
    let v: serde_json::Value = serde_json::from_str(&json).expect("json valid");
    let ci = &v["contract_interaction"];

    assert_eq!(v["tx_type"], "contract_deploy");
    assert_eq!(v["status"], "FINALIZED");
    assert_eq!(v["block_height"], 1);
    assert_eq!(ci["kind"], "deploy");
    assert_eq!(ci["decode_status"], "decoded");
    assert_eq!(ci["bytecode_bytes"], echo_runtime().len());

    // Alamat kontrak harus persis derivasi STF, dalam bech32m.
    let expected = aurion::crypto::encode_address_bech32m(&contract, "aur").expect("bech32m");
    assert_eq!(ci["contract_address"], expected);
    assert_eq!(ci["code_hash"], blake3_hash(&echo_runtime()).to_hex());
    // Deploy tidak punya selector/argumen.
    assert!(ci["selector"].is_null());
    assert_eq!(ci["arguments"].as_array().expect("array").len(), 0);
}

// ---------------------------------------------------------------------------
// 2. Alur lengkap: deploy -> call -> Explorer ter-decode
// ---------------------------------------------------------------------------

#[test]
fn full_deploy_then_call_lifecycle_decodes_in_explorer() {
    let mut chain = funded_chain();
    let sender = aurion::crypto::derive_address_from_pubkey(&key().public_key_bytes());

    // 2.1 Deploy.
    let deployed = chain.apply(deploy_tx(sender, 0), 1);
    let contract = deployed.expect("deploy harus melaporkan kontrak");
    assert!(
        chain.accounts.contains_key(&contract),
        "akun kontrak harus ada"
    );

    // 2.2 Daftarkan metadata pada code_hash yang BENAR-benar ada di state.
    let code_hash = chain.accounts[&contract].code_hash.expect("code_hash");
    chain.register_metadata(contract, &code_hash, vec![transfer_method()]);

    // 2.3 Call: nonce 1 (deploy sudah menaikkan nonce sender menjadi 1).
    let method = transfer_method();
    let payload = encode_call_payload(
        &method,
        &[
            AbiValue::U64(7_777),
            AbiValue::Address(Address::from_bytes([0x9A; 32])),
        ],
        &echo_runtime(),
    )
    .expect("payload");
    chain.apply(call_tx(sender, contract, 1, payload), 2);
    let call_id = chain.ctx.recent_transactions.lock().expect("recent")[1].tx_id;

    // 2.4 Explorer harus men-decode call dari state nyata tersebut.
    let json = render_tx_by_hash(&chain.ctx, &call_id.to_hex()).expect("call harus ditemukan");
    let v: serde_json::Value = serde_json::from_str(&json).expect("json valid");
    let ci = &v["contract_interaction"];

    assert_eq!(v["tx_type"], "contract_call");
    assert_eq!(ci["kind"], "call");
    assert_eq!(
        ci["decode_status"], "decoded",
        "metadata terpasang, harus ter-decode"
    );
    assert_eq!(ci["contract_name"], "Echo");
    assert_eq!(ci["method"], "transfer(u64,address)");
    assert_eq!(ci["method_name"], "transfer");

    // Alamat kontrak pada tampilan harus kontrak yang benar-benar di-deploy.
    let expected_addr = aurion::crypto::encode_address_bech32m(&contract, "aur").expect("bech32m");
    assert_eq!(ci["contract_address"], expected_addr);
    assert_eq!(ci["code_hash"], code_hash.to_hex());

    // Argumen harus terbaca dalam URUTAN DEKLARASI metadata.
    let args = ci["arguments"].as_array().expect("arguments");
    assert_eq!(args.len(), 2);
    assert_eq!(args[0]["index"], 0);
    assert_eq!(args[0]["name"], "amount");
    assert_eq!(args[0]["abi_type"], "U64");
    assert!(
        args[0]["value"].as_str().expect("value").contains("7777"),
        "amount harus terbaca 7777: {ci}"
    );
    assert_eq!(args[1]["name"], "recipient");
    assert_eq!(args[1]["abi_type"], "Address");
    assert!(
        args[1]["value"].as_str().expect("value").contains("aur1"),
        "recipient harus bech32m: {ci}"
    );

    // Fee & nonce benar-benar berubah -> bukti STF dieksekusi, bukan di-mock.
    assert_eq!(v["fee_quanta"], FEE.as_u128().to_string());
    assert_eq!(chain.accounts[&sender].nonce, 2, "nonce naik per transaksi");

    // Saldo bersih tetap utuh karena proposer == sender dan `split_fee`
    // mengalokasikan 100% fee ke proposer (0% burn). Fee dipotong lalu
    // dikreditkan balik ke akun yang sama, jadi saldo bersih tidak berubah.
    // Yang dibuktikan di sini adalah nonce naik dan fee tercatat di respons.
    assert_eq!(
        chain.accounts[&sender].balance,
        Quantum::new(100_000_000_000),
        "proposer==sender membuat fee berbalik (0% burn)"
    );
}

/// Fee harus benar-benar mengurangi saldo bila proposer BUKAN pengirim.
///
/// Test ini menutup celah: `full_deploy_then_call_...` memakai proposer yang
/// sama dengan sender sehingga fee netto nol dan bisa menutupi bug pemotongan.
#[test]
fn fee_is_deducted_when_proposer_differs_from_sender() {
    let mut chain = funded_chain();
    let sender = aurion::crypto::derive_address_from_pubkey(&key().public_key_bytes());
    // Proposer terpisah yang juga punya akun, agar `entry().or_default()`
    // tidak mengubah apa pun yang tidak relevan dengan pembuktian fee.
    let proposer = Address::from_bytes([0xE0; 32]);
    chain
        .accounts
        .insert(proposer, Account::new(Quantum::ZERO, 0));
    chain.proposer = proposer;

    chain.apply(deploy_tx(sender, 0), 1);

    // Satu transaksi: saldo sender harus berkurang tepat sebesar fee.
    assert_eq!(
        chain.accounts[&sender].balance,
        Quantum::new(100_000_000_000)
            .checked_sub(FEE)
            .expect("fee dipotong"),
        "fee harus benar-benar mengurangi saldo saat proposer != sender"
    );
    // Dan proposer menerimanya.
    assert_eq!(chain.accounts[&proposer].balance, FEE);
}

// ---------------------------------------------------------------------------
// 3. Tanpa metadata -> Unknown Method, tapi selector tetap tampil
// ---------------------------------------------------------------------------

#[test]
fn call_without_metadata_degrades_to_unknown_method() {
    let mut chain = funded_chain();
    let sender = aurion::crypto::derive_address_from_pubkey(&key().public_key_bytes());

    chain.apply(deploy_tx(sender, 0), 1);
    let contract = aurion::state::stf::derive_contract_address(&sender, 0);
    // SENGAJA tidak mendaftarkan metadata.

    let method = transfer_method();
    let payload = encode_call_payload(
        &method,
        &[
            AbiValue::U64(5),
            AbiValue::Address(Address::from_bytes([0x1B; 32])),
        ],
        &echo_runtime(),
    )
    .expect("payload");
    chain.apply(call_tx(sender, contract, 1, payload), 2);
    let call_id = chain.ctx.recent_transactions.lock().expect("recent")[1].tx_id;

    let json = render_tx_by_hash(&chain.ctx, &call_id.to_hex()).expect("call ditemukan");
    let v: serde_json::Value = serde_json::from_str(&json).expect("json valid");
    let ci = &v["contract_interaction"];

    assert_eq!(ci["decode_status"], "metadata_missing");
    assert!(ci["method"].is_null(), "tidak boleh mengarang nama metode");
    // Selector tetap terekspos sebagai petunjuk audit.
    assert_eq!(
        ci["selector"],
        format!("0x{}", hex::encode(method.selector)),
        "selector harus tetap tampil"
    );
    // Calldata mentah tetap tersedia sebagai fallback audit.
    assert!(!ci["raw_payload"].as_str().expect("raw").is_empty());
    assert!(ci["reason"].as_str().is_some(), "harus ada penjelasan");
}

// ---------------------------------------------------------------------------
// 4. Keamanan: signature palsu & nonce reuse ditolak STF
// ---------------------------------------------------------------------------

#[test]
fn tampered_signature_is_rejected_before_reaching_stf() {
    use aurion::transaction::validator::validate_transaction_stateless;

    let chain = funded_chain();
    let sender = aurion::crypto::derive_address_from_pubkey(&key().public_key_bytes());

    let mut tx = deploy_tx(sender, 0);
    // Ditandatangani seed lain -> signature tidak cocok dengan `tx.sender`.
    let impostor = Keypair::from_seed(&[0x77; 32]);
    tx.signature = impostor.sign(&tx.signing_preimage());

    // Verifikasi signature terjadi di lapisan VALIDASI (sebelum mempool),
    // bukan di STF. STF hanya menjalankan transisi state atas transaksi yang
    // sudah lolos validasi.
    let real_pubkey = key().public_key_bytes();
    let err = validate_transaction_stateless(&tx, &real_pubkey)
        .expect_err("signature palsu harus ditolak");
    assert!(
        format!("{err}").to_lowercase().contains("signature"),
        "error harus menyebut signature: {err}"
    );

    // State tidak boleh tersentuh karena transaksi tak pernah valid.
    assert_eq!(chain.accounts[&sender].nonce, 0);
}

/// Lapisan kedua: pubkey penipu yang menandatangani dengan key-nya sendiri,
/// tetapi menyatakan `tx.sender` milik orang lain, harus ditolak oleh MEMPOOL.
///
/// `validate_transaction_stateless` hanya membuktikan "signature ini sah untuk
/// pubkey ini"; ia TIDAK mengikat pubkey ke `tx.sender`. Pengikatan itu adalah
/// tugas mempool (`MempoolError::SenderMismatch`).
#[test]
fn mempool_rejects_pubkey_that_does_not_match_declared_sender() {
    use aurion::consensus::mempool::engine::MempoolEngine;
    use aurion::consensus::mempool::engine::MempoolError;
    use aurion::mempool::DEFAULT_MAX_MEMPOOL_CAPACITY;
    use aurion::mempool::DEFAULT_MEMPOOL_TTL_SECS;

    let victim = aurion::crypto::derive_address_from_pubkey(&key().public_key_bytes());
    let attacker = Keypair::from_seed(&[0x77; 32]);
    let attacker_pubkey = attacker.public_key_bytes();

    // Penipu menandatangani transaksi yang menyatakan `sender` = korban.
    let mut tx = deploy_tx(victim, 0);
    tx.signature = attacker.sign(&tx.signing_preimage());

    let mut mempool = MempoolEngine::new(DEFAULT_MAX_MEMPOOL_CAPACITY, DEFAULT_MEMPOOL_TTL_SECS);
    let err = mempool
        .submit_transaction(
            tx,
            &attacker_pubkey,
            1_000,
            &Account::new(Quantum::new(1_000_000), 0),
        )
        .expect_err("pubkey tidak cocok dengan sender harus ditolak");
    assert!(
        matches!(err, MempoolError::SenderMismatch { .. }),
        "harus SenderMismatch, dapat: {err}"
    );
    assert_eq!(mempool.len(), 0, "tidak boleh masuk mempool");
}

#[test]
fn replayed_nonce_is_rejected_by_stf() {
    let mut chain = funded_chain();
    let sender = aurion::crypto::derive_address_from_pubkey(&key().public_key_bytes());
    chain.apply(deploy_tx(sender, 0), 1);

    // Deploy ulang dengan nonce 0 (sudah dipakai) harus ditolak.
    let err = apply_transaction(
        &mut chain.accounts,
        &mut chain.monetary,
        &chain.proposer,
        &deploy_tx(sender, 0),
    )
    .expect_err("nonce reuse harus ditolak");
    assert!(
        format!("{err}").to_lowercase().contains("nonce"),
        "error harus menyebut nonce: {err}"
    );
}

// ---------------------------------------------------------------------------
// 5. Explorer read-only: dekoder tidak boleh memutasi state
// ---------------------------------------------------------------------------

#[test]
fn explorer_decoding_never_mutates_state() {
    let mut chain = funded_chain();
    let sender = aurion::crypto::derive_address_from_pubkey(&key().public_key_bytes());
    chain.apply(deploy_tx(sender, 0), 1);
    let contract = aurion::state::stf::derive_contract_address(&sender, 0);
    let code_hash = chain.accounts[&contract].code_hash.expect("code_hash");
    chain.register_metadata(contract, &code_hash, vec![transfer_method()]);

    let method = transfer_method();
    let payload = encode_call_payload(
        &method,
        &[AbiValue::U64(1), AbiValue::Address(Address::ZERO)],
        &echo_runtime(),
    )
    .expect("payload");
    chain.apply(call_tx(sender, contract, 1, payload), 2);
    let call_id = chain.ctx.recent_transactions.lock().expect("recent")[1].tx_id;

    // Salin state sebelum beberapa kali decode.
    let before = chain.ctx.accounts.lock().expect("accounts").clone();
    let before_recent = chain.ctx.recent_transactions.lock().expect("recent").len();

    for _ in 0..3 {
        let json = render_tx_by_hash(&chain.ctx, &call_id.to_hex()).expect("ditemukan");
        assert!(!json.is_empty());
    }

    let after = chain.ctx.accounts.lock().expect("accounts").clone();
    let after_recent = chain.ctx.recent_transactions.lock().expect("recent").len();
    assert_eq!(before, after, "dekoder tidak boleh memutasi akun");
    assert_eq!(
        before_recent, after_recent,
        "dekoder tidak boleh menambah riwayat"
    );
}
