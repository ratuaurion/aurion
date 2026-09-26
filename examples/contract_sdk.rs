//! Contoh Contract SDK Aurion: **deploy dan memanggil kontrak dalam beberapa
//! baris** — seluruh langkah manual (fetch nonce, craft payload, estimasi gas,
//! simulasi, signing, broadcast) diotomatisasi oleh `aurion::contract`.
//!
//! Jalankan: `cargo run --example contract_sdk`

use aurion::contract::{
    AbiParam, AbiType, AbiValue, ApprovalMode, CallOptions, ContractInstance, DeployRequest,
    KeystoreSigner, MemoryProvider, MethodAbi, Signer,
};
use aurion::core::Quantum;
use aurion::genesis::builder::GENESIS_CHAIN_ID;
use aurion::vm::Opcode;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Signer (Wallet): kunci deterministik + mode persetujuan otomatis.
    //    Di produksi gunakan `KeystoreSigner::from_keystore_file(path, pw, ApprovalMode::Interactive)`.
    let signer = KeystoreSigner::from_seed([7u8; 32], ApprovalMode::AutoApprove);

    // 2. Provider: ledger in-process (ganti `RpcProvider::new("http://127.0.0.1:8545")`
    //    untuk terhubung ke simpul jaringan nyata).
    let provider = MemoryProvider::with_account(
        GENESIS_CHAIN_ID,
        &signer.address(),
        Quantum::new(1_000_000_000), // 1 AUR untuk fee & value
    );

    // 3. Definisikan kontrak: konstruktor (disimpan sebagai code_hash on-chain)
    //    dan runtime (dipakai saat call), plus deklarasi ABI metode.
    let constructor = vec![
        Opcode::Push1 as u8,
        100, // simpan 100 di slot 0
        Opcode::Push1 as u8,
        0,
        Opcode::SStore as u8,
        Opcode::Stop as u8,
    ];
    let runtime = vec![
        Opcode::Pop as u8, // buang selector dari call frame
        Opcode::Push1 as u8,
        0x00,
        Opcode::MStore as u8, // simpan argumen pertama ke memori
        Opcode::Push1 as u8,
        0x20,
        Opcode::Push1 as u8,
        0x00,
        Opcode::Return as u8, // kembalikan 32-byte word
    ];
    let echo = MethodAbi::new(
        "echo",
        "echo(u64)",
        vec![AbiParam {
            name: "value".to_string(),
            ty: AbiType::U64,
        }],
        vec![AbiParam {
            name: "value".to_string(),
            ty: AbiType::U64,
        }],
        false,
    )?;

    // 4. DEPLOY otomatis: nonce -> verifikasi -> simulasi STF -> clear signing -> broadcast.
    let request = DeployRequest::new("AurionEcho", constructor, runtime).with_method(echo);
    let (deploy, echo_contract) = ContractInstance::deploy(provider, signer, request)?;
    println!(
        "Deploy TxID   : 0x{}\nKontrak       : {}\nGas simulasi  : {}\n",
        deploy.tx_id.to_hex(),
        deploy.contract_bech32m,
        deploy.gas_used
    );

    // 5. CALL otomatis dalam SATU baris: payload ter-encode, nonce & gas diambil
    //    sendiri, dry-run STF dijalankan, intent clear-signing ditampilkan, lalu
    //    transaksi disiarkan.
    let outcome = echo_contract.call("echo", &[AbiValue::U64(42)], &CallOptions::default())?;
    let returned = outcome.decode_return(AbiType::U64)?;
    println!(
        "Call TxID     : 0x{}\nNonce otomatis: {}\nGas simulasi  : {}\nReturn data   : {:?}\n",
        outcome.tx_id.to_hex(),
        outcome.nonce,
        outcome.gas_used,
        returned
    );
    println!(
        "--- Prompt clear signing yang disetujui ---\n{}",
        outcome.prompt
    );
    Ok(())
}
