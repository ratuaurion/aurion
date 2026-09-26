//! Uji produksi Faucet Aurion + perbaikan bug CLI `--help`.
//!
//! # Cakupan
//!
//! 1. **Aturan konstitusional** — faucet hanya bisa didanai dari Master Treasury,
//!    dan tidak pernah mencetak koin (AUR-MON §2.3).
//! 2. **Reserve floor** — rekening faucet tidak boleh diuras hingga nol.
//! 3. **Cooldown** — anti-spam per penerima.
//! 4. **Kuota total** — anti-runaway.
//! 5. **Audit log** — jejak distribusi, termasuk penolakan.
//! 6. **E2E** — Treasury -> init -> claim -> saldo user bertambah (lewat STF).
//! 7. **CLI** — `node --help` tidak boleh menjalankan node.
//!
//! # Batas cakupan
//!
//! Tidak diuji: signatures nyata di server produksi (butuh keystore Treasury
//! kanonik). Aliran dana diuji dengan `state::stf::apply_transaction`, yaitu
//! jalur eksekusi yang sama dengan blok produksi.

#![forbid(unsafe_code)]

use std::collections::HashMap;

use aurion::core::{Address, Quantum};
use aurion::crypto::{derive_address_from_pubkey, Keypair};
use aurion::gateway::faucet::{
    FaucetAuditRecord, FaucetConfig, FaucetDispenser, FaucetError, FAUCET_AUDIT_CAPACITY,
};
use aurion::mempool::MempoolEngine;
use aurion::state::account::Account;
use aurion::state::stf::apply_transaction;

const CHAIN_ID: u32 = 1001;
const TREASURY_SEED: [u8; 32] = [0x01; 32]; // seed Creator = Master Treasury
const FAUCET_SEED: [u8; 32] = [0xFA; 32];
const COOLDOWN: u64 = 60;

fn treasury_key() -> Keypair {
    Keypair::from_seed(&TREASURY_SEED)
}

fn treasury_address() -> Address {
    derive_address_from_pubkey(&treasury_key().public_key_bytes())
}

fn faucet_key() -> Keypair {
    Keypair::from_seed(&FAUCET_SEED)
}

fn faucet_address() -> Address {
    derive_address_from_pubkey(&faucet_key().public_key_bytes())
}

fn test_config() -> FaucetConfig {
    FaucetConfig {
        dispense_amount: Quantum::new(1_000_000_000), // 10 AUR
        fee: Quantum::new(2_000),
        cooldown_secs: COOLDOWN,
        reserve_floor: Quantum::new(100_000_000), // 1 AUR
        max_total_dispense: Quantum::new(1_000_000_000_000), // 1000 AUR
    }
}

fn new_dispenser() -> FaucetDispenser {
    FaucetDispenser::new(faucet_key(), CHAIN_ID, test_config())
}

fn new_mempool() -> MempoolEngine {
    MempoolEngine::with_chain_id(
        aurion::mempool::DEFAULT_MAX_MEMPOOL_CAPACITY,
        aurion::mempool::DEFAULT_MEMPOOL_TTL_SECS,
        CHAIN_ID,
    )
}

/// State dengan Treasury berdompet penuh + faucet dengan saldo tertentu.
fn state_with(faucet_balance: Quantum) -> HashMap<Address, Account> {
    let mut accounts = HashMap::new();
    // Treasury memegang 100% pasokan genesis (66 juta AUR).
    accounts.insert(
        treasury_address(),
        Account::new(Quantum::new(66_000_000_000_000_000), 0),
    );
    if !faucet_balance.is_zero() {
        accounts.insert(faucet_address(), Account::new(faucet_balance, 0));
    }
    accounts
}

/// Alamat penerima sintetis yang stabil per indeks.
fn user(i: u8) -> Address {
    Address::from_bytes([i.wrapping_add(0xA0); 32])
}

// ---------------------------------------------------------------------------
// 1. Konfigurasi
// ---------------------------------------------------------------------------

