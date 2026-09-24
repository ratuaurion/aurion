//! Antarmuka Baris Perintah (CLI) Subcommand Dompet Aurion (`aurion wallet`).
//! Mematuhi Dokumen 01 (01-WALLET-RULES.md) dan Invariant AUR-ARCH-001 (Single Binary).

#![allow(clippy::collapsible_match)]

use crate::wallet::bip39::{entropy_to_mnemonic_24, mnemonic_to_entropy_24, mnemonic_to_seed};
use crate::wallet::client;
use crate::wallet::derivation::DerivedAccount;
use crate::wallet::keystore::Keystore;
use crate::wallet::password::{
    resolve_mnemonic, resolve_password, ENV_WALLET_MNEMONIC, ENV_WALLET_PASSWORD,
};
use crate::wallet::signing::ClearSigningDetails;
use crate::genesis::ceremony::CanonicalCeremonyKeypairs;
use crate::crypto::encode_address_bech32m;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use rand::RngCore;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use zeroize::Zeroizing;

fn generate_wallet_entropy() -> Zeroizing<[u8; 32]> {
    let mut entropy = Zeroizing::new([0u8; 32]);
    let mut rng = OsRng;
    rng.fill_bytes(&mut *entropy);
    entropy
}

pub fn handle_wallet_subcommand(args: &[String]) {
    let subcmd = if !args.is_empty() {
        args[0].as_str()
    } else {
        "help"
    };

    match subcmd {
        "create" => handle_create(&args[1..]),
        "import" => handle_import(&args[1..]),
        "address" => handle_address(&args[1..]),
        "balance" => handle_balance(&args[1..]),
        "nonce" => handle_nonce(&args[1..]),
        "send" => handle_send(&args[1..]),
        "sign-tx" => handle_sign_tx(&args[1..]),
        _ => print_wallet_help(),
    }
}

/// Mendeteksi flag `--password-stdin` dan menghasilkan teks prompt yang benar.
fn parse_password_flags(args: &[String]) -> (bool, String) {
    let from_stdin = args.iter().any(|a| a == "--password-stdin" || a == "--passphrase-stdin");
    let prompt = "Masukkan password wallet";
    (from_stdin, prompt.to_string())
}

fn confirm_clear_signing(assume_yes: bool) -> Result<(), &'static str> {
    if assume_yes {
        return Ok(());
    }
    if !io::stdin().is_terminal() {
        return Err("Lingkungan non-TTY: gunakan flag --yes/-y untuk menandatangani secara non-interaktif");
    }

    eprint!("Continue signing? [y/N]: ");
    io::stderr().flush().map_err(|_| "Gagal menampilkan prompt konfirmasi")?;
    confirm_from_reader(io::stdin().lock())
}

fn confirm_from_reader<R: BufRead>(mut reader: R) -> Result<(), &'static str> {
    let mut input = String::new();
    reader
        .read_line(&mut input)
        .map_err(|_| "Gagal membaca konfirmasi penandatanganan")?;
    if matches!(input.trim(), "y" | "Y") {
        Ok(())
    } else {
        Err("Penandatanganan dibatalkan oleh pengguna")
    }
}

fn handle_create(args: &[String]) {
    let mut name = "default".to_string();
    let mut password: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--name" => {
                if i + 1 < args.len() {
                    name = args[i + 1].clone();
                    i += 1;
                }
            }
            "--password" | "--passphrase" => {
                eprintln!(
                    "[AURION WALLET WARNING] Opsi --password/-p pada argv TIDAK AMAN \
                     (terekspos di process table & shell history). Opsi ini diabaikan. \
                     Gunakan prompt interaktif, --password-stdin, atau env {ENV_WALLET_PASSWORD}."
                );
                if i + 1 < args.len() {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    // Pembangkitan entropi 256-bit dari CSPRNG sistem operasi.
    let entropy = generate_wallet_entropy();
    let mnemonic = entropy_to_mnemonic_24(&entropy);
    let master_seed = mnemonic_to_seed(&mnemonic, "");
    let derived = DerivedAccount::derive_account(&master_seed, 0, 0);

    if password.is_none() {
        let (from_stdin, prompt_text) = parse_password_flags(args);
        match resolve_password(from_stdin, Some(ENV_WALLET_PASSWORD), &prompt_text, true) {
            Ok(pw) => password = Some(pw),
            Err(e) => {
                eprintln!("[AURION WALLET ERROR] Gagal memperoleh password: {e}");
                return;
            }
        }
    }
    let password = password.expect("password guaranteed by earlier branch");

    let keystore = match Keystore::encrypt(&derived.signing_key, &password, &derived.bech32m_address) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("[AURION WALLET ERROR] Gagal mengenkripsi keystore: {e}");
            return;
        }
    };
    let keystore_json = keystore.to_json_string();
    let filename = format!("{name}.keystore.json");

    if let Err(e) = fs::write(&filename, keystore_json) {
        eprintln!("[AURION WALLET] Gagal menyimpan file keystore: {e}");
        return;
    }

    println!("================================================================================");
    println!("                     AURION WALLET CREATED SUCCESSFULLY");
    println!("================================================================================");
    println!("  Address (Bech32m):  {}", derived.bech32m_address);
    println!("  Derivation Path:   {}", derived.path);
    println!("  Keystore File:     {filename}");
    println!("--------------------------------------------------------------------------------");
    println!("  [PERINGATAN KRITIS] SIMPAN 24 KATA MNEMONIK DI TEMPAT AMAN SECARA OFFLINE:");
    println!("  {mnemonic}");
    println!("================================================================================");
}

