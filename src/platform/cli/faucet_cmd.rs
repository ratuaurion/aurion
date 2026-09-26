#![forbid(unsafe_code)]

//! CLI Faucet Aurion (`aurion faucet ...`) — AUR-MON §2.3.
//!
//! ## Aturan konstitusional yang ditegakkan
//!
//! Faucet resmi **WAJIB** mengambil likuiditas secara eksklusif dari rekening
//! operasional Master Treasury dan **DILARANG** mencetak koin baru di luar
//! mekanisme penerbitan resmi. Modul ini tidak pernahadies lewat jalur
//! pintasan:
//!
//! - `init` membangun transaksi `Transfer` **bertanda tangan Treasury** dan
//!   menyiarkannya ke mempool — dana mengalir lewat consensus reguler.
//! - `claim` hanya bisa memakai saldo yang benar-benar ada di state on-chain.
//!
//! Tidak ada API yang menambah saldo faucet tanpa transfer Treasury nyata.

use std::collections::HashMap;

use crate::core::{Address, Quantum};
use crate::gateway::faucet::{FaucetConfig, FaucetDispenser, FAUCET_AUDIT_CAPACITY};
use crate::mempool::MempoolEngine;
use crate::state::account::Account;

use super::output::OutputFormat;

/// Ambil nilai flag `--key value` dari argumen.
fn flag(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn has_flag(args: &[String], key: &str) -> bool {
    args.iter().any(|a| a == key)
}

/// Parse nominal AUR menjadi `Quantum` (presisi 9 desimal, tanpa float).
fn parse_aur(value: &str) -> Result<Quantum, String> {
    Quantum::from_aur_str(value.trim()).map_err(|e| format!("Nominal '{value}' tidak valid: {e}"))
}

/// Parse alamat penerima (`aur1...` atau hex 64) secara ketat.
fn parse_address(value: &str) -> Result<Address, String> {
    let trimmed = value.trim();
    let hexpart = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    if hexpart.len() == 64 && hexpart.chars().all(|c| c.is_ascii_hexdigit()) {
        let bytes = hex::decode(hexpart).map_err(|e| format!("Alamat hex tidak valid: {e}"))?;
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        return Ok(Address::from_bytes(arr));
    }
    crate::crypto::decode_address_bech32m(trimmed, "aur")
        .map_err(|e| format!("Alamat '{trimmed}' bukan bech32m aur yang valid: {e}"))
}

/// Baca konfigurasi faucet dari flag CLI, dengan default konservatif.
pub fn build_config(args: &[String]) -> Result<FaucetConfig, String> {
    let mut config = FaucetConfig::default();
    if let Some(v) = flag(args, "--amount") {
        config.dispense_amount = parse_aur(&v)?;
    }
    if let Some(v) = flag(args, "--reserve") {
        config.reserve_floor = parse_aur(&v)?;
    }
    if let Some(v) = flag(args, "--fund") {
        config.max_total_dispense = parse_aur(&v)?;
    }
    if let Some(v) = flag(args, "--max-total") {
        config.max_total_dispense = parse_aur(&v)?;
    }
    if let Some(v) = flag(args, "--cooldown") {
        config.cooldown_secs = v
            .parse::<u64>()
            .map_err(|_| format!("--cooldown '{v}' bukan bilangan bulat yang valid"))?;
    }
    config
        .validate()
        .map_err(|e| format!("Konfigurasi faucet tidak valid: {e}"))?;
    Ok(config)
}

/// Ambil password keystore sesuai aturan CLI (AUR-CLI-005).
pub fn read_password(args: &[String]) -> Result<String, String> {
    if has_flag(args, "--password-stdin") {
        let mut buf = String::new();
        std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut buf)
            .map_err(|e| format!("Gagal membaca password dari stdin: {e}"))?;
        return Ok(buf.trim_end_matches(['\r', '\n']).to_string());
    }
    if let Ok(pw) = std::env::var("AURION_WALLET_PASSWORD") {
        return Ok(pw);
    }
    Err(
        "Password keystore tidak tersedia: gunakan --password-stdin atau set \
         AURION_WALLET_PASSWORD"
            .to_string(),
    )
}