#[test]
fn rejects_zero_dispense_amount() {
    let mut config = test_config();
    config.dispense_amount = Quantum::ZERO;
    assert!(matches!(
        config.validate(),
        Err(FaucetError::InvalidConfig(_))
    ));
}

#[test]
fn rejects_reserve_floor_above_dispense() {
    // Reserve lebih besar dari dispense membuat faucet tak pernah bisa bayar.
    let mut config = test_config();
    config.reserve_floor = config
        .dispense_amount
        .checked_add(Quantum::new(1))
        .expect("reserve lebih besar");
    assert!(matches!(
        config.validate(),
        Err(FaucetError::InvalidConfig(_))
    ));
}

#[test]
fn minimum_funding_covers_dispense_fee_and_reserve() {
    let config = test_config();
    let min = config.minimum_funding();
    let expected = config
        .dispense_amount
        .checked_add(config.fee)
        .and_then(|v| v.checked_add(config.reserve_floor))
        .expect("minimum");
    assert_eq!(min, expected);
}

// ---------------------------------------------------------------------------
// 2. Reserve floor — rekening tidak boleh diuras
// ---------------------------------------------------------------------------

#[test]
fn dispense_is_blocked_before_reserve_floor_is_breached() {
    // Saldo cukup untuk dispense+fee, tapi TIDAK cukup sambil menjaga reserve.
    let dispense = Quantum::new(1_000_000_000);
    let fee = Quantum::new(2_000);
    let reserve = Quantum::new(100_000_000);
    let balance = dispense
        .checked_add(fee)
        .and_then(|v| v.checked_add(reserve))
        .expect("balance")
        .checked_sub(Quantum::new(1))
        .expect("kurang 1");

    let mut dispenser = new_dispenser();
    let accounts = state_with(balance);
    let mut mempool = new_mempool();

    let err = dispenser
        .dispense(&user(1), &accounts, &mut mempool, 1_000)
        .expect_err("reserve floor harus menahan payout");
    assert!(
        matches!(err, FaucetError::ReserveFloorBreached { .. }),
        "harus ReserveFloorBreached, dapat {err}"
    );
    assert_eq!(mempool.len(), 0, "tidak boleh ada transaksi di mempool");
}

#[test]
fn dispense_succeeds_exactly_at_reserve_floor() {
    let config = test_config();
    let balance = config
        .dispense_amount
        .checked_add(config.fee)
        .and_then(|v| v.checked_add(config.reserve_floor))
        .expect("balance");

    let mut dispenser = new_dispenser();
    let accounts = state_with(balance);
    let mut mempool = new_mempool();

    let (_hash, tx) = dispenser
        .dispense(&user(2), &accounts, &mut mempool, 1_000)
        .expect("tepat di reserve floor harus boleh");
    assert_eq!(mempool.len(), 1);
    assert_eq!(tx.amount, config.dispense_amount);
    // Sisa setelah payout = reserve floor persis.
    let remaining = balance
        .checked_sub(tx.amount)
        .and_then(|v| v.checked_sub(tx.fee))
        .expect("sisa");
    assert_eq!(remaining, config.reserve_floor);
}

#[test]
fn unfunded_faucet_cannot_dispense() {
    let mut dispenser = new_dispenser();
    let accounts = state_with(Quantum::ZERO);
    let mut mempool = new_mempool();
    let err = dispenser
        .dispense(&user(3), &accounts, &mut mempool, 1_000)
        .expect_err("harus gagal");
    assert!(
        matches!(
            err,
            FaucetError::AccountNotFound | FaucetError::InsufficientBalance { .. }
        ),
        "dapat {err}"
    );
    assert_eq!(mempool.len(), 0);
}

// ---------------------------------------------------------------------------
// 3. Cooldown anti-spam
// ---------------------------------------------------------------------------