fn handle_import(args: &[String]) {
    let mut mnemonic: Option<String> = None;
    let mut name = "imported".to_string();
    let mut password: Option<String> = None;
    let mut mnemonic_from_stdin = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--mnemonic" => {
                eprintln!(
                    "[AURION WALLET WARNING] Opsi --mnemonic pada argv TIDAK AMAN (bocor ke process table, \
                     shell history, dan audit log). Diabaikan. Gunakan --mnemonic-stdin, env {ENV_WALLET_MNEMONIC}, \
                     atau prompt interaktif tanpa echo."
                );
                if i + 1 < args.len() {
                    i += 1;
                }
            }
            "--mnemonic-stdin" => {
                mnemonic_from_stdin = true;
            }
            "--name" => {
                if i + 1 < args.len() {
                    name = args[i + 1].clone();
                    i += 1;
                }
            }
            "--password" | "--passphrase" => {
                eprintln!(
                    "[AURION WALLET WARNING] Opsi --password/-p pada argv TIDAK AMAN. Diabaikan. \
                     Gunakan prompt interaktif, --password-stdin, atau env {ENV_WALLET_PASSWORD}."
                );
                if i + 1 < args.len() {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    if mnemonic.is_none() {
        match resolve_mnemonic(mnemonic_from_stdin, "Masukkan mnemonic wallet (24 kata)") {
            Ok(m) => mnemonic = Some(m),
            Err(e) => {
                eprintln!("[AURION WALLET ERROR] Gagal memperoleh mnemonic: {e}");
                return;
            }
        }
    }
    let mnemonic = mnemonic.expect("mnemonic guaranteed by earlier branch");

    if mnemonic.trim().is_empty() {
        eprintln!("[AURION WALLET ERROR] Mnemonic wajib diisi (24 kata).");
        return;
    }

    if let Err(e) = mnemonic_to_entropy_24(&mnemonic) {
        eprintln!("[AURION WALLET ERROR] Mnemonik tidak valid: {e}");
        return;
    }

    if password.is_none() {
        let (from_stdin, prompt_text) = parse_password_flags(args);
        match resolve_password(from_stdin, Some(ENV_WALLET_PASSWORD), &prompt_text, false) {
            Ok(pw) => password = Some(pw),
            Err(e) => {
                eprintln!("[AURION WALLET ERROR] Gagal memperoleh password: {e}");
                return;
            }
        }
    }
    let password = password.expect("password guaranteed by earlier branch");

    let master_seed = mnemonic_to_seed(&mnemonic, "");
    let derived = DerivedAccount::derive_account(&master_seed, 0, 0);

    let keystore = match Keystore::encrypt(&derived.signing_key, &password, &derived.bech32m_address) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("[AURION WALLET ERROR] Gagal mengenkripsi keystore: {e}");
            return;
        }
    };
    let keystore_json = keystore.to_json_string();
    let filename = format!("{name}.keystore.json");

    if let Err(e) = fs::write(&filename, keystore_json) {
        eprintln!("[AURION WALLET ERROR] Gagal menyimpan keystore: {e}");
        return;
    }

    println!("================================================================================");
    println!("                     AURION WALLET IMPORTED SUCCESSFULLY");
    println!("================================================================================");
    println!("  Address (Bech32m):  {}", derived.bech32m_address);
    println!("  Derivation Path:   {}", derived.path);
    println!("  Keystore File:     {filename}");
    println!("================================================================================");
}