/// Buka keystore faucet: `--faucet-keystore`, `--keystore`, atau seed dev.
fn open_faucet_signer(args: &[String]) -> Result<crate::contract::KeystoreSigner, String> {
    use crate::contract::{ApprovalMode, KeystoreSigner};

    match flag(args, "--faucet-keystore").or_else(|| flag(args, "--keystore")) {
        Some(path) => {
            let password = read_password(args)?;
            KeystoreSigner::from_keystore_file(&path, &password, ApprovalMode::AutoApprove)
                .map_err(|e| format!("Gagal membuka keystore faucet '{path}': {e}"))
        }
        None => {
            if has_flag(args, "--faucet-dev-seed") {
                Ok(KeystoreSigner::from_seed(
                    [0xFA; 32],
                    ApprovalMode::AutoApprove,
                ))
            } else {
                Err(
                    "Wajib: --faucet-keystore <path> atau --faucet-dev-seed (khusus devnet)"
                        .to_string(),
                )
            }
        }
    }
}

/// Muat peta akun on-chain dari genesis kanonikal.
fn load_accounts() -> Result<HashMap<Address, Account>, String> {
    let genesis = crate::genesis::ceremony::CeremonyTranscript::canonical_mainnet_genesis();
    let ledger = crate::state::chain::ChainLedger::from_genesis(genesis);
    Ok(ledger.accounts)
}