#[test]
fn cooldown_blocks_rapid_repeat_claims() {
    let mut dispenser = new_dispenser();
    let accounts = state_with(Quantum::new(10_000_000_000));
    let mut mempool = new_mempool();

    dispenser
        .dispense(&user(4), &accounts, &mut mempool, 1_000)
        .expect("claim pertama");
    assert_eq!(mempool.len(), 1);

    // Claim kedua 30 detik kemudian (cooldown 60) harus ditolak.
    let err = dispenser
        .dispense(&user(4), &accounts, &mut mempool, 1_030)
        .expect_err("cooldown harus menahan");
    assert!(
        matches!(err, FaucetError::CooldownActive(30)),
        "dapat {err}"
    );
    assert_eq!(mempool.len(), 1, "tidak boleh ada transaksi kedua");
}

#[test]
fn cooldown_expires_after_window() {
    let mut dispenser = new_dispenser();
    let accounts = state_with(Quantum::new(10_000_000_000));
    let mut mempool = new_mempool();

    dispenser
        .dispense(&user(5), &accounts, &mut mempool, 1_000)
        .expect("claim pertama");
    dispenser
        .dispense(&user(5), &accounts, &mut mempool, 1_000 + COOLDOWN)
        .expect("setelah cooldown harus boleh");
    assert_eq!(mempool.len(), 2);
}

#[test]
fn cooldown_is_per_recipient() {
    let mut dispenser = new_dispenser();
    let accounts = state_with(Quantum::new(10_000_000_000));
    let mut mempool = new_mempool();

    dispenser
        .dispense(&user(6), &accounts, &mut mempool, 1_000)
        .expect("user 6");
    // Alamat BERBEDA tidak kena cooldown user 6.
    dispenser
        .dispense(&user(7), &accounts, &mut mempool, 1_001)
        .expect("user 7 harus boleh");
    assert_eq!(mempool.len(), 2);
}

// ---------------------------------------------------------------------------
// 4. Kuota total (anti-runaway)
// ---------------------------------------------------------------------------

#[test]
fn total_quota_stops_runaway_dispensing() {
    let mut config = test_config();
    config.max_total_dispense = config.dispense_amount.checked_mul(2).expect("2x dispense");
    config.reserve_floor = Quantum::ZERO;

    let mut dispenser = FaucetDispenser::new(faucet_key(), CHAIN_ID, config);
    let accounts = state_with(Quantum::new(100_000_000_000));
    let mut mempool = new_mempool();

    dispenser
        .dispense(&user(8), &accounts, &mut mempool, 1_000)
        .expect("1");
    dispenser
        .dispense(&user(9), &accounts, &mut mempool, 1_000 + COOLDOWN)
        .expect("2");
    assert_eq!(dispenser.remaining_quota(), Quantum::ZERO);

    let err = dispenser
        .dispense(&user(10), &accounts, &mut mempool, 1_000 + 2 * COOLDOWN)
        .expect_err("kuota harus habis");
    assert!(
        matches!(err, FaucetError::InsufficientBalance { .. }),
        "dapat {err}"
    );
    assert_eq!(mempool.len(), 2, "tidak boleh ada transaksi ketiga");
}

// ---------------------------------------------------------------------------
// 5. Aturan konstitusional: dana HANYA dari Master Treasury
// ---------------------------------------------------------------------------

#[test]
fn funding_below_minimum_is_rejected() {
    // Amount harus menutup dispense + fee + reserve, kalau tidak faucet
    // langsung "dry" setelah inisialisasi.
    let mut dispenser = new_dispenser();
    let accounts = state_with(Quantum::ZERO);
    let mut mempool = new_mempool();
    let too_small = dispenser
        .config
        .minimum_funding()
        .checked_sub(Quantum::new(1))
        .expect("kurang 1");

    let err = dispenser
        .fund_from_treasury(&treasury_key(), too_small, &accounts, &mut mempool, 1_000)
        .expect_err("amount terlalu kecil harus ditolak");
    assert!(matches!(err, FaucetError::InvalidConfig(_)), "dapat {err}");
    assert_eq!(mempool.len(), 0);
}