fn handle_address(args: &[String]) {
    let mut keystore_path = "default.keystore.json".to_string();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--keystore" && i + 1 < args.len() {
            keystore_path = args[i + 1].clone();
            i += 1;
        }
        i += 1;
    }

    match fs::read_to_string(&keystore_path) {
        Ok(content) => match Keystore::from_json_str(&content) {
            Ok(ks) => {
                println!("{}", ks.address);
            }
            Err(e) => eprintln!("[AURION WALLET ERROR] Gagal mem-parsing keystore: {e}"),
        },
        Err(e) => eprintln!("[AURION WALLET ERROR] Tidak dapat membaca file '{keystore_path}': {e}"),
    }
}

fn get_flag_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

fn rpc_url(args: &[String]) -> String {
    get_flag_value(args, "--rpc").unwrap_or_else(|| "http://127.0.0.1:8545".to_string())
}

fn handle_balance(args: &[String]) {
    let address = get_flag_value(args, "--address").or_else(|| args.first().cloned());
    let Some(address) = address else {
        eprintln!("[AURION WALLET ERROR] Gunakan: wallet balance --address <ADDR> [--rpc <URL>]");
        return;
    };
    match client::get_balance(&rpc_url(args), &address) {
        Ok(balance) => println!("Balance: {balance} Quanta"),
        Err(error) => eprintln!("[AURION WALLET ERROR] {error}"),
    }
}

fn handle_nonce(args: &[String]) {
    let address = get_flag_value(args, "--address").or_else(|| args.first().cloned());
    let Some(address) = address else {
        eprintln!("[AURION WALLET ERROR] Gunakan: wallet nonce --address <ADDR> [--rpc <URL>]");
        return;
    };
    match client::get_nonce(&rpc_url(args), &address) {
        Ok(nonce) => println!("Nonce: {nonce}"),
        Err(error) => eprintln!("[AURION WALLET ERROR] {error}"),
    }
}

fn handle_send(args: &[String]) {
    let dev_sender = args.iter().any(|arg| arg == "--dev-sender");
    let recipient = get_flag_value(args, "--to").unwrap_or_default();
    let amount = get_flag_value(args, "--amount")
        .and_then(|value| value.parse::<u128>().ok())
        .unwrap_or(0);
    let fee = get_flag_value(args, "--fee")
        .and_then(|value| value.parse::<u128>().ok())
        .unwrap_or(10_000);
    let assume_yes = args.iter().any(|arg| arg == "--yes" || arg == "-y");
    let rpc = rpc_url(args);

    let (sender_address, signing_key): (String, SigningKey) = if dev_sender {
        let creator = CanonicalCeremonyKeypairs::new_deterministic().creator;
        let address = encode_address_bech32m(&creator.derive_address(), "aur")
            .expect("canonical creator address encoding must succeed");
        eprintln!("[AURION WALLET] WARNING: --dev-sender uses the deterministic genesis creator treasury key for local testing only.");
        (address, creator.to_signing_key())
    } else {
        let keystore_path = get_flag_value(args, "--keystore")
            .unwrap_or_else(|| "default.keystore.json".to_string());
        let keystore_raw = match fs::read_to_string(&keystore_path) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("[AURION WALLET ERROR] Tidak dapat membuka keystore: {error}");
                return;
            }
        };
        let keystore = match Keystore::from_json_str(&keystore_raw) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("[AURION WALLET ERROR] Format keystore rusak: {error}");
                return;
            }
        };
        let password = match resolve_password(
            args.iter().any(|arg| arg == "--password-stdin"),
            Some(ENV_WALLET_PASSWORD),
            "Masukkan password wallet",
            false,
        ) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("[AURION WALLET ERROR] Gagal memperoleh password: {error}");
                return;
            }
        };
        let (signing_key, _) = match keystore.unlock_and_migrate(&password) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("[AURION WALLET ERROR] Gagal membuka keystore: {error}");
                return;
            }
        };
        (keystore.address, signing_key)
    };
    let nonce = if let Some(n) = get_flag_value(args, "--nonce").and_then(|v| v.parse::<u64>().ok()) {
        n
    } else {
        match client::get_nonce(&rpc, &sender_address) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("[AURION WALLET ERROR] {error}");
                return;
            }
        }
    };
    let details = match ClearSigningDetails::new(
        &sender_address,
        &recipient,
        amount,
        fee,
        nonce,
        "",
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("[AURION WALLET ERROR] Validasi transaksi gagal: {error}");
            return;
        }
    };
    print!("{}", details.format_clear_signing_prompt());
    if assume_yes {
        // Automation explicitly opted into signing.
    } else if let Err(error) = confirm_clear_signing(false) {
        eprintln!("[AURION WALLET ERROR] {error}");
        return;
    }
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (tx, raw_hex) = details.sign(
        &signing_key,
        crate::genesis::builder::GENESIS_CHAIN_ID,
        current_time + 3600,
    );
    let sender_pubkey = hex::encode(signing_key.verifying_key().as_bytes());
    match client::broadcast_raw_tx(&rpc, &raw_hex, &sender_pubkey) {
        Ok(tx_id) => {
            println!("Transaksi berhasil disiarkan!");
            println!("TxID   : 0x{tx_id}");
            println!("Sender : {sender_address}");
            println!("To     : {recipient}");
            println!("Amount : {amount} Quanta");
            println!("Nonce  : {nonce}");
            println!("Status : PENDING (Menunggu finalitas blok)");
            let _ = tx;
        }
        Err(error) => eprintln!("[AURION WALLET ERROR] {error}"),
    }
}