/// Mempool standalone untuk operasi faucet CLI.
fn new_mempool(chain_id: u32) -> MempoolEngine {
    MempoolEngine::with_chain_id(
        crate::mempool::DEFAULT_MAX_MEMPOOL_CAPACITY,
        crate::mempool::DEFAULT_MEMPOOL_TTL_SECS,
        chain_id,
    )
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn aur_string(q: u128) -> String {
    format!("{}.{:08} AUR", q / 1_000_000_000, q % 1_000_000_000)
}

/// `aurion faucet init` — danai rekening faucet dari Master Treasury.
///
/// Membangun transaksi `Transfer` bertanda tangan Treasury dan menyiarkannya ke
/// mempool. Tidak ada pencetakan koin: saldo faucet hanya bisa bertambah
/// melalui transfer Treasury yang benar-benar melewati consensus.
pub fn cmd_init(args: &[String], format: OutputFormat) -> Result<(), String> {
    use crate::contract::{ApprovalMode, KeystoreSigner, Signer};

    let config = build_config(args)?;
    let amount = match flag(args, "--fund") {
        Some(v) => parse_aur(&v)?,
        None => Quantum::from_aur(1_000).map_err(|e| format!("Nominal default: {e}"))?,
    };

    let treasury_path = flag(args, "--treasury-keystore")
        .ok_or("Wajib: --treasury-keystore <treasury.keystore.json>")?;
    let password = read_password(args)?;
    let treasury =
        KeystoreSigner::from_keystore_file(&treasury_path, &password, ApprovalMode::AutoApprove)
            .map_err(|e| format!("Gagal membuka keystore Master Treasury: {e}"))?;

    let faucet_signer = open_faucet_signer(args)?;
    let chain_id = crate::genesis::builder::GENESIS_CHAIN_ID;
    let mut dispenser = FaucetDispenser::new(faucet_signer.to_keypair(), chain_id, config);
    let faucet_address = dispenser.address();

    let accounts = load_accounts()?;
    let mut mempool = new_mempool(chain_id);

    let (tx_hash, tx) = dispenser
        .fund_from_treasury(
            &treasury.to_keypair(),
            amount,
            &accounts,
            &mut mempool,
            now(),
        )
        .map_err(|e| format!("Pendanaan faucet gagal: {e}"))?;

    let faucet_bech =
        crate::crypto::encode_address_bech32m(&faucet_address, "aur").unwrap_or_default();
    let treasury_hex = treasury.address().to_hex();
    let tx_hex = tx_hash.to_hex();
    let amount_aur = aur_string(amount.as_u128());
    let body = serde_json::json!({
        "status": "FUNDED",
        "faucet_address": faucet_bech,
        "treasury_address": treasury_hex,
        "amount_aur": amount_aur,
        "amount_quanta": amount.as_u128().to_string(),
        "tx_id": tx_hex,
        "nonce": tx.nonce,
        "constitutional_note": "Faucet MUST draw from Master Treasury; MUST NOT mint (AUR-MON 2.3)",
    });

    format.print(&body, || {
        println!("==================================================================");
        println!("            AURION FAUCET - TREASURY FUNDING                      ");
        println!("==================================================================");
        println!("  Status:            FUNDED");
        println!("  Faucet Address:    {faucet_bech}");
        println!("  Treasury Address:  {treasury_hex}");
        println!(
            "  Amount:            {amount_aur} ({} Quanta)",
            amount.as_u128()
        );
        println!("  TxID:              0x{tx_hex}");
        println!("  Nonce:             {}", tx.nonce);
        println!("==================================================================");
        println!("  Dana faucet berasal dari Master Treasury (AUR-MON 2.3).");
        println!("  Tidak ada koin baru yang dicetak.");
        println!("==================================================================");
    });
    Ok(())
}

/// `aurion faucet claim <aur1...>` — request dana untuk satu alamat.
pub fn cmd_claim(args: &[String], format: OutputFormat) -> Result<(), String> {
    let recipient_arg = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .cloned()
        .or_else(|| flag(args, "--to"))
        .ok_or("Wajib: aurion faucet claim <aur1...>  atau  --to <aur1...>")?;
    let recipient = parse_address(&recipient_arg)?;

    let config = build_config(args)?;
    let signer = open_faucet_signer(args)?;
    let chain_id = crate::genesis::builder::GENESIS_CHAIN_ID;
    let mut dispenser = FaucetDispenser::new(signer.to_keypair(), chain_id, config);
    let faucet_address = dispenser.address();

    let accounts = load_accounts()?;
    let mut mempool = new_mempool(chain_id);

    let (tx_hash, _tx) = dispenser
        .dispense(&recipient, &accounts, &mut mempool, now())
        .map_err(|e| format!("Claim gagal: {e}"))?;

    let recipient_bech =
        crate::crypto::encode_address_bech32m(&recipient, "aur").unwrap_or_default();
    let tx_hex = tx_hash.to_hex();
    let amount = dispenser.config.dispense_amount;
    let amount_aur = aur_string(amount.as_u128());
    let cooldown = dispenser.config.cooldown_secs;
    let total = dispenser.total_dispensed_quanta;

    let body = serde_json::json!({
        "status": "DISPENSED",
        "recipient": recipient_bech,
        "faucet_address": crate::crypto::encode_address_bech32m(&faucet_address, "aur")
            .unwrap_or_default(),
        "amount_aur": amount_aur,
        "amount_quanta": amount.as_u128().to_string(),
        "tx_id": tx_hex,
        "cooldown_secs": cooldown,
        "total_dispensed_quanta": total.to_string(),
    });

    format.print(&body, || {
        println!("==================================================================");
        println!("                AURION FAUCET - DISPENSE                          ");
        println!("==================================================================");
        println!("  Status:      DISPENSED");
        println!("  Recipient:   {recipient_bech}");
        println!("  Amount:      {amount_aur} ({} Quanta)", amount.as_u128());
        println!("  TxID:        0x{tx_hex}");
        println!("  Cooldown:    {cooldown} detik per alamat");
        println!("==================================================================");
    });
    Ok(())
}

/// `aurion faucet status` — saldo, reserve floor, dan kuota.
pub fn cmd_status(args: &[String], format: OutputFormat) -> Result<(), String> {
    let config = build_config(args)?;
    let signer = open_faucet_signer(args)?;
    let chain_id = crate::genesis::builder::GENESIS_CHAIN_ID;
    let dispenser = FaucetDispenser::new(signer.to_keypair(), chain_id, config);
    let faucet_address = dispenser.address();

    let accounts = load_accounts()?;
    let balance = accounts
        .get(&faucet_address)
        .map_or(Quantum::ZERO, |a| a.balance);
    let reserve = dispenser.config.reserve_floor;
    let dispense = dispenser.config.dispense_amount;
    // `healthy` = saldo cukup untuk SATU dispense sambil menjaga reserve floor.
    let healthy = balance >= dispenser.config.minimum_funding();
    let funded = !balance.is_zero();
    let remaining = dispenser.remaining_quota();

    let bech = crate::crypto::encode_address_bech32m(&faucet_address, "aur").unwrap_or_default();
    let body = serde_json::json!({
        "faucet_address": bech,
        "chain_id": chain_id,
        "balance_aur": aur_string(balance.as_u128()),
        "balance_quanta": balance.as_u128().to_string(),
        "funded": funded,
        "healthy": healthy,
        "dispense_per_claim_aur": aur_string(dispense.as_u128()),
        "reserve_floor_aur": aur_string(reserve.as_u128()),
        "reserve_floor_quanta": reserve.as_u128().to_string(),
        "cooldown_secs": dispenser.config.cooldown_secs,
        "remaining_quota_quanta": remaining.as_u128().to_string(),
        "constitutional_note": "Faucet MUST draw from Master Treasury; MUST NOT mint (AUR-MON 2.3)",
    });

    format.print(&body, || {
        println!("==================================================================");
        println!("                  AURION FAUCET STATUS                            ");
        println!("==================================================================");
        println!("  Faucet Address:   {bech}");
        println!("  Chain ID:         {chain_id}");
        println!("  Balance:          {}", aur_string(balance.as_u128()));
        println!("  Funded:           {funded}");
        println!("  Per Claim:        {}", aur_string(dispense.as_u128()));
        println!("  Reserve Floor:    {}", aur_string(reserve.as_u128()));
        println!(
            "  Cooldown:         {} detik per alamat",
            dispenser.config.cooldown_secs
        );
        println!("  Remaining Quota:  {} Quanta", remaining.as_u128());
        println!("  Healthy:          {healthy}");
        println!("==================================================================");
        if !funded {
            println!("  PERINGATAN: rekening faucet belum didanai.");
            println!("  Jalankan 'aurion faucet init --treasury-keystore <ks>'.");
        } else if !healthy {
            println!("  PERINGATAN: saldo di bawah minimum dispense + reserve.");
            println!("  Jalankan 'aurion faucet init' untuk menambah dana.");
        }
        println!("  Dana faucet WAJIB dari Master Treasury (AUR-MON 2.3).");
        println!("==================================================================");
    });
    Ok(())
}

/// `aurion faucet audit` — jejak distribusi pada proses simpul berjalan.
///
/// Log audit faucet bersifat in-memory per proses. CLI invokasi baru tidak
/// memiliki log simpul yang sudah berjalan, jadi yang ditampilkan adalah
/// kapasitas, konfigurasi, dan catatan pada proses ini saja — bukan
/// rekonstruksi riwayat rekaan.
pub fn cmd_audit(args: &[String], format: OutputFormat) -> Result<(), String> {
    let config = build_config(args)?;
    let signer = open_faucet_signer(args)?;
    let chain_id = crate::genesis::builder::GENESIS_CHAIN_ID;
    let dispenser = FaucetDispenser::new(signer.to_keypair(), chain_id, config);
    let bech =
        crate::crypto::encode_address_bech32m(&dispenser.address(), "aur").unwrap_or_default();

    let body = serde_json::json!({
        "faucet_address": bech,
        "audit_capacity": FAUCET_AUDIT_CAPACITY,
        "records_in_this_process": dispenser.audit_log.len(),
        "cooldown_secs": dispenser.config.cooldown_secs,
        "note": "Audit log faucet in-memory per proses simpul. \
                 Jalankan simpul dengan --faucet-key untuk log distribusi live.",
    });

    format.print(&body, || {
        println!("==================================================================");
        println!("                 AURION FAUCET AUDIT TRAIL                       ");
        println!("==================================================================");
        println!("  Faucet Address:     {bech}");
        println!("  Audit Capacity:     {FAUCET_AUDIT_CAPACITY}");
        println!("  Records (proses ini): {}", dispenser.audit_log.len());
        println!(
            "  Cooldown:           {} detik",
            dispenser.config.cooldown_secs
        );
        println!("==================================================================");
        println!("  Log audit disimpan in-memory per proses simpul.");
        println!("  Untuk riwayat live, jalankan simpul dengan --faucet-key dan");
        println!("  baca log distribusi dari stdout simpul.");
        println!("==================================================================");
    });
    Ok(())
}