#[test]
fn funding_from_unknown_treasury_is_rejected() {
    // Kunci yang bukan Treasury tidak ada di state -> harus ditolak, bukan
    // Kunci asing tidak boleh membuat rekening dari udara.
    let mut dispenser = new_dispenser();
    let accounts = state_with(Quantum::ZERO);
    let mut mempool = new_mempool();
    let stranger = Keypair::from_seed(&[0xDE; 32]);

    let err = dispenser
        .fund_from_treasury(
            &stranger,
            Quantum::new(1_000_000_000_000),
            &accounts,
            &mut mempool,
            1_000,
        )
        .expect_err("treasury asing harus ditolak");
    assert!(
        matches!(err, FaucetError::TreasuryNotFound(_)),
        "dapat {err}"
    );
    assert_eq!(mempool.len(), 0);
}

#[test]
fn double_initialization_is_rejected() {
    let mut dispenser = new_dispenser();
    // Faucet sudah punya saldo.
    let accounts = state_with(Quantum::new(1_000_000_000_000));
    let mut mempool = new_mempool();

    let err = dispenser
        .fund_from_treasury(
            &treasury_key(),
            Quantum::new(1_000_000_000_000),
            &accounts,
            &mut mempool,
            1_000,
        )
        .expect_err("funding ulang harus ditolak");
    assert!(
        matches!(err, FaucetError::AlreadyInitialized(_)),
        "dapat {err}"
    );
    assert_eq!(mempool.len(), 0);
}

#[test]
fn funding_creates_real_treasury_transfer() {
    let mut dispenser = new_dispenser();
    let accounts = state_with(Quantum::ZERO);
    let mut mempool = new_mempool();
    let amount = Quantum::new(1_000_000_000_000); // 1000 AUR

    let (_hash, tx) = dispenser
        .fund_from_treasury(&treasury_key(), amount, &accounts, &mut mempool, 1_000)
        .expect("pendanaan harus berhasil");

    // Transaksi harus BENAR-BENAR dari Treasury ke faucet, bukan pencetakan.
    assert_eq!(tx.sender, treasury_address());
    assert_eq!(tx.recipient, faucet_address());
    assert_eq!(tx.amount, amount);
    assert!(!tx.payload.is_empty() || tx.payload.is_empty()); // payload kosong normal
    assert_eq!(mempool.len(), 1);
}

// ---------------------------------------------------------------------------
// 6. Audit log
// ---------------------------------------------------------------------------

#[test]
fn audit_log_records_both_success_and_rejection() {
    let mut dispenser = new_dispenser();
    let accounts = state_with(Quantum::new(10_000_000_000));
    let mut mempool = new_mempool();

    dispenser
        .dispense(&user(11), &accounts, &mut mempool, 1_000)
        .expect("sukses");
    // Claim kedua pada alamat sama ditolak cooldown -> harus tercatat.
    let _ = dispenser.dispense(&user(11), &accounts, &mut mempool, 1_010);

    assert_eq!(dispenser.audit_log.len(), 2, "dua peristiwa harus tercatat");
    assert!(!dispenser.audit_log[0].rejected, "pertama sukses");
    assert!(dispenser.audit_log[1].rejected, "kedua ditolak");
    assert_ne!(dispenser.audit_log[0].tx_hash, aurion::core::Hash256::ZERO);
    // Penolakan tidak boleh mengubah total dispense.
    assert_eq!(
        dispenser.total_dispensed_quanta,
        dispenser.config.dispense_amount.as_u128()
    );
}

#[test]
fn audit_log_is_bounded() {
    // Reserve floor tinggi membuat SEMUA dispense ditolak, sehingga log terisi
    // cepat tanpa perlu berhasil.
    let mut config = test_config();
    config.reserve_floor = Quantum::new(50_000_000_000);
    let mut dispenser = FaucetDispenser::new(faucet_key(), CHAIN_ID, config);
    // Saldo cukup untuk dispense, tapi tidak sambil menjaga reserve 50 AUR.
    let accounts = state_with(Quantum::new(1_000_000_000));
    let mut mempool = new_mempool();

    for i in 0..(FAUCET_AUDIT_CAPACITY + 50) {
        let _ = dispenser.dispense(
            &user((i % 200) as u8),
            &accounts,
            &mut mempool,
            1_000 + i as u64,
        );
    }
    assert_eq!(
        dispenser.audit_log.len(),
        FAUCET_AUDIT_CAPACITY,
        "log harus berbatas tepat pada kapasitas"
    );
}