fn handle_sign_tx(args: &[String]) {
    let mut keystore_path = "default.keystore.json".to_string();
    let mut password: Option<String> = None;
    let mut to_addr = String::new();
    let mut amount: u128 = 0;
    let mut fee: u128 = 10_000; // default 0.0001 AUR
    let mut nonce: u64 = 0;
    let mut memo = String::new();
    let mut chain_id: u32 = crate::genesis::builder::GENESIS_CHAIN_ID;
    let mut assume_yes = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--keystore" => {
                if i + 1 < args.len() {
                    keystore_path = args[i + 1].clone();
                    i += 1;
                }
            }
            "--password" | "--passphrase" => {
                eprintln!(
                    "[AURION WALLET WARNING] Opsi --password/-p pada argv TIDAK AMAN. Diabaikan. \
                     Gunakan prompt interaktif, --password-stdin, atau env {ENV_WALLET_PASSWORD}."
                );
                if i + 1 < args.len() {
                    i += 1;
                }
            }
            "--to" => {
                if i + 1 < args.len() {
                    to_addr = args[i + 1].clone();
                    i += 1;
                }
            }
            "--amount" => {
                if i + 1 < args.len() {
                    amount = args[i + 1].parse::<u128>().unwrap_or(0);
                    i += 1;
                }
            }
            "--fee" => {
                if i + 1 < args.len() {
                    fee = args[i + 1].parse::<u128>().unwrap_or(10_000);
                    i += 1;
                }
            }
            "--nonce" => {
                if i + 1 < args.len() {
                    nonce = args[i + 1].parse::<u64>().unwrap_or(0);
                    i += 1;
                }
            }
            "--memo" => {
                if i + 1 < args.len() {
                    memo = args[i + 1].clone();
                    i += 1;
                }
            }
            "--chain-id" => {
                if i + 1 < args.len() {
                    chain_id = args[i + 1].parse::<u32>().unwrap_or(1);
                    i += 1;
                }
            }
            "--yes" | "-y" => {
                assume_yes = true;
            }
            _ => {}
        }
        i += 1;
    }

    if password.is_none() {
        let (from_stdin, prompt_text) = parse_password_flags(args);
        match resolve_password(from_stdin, Some(ENV_WALLET_PASSWORD), &prompt_text, false) {
            Ok(pw) => password = Some(pw),
            Err(e) => {
                eprintln!("[AURION WALLET ERROR] Gagal memperoleh password: {e}");
                return;
            }
        }
    }
    let password = password.expect("password guaranteed by earlier branch");

    let keystore_raw = match fs::read_to_string(&keystore_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[AURION WALLET ERROR] Tidak dapat membuka file keystore '{keystore_path}': {e}");
            return;
        }
    };

    let keystore = match Keystore::from_json_str(&keystore_raw) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("[AURION WALLET ERROR] Format keystore rusak: {e}");
            return;
        }
    };

    let (signing_key, upgraded) = match keystore.unlock_and_migrate(&password) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[AURION WALLET ERROR] Autentikasi kata sandi gagal: {e}");
            return;
        }
    };

    if let Some(upgraded_keystore) = upgraded {
        match fs::write(&keystore_path, upgraded_keystore.to_json_string()) {
            Ok(()) => println!(
                "[AURION WALLET] Keystore legacy dimigrasikan otomatis ke format kanonik V2 (Argon2id + ChaCha20Poly1305)."
            ),
            Err(e) => eprintln!(
                "[AURION WALLET WARNING] Gagal menulis migrasi keystore V2 ke '{keystore_path}': {e}"
            ),
        }
    }

    let details = match ClearSigningDetails::new(
        &keystore.address,
        &to_addr,
        amount,
        fee,
        nonce,
        &memo,
    ) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[AURION WALLET ERROR] Validasi transaksi gagal: {e}");
            return;
        }
    };

    // Cetak Clear Signing Prompt
    print!("{}", details.format_clear_signing_prompt());

    if let Err(error) = confirm_clear_signing(assume_yes) {
        eprintln!("[AURION WALLET ERROR] {error}");
        return;
    }

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let valid_until = current_time + 3600;

    let (_tx, raw_hex) = details.sign(&signing_key, chain_id, valid_until);
    println!("[AURION WALLET] Transaksi Berhasil Ditandatangani!");
    println!("Raw Transaction Hex:");
    println!("{raw_hex}");
}

fn print_wallet_help() {
    println!("Aurion Sovereign Wallet Subsystem CLI (/bin/aurion wallet)");
    println!("Penggunaan:");
    println!("  aurion wallet create [--name <name>] [--password-stdin]");
    println!("  aurion wallet import --mnemonic-stdin [--name <name>] [--password-stdin]");
    println!("  aurion wallet address [--keystore <path>]");
    println!("  aurion wallet balance --address <addr> [--rpc <url>]");
    println!("  aurion wallet nonce --address <addr> [--rpc <url>]");
    println!("  aurion wallet send --to <addr> --amount <quanta> [--fee <quanta>] [--nonce <n>] [--keystore <path>] [--rpc <url>] [--yes|-y] [--dev-sender]");
    println!("  --dev-sender uses the deterministic genesis developer key for local testing only");
    println!("  aurion wallet sign-tx --to <addr> --amount <quanta> --nonce <n> [--keystore <path>] [--password-stdin] [--fee <quanta>] [--memo <text>] [--yes|-y]");
    println!("Keamanan password:");
    println!("  - Tanpa flag, password diminta lewat prompt interaktif (no echo, konfirmasi ganda saat create).");
    println!("  - --password-stdin membaca password dari stdin (aman untuk pipa/CI).");
    println!("  - Env {ENV_WALLET_PASSWORD} dibaca bila stdin adalah terminal.");
    println!("  - Opsi --password/-p PADA ARGV TIDAK DIDUKUNG (tidak aman, diabaikan).");
    println!("Keamanan mnemonic:");
    println!("  - Mnemonic dibaca dari stdin (--mnemonic-stdin), env {ENV_WALLET_MNEMONIC}, atau prompt interaktif tanpa echo.");
    println!("  - Opsi --mnemonic \"<24 words>\" PADA ARGV TIDAK DIDUKUNG (tidak aman, diabaikan).");
}

#[cfg(test)]
mod tests {
    use super::{confirm_from_reader, generate_wallet_entropy};

    #[test]
    fn wallet_entropy_is_non_zero_and_non_deterministic() {
        let samples: Vec<[u8; 32]> = (0..8)
            .map(|_| *generate_wallet_entropy())
            .collect();

        for sample in &samples {
            assert_ne!(*sample, [0u8; 32]);
        }

        for (index, sample) in samples.iter().enumerate() {
            assert!(samples[index + 1..].iter().all(|other| other != sample));
        }
    }

    #[test]
    fn clear_signing_confirmation_accepts_only_yes() {
        assert!(confirm_from_reader(std::io::Cursor::new("y\n")).is_ok());
        assert!(confirm_from_reader(std::io::Cursor::new("Y\n")).is_ok());
        assert!(confirm_from_reader(std::io::Cursor::new("n\n")).is_err());
        assert!(confirm_from_reader(std::io::Cursor::new("\n")).is_err());
    }
}