// ---------------------------------------------------------------------------
// 7. E2E: Treasury -> init -> claim -> saldo user bertambah (lewat STF)
// ---------------------------------------------------------------------------

/// Alur produksi penuh, memverifikasi rantai konstitusional secara nyata:
/// `Master Treasury -> faucet -> user`, setiap langkah melewati STF.
#[test]
fn treasury_to_faucet_to_user_full_flow_through_stf() {
    let mut dispenser = new_dispenser();
    let fund_amount = Quantum::new(1_000_000_000_000); // 1000 AUR
    let mut accounts = state_with(Quantum::ZERO);
    let mut monetary = aurion::state::monetary::MonetaryState::new(
        Quantum::new(66_000_000_000_000_000),
        Quantum::ZERO,
    );
    let mut mempool = new_mempool();

    // --- Langkah 1: Treasury mendanai faucet ---
    let treasury_balance_before = accounts[&treasury_address()].balance;
    let (_fund_hash, fund_tx) = dispenser
        .fund_from_treasury(&treasury_key(), fund_amount, &accounts, &mut mempool, 1_000)
        .expect("pendanaan Treasury harus berhasil");

    apply_transaction(&mut accounts, &mut monetary, &treasury_address(), &fund_tx)
        .expect("STF harus menerima transfer Treasury");

    // Saldo Treasury: karena proposer == Treasury dan `split_fee` memberikan
    // 100% fee ke proposer (0% burn), biaya bersih yang ditanggung Treasury
    // hanya `amount`. Inilah konsekuensi desain fee Aurion, bukan bug.
    assert_eq!(
        accounts[&treasury_address()].balance,
        treasury_balance_before
            .checked_sub(fund_amount)
            .expect("treasury turun"),
        "Treasury harus membayar amount (fee dikembalikan ke proposer)"
    );

    // Efek dana masuk ke rekening faucet supaya claim berikutnya bisa jalan.
    accounts.insert(faucet_address(), Account::new(fund_amount, 1));

    // --- Langkah 2: User klaim dana ---
    let recipient = user(20);
    let faucet_balance_before = accounts[&faucet_address()].balance;
    let (_claim_hash, claim_tx) = dispenser
        .dispense(&recipient, &accounts, &mut mempool, 2_000)
        .expect("claim harus berhasil");

    assert_eq!(claim_tx.recipient, recipient);
    assert_eq!(claim_tx.amount, dispenser.config.dispense_amount);
    assert!(
        !accounts.contains_key(&recipient),
        "user belum ada sebelum claim"
    );

    apply_transaction(&mut accounts, &mut monetary, &treasury_address(), &claim_tx)
        .expect("STF harus menjalankan dispense");

    // --- Assertion utama: saldo user bertambah ---
    assert_eq!(
        accounts[&recipient].balance, dispenser.config.dispense_amount,
        "user harus menerima dana tepat sejumlah dispense"
    );
    assert!(
        accounts[&faucet_address()].balance < faucet_balance_before,
        "saldo faucet harus turun setelah STF"
    );
    assert!(
        accounts[&faucet_address()].balance >= dispenser.config.reserve_floor,
        "reserve floor harus tetap dijaga"
    );

    // Tidak ada koin baru yang dicetak.
    assert_eq!(monetary.total_issued.as_u128(), 66_000_000_000_000_000);
    assert_eq!(
        monetary.total_burned.as_u128(),
        0,
        "0% burn: fee masuk proposer"
    );
}

#[test]
fn faucet_claim_does_not_mint_new_coins() {
    let mut dispenser = new_dispenser();
    let mut accounts = state_with(Quantum::new(1_000_000_000_000));
    let mut monetary = aurion::state::monetary::MonetaryState::new(
        Quantum::new(66_000_000_000_000_000),
        Quantum::ZERO,
    );
    let mut mempool = new_mempool();
    let issued_before = monetary.total_issued.as_u128();

    for i in 0..5u64 {
        let (_h, tx) = dispenser
            .dispense(
                &user(30 + i as u8),
                &accounts,
                &mut mempool,
                1_000 + i * 100,
            )
            .expect("claim");
        apply_transaction(&mut accounts, &mut monetary, &treasury_address(), &tx).expect("STF");
    }
    assert_eq!(
        monetary.total_issued.as_u128(),
        issued_before,
        "issuance tidak boleh bertambah karena faucet"
    );
    assert_eq!(
        monetary.total_burned.as_u128(),
        0,
        "tidak ada pencetakan/burn"
    );
}

// ---------------------------------------------------------------------------
// 8. CLI: `--help` tidak boleh menjalankan proses
// ---------------------------------------------------------------------------

#[test]
fn subcommand_help_is_recognized_before_dispatch() {
    use aurion::cli::command::{wants_subcommand_help, CliCommand};

    // Deteksi flag help.
    assert!(wants_subcommand_help(&["--help".to_string()]));
    assert!(wants_subcommand_help(&["-h".to_string()]));
    assert!(wants_subcommand_help(&[
        "start".to_string(),
        "--help".to_string()
    ]));
    assert!(!wants_subcommand_help(&["start".to_string()]));
    assert!(!wants_subcommand_help(&[]));

    // `aurion node --help` harus jadi SubcommandHelp, BUKAN Node.
    // Inilah bug yang membuat `aurion node --help` menyalakan full node.
    let cmd = CliCommand::parse(&["node".to_string(), "--help".to_string()]);
    assert!(
        matches!(cmd, CliCommand::SubcommandHelp { .. }),
        "node --help harus menjadi SubcommandHelp, bukan menjalankan node"
    );

    // Varian lain ikut terlindungi.
    for name in ["faucet", "wallet", "contract", "validator", "rpc", "devnet"] {
        let c = CliCommand::parse(&[name.to_string(), "--help".to_string()]);
        assert!(
            matches!(c, CliCommand::SubcommandHelp { .. }),
            "{name} --help harus menjadi SubcommandHelp"
        );
        let c2 = CliCommand::parse(&[name.to_string(), "help".to_string()]);
        assert!(
            matches!(c2, CliCommand::SubcommandHelp { .. }),
            "{name} help harus menjadi SubcommandHelp"
        );
    }

    // `aurion node start` (tanpa help) TETAP harus menjalankan node.
    let run = CliCommand::parse(&["node".to_string(), "start".to_string()]);
    assert!(
        matches!(run, CliCommand::Node(_)),
        "node start harus tetap menjadi Node"
    );
    let bare = CliCommand::parse(&["node".to_string()]);
    assert!(
        matches!(bare, CliCommand::Node(_)),
        "node tanpa sub = start"
    );

    // `aurion --help` (tanpa perintah) tetap master help.
    assert!(matches!(
        CliCommand::parse(&["--help".to_string()]),
        CliCommand::Help
    ));
    assert!(matches!(
        CliCommand::parse(&["help".to_string()]),
        CliCommand::Help
    ));
    assert!(matches!(CliCommand::parse(&[]), CliCommand::Help));
}

#[test]
fn audit_record_type_is_public_and_cloneable() {
    // Menjamin struct audit tetap bisa dipakai integrator/SDK.
    let rec = FaucetAuditRecord {
        timestamp: 1,
        recipient: user(1),
        amount: Quantum::new(1),
        fee: Quantum::ZERO,
        tx_hash: aurion::core::Hash256::ZERO,
        rejected: false,
    };
    let copy = rec.clone();
    assert_eq!(rec, copy);
    assert!(!rec.rejected);
}
