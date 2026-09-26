//! Pengujian Integrasi Suite Dompet Klien (Client Wallet Subsystem) Aurion.
//! Mematuhi Dokumen 01 (01-WALLET-RULES.md) dan Invariant AUR-ARCH-011 / 012.

#![forbid(unsafe_code)]

use aurion::core::{Quantum, Signature};
use aurion::crypto::{derive_address_from_pubkey, ed25519_verify_strict, encode_address_bech32m};
use aurion::wallet::bip39::{entropy_to_mnemonic_24, mnemonic_to_entropy_24, mnemonic_to_seed};
use aurion::wallet::derivation::{DerivedAccount, AURION_COIN_TYPE};
use aurion::wallet::keystore::{Keystore, KeystoreError};
use aurion::wallet::signing::{ClearSigningDetails, SigningError};
use ed25519_dalek::SigningKey;

#[test]
fn test_bip39_24_words_mnemonic_roundtrip() {
    let entropy = [0xabu8; 32];
    let mnemonic = entropy_to_mnemonic_24(&entropy);

    let words: Vec<&str> = mnemonic.split_whitespace().collect();
    assert_eq!(words.len(), 24);

    let recovered = mnemonic_to_entropy_24(&mnemonic).expect("Harus berhasil dipulihkan");
    assert_eq!(entropy, recovered);
}

#[test]
fn test_bip39_invalid_word_count_and_checksum_rejection() {
    // 1. Kurang dari 24 kata
    let invalid_short = "abandon ability able about";
    assert!(mnemonic_to_entropy_24(invalid_short).is_err());

    // 2. 24 kata tetapi checksum salah
    let entropy = [0x55u8; 32];
    let mnemonic = entropy_to_mnemonic_24(&entropy);
    let mut words: Vec<&str> = mnemonic.split_whitespace().collect();
    words[23] = "zoo"; // Manipulasi kata terakhir
    let tampered = words.join(" ");

    assert!(mnemonic_to_entropy_24(&tampered).is_err());
}

#[test]
fn test_hierarchical_derivation_path_aurion_standard() {
    let entropy = [0x33u8; 32];
    let mnemonic = entropy_to_mnemonic_24(&entropy);
    let master_seed = mnemonic_to_seed(&mnemonic, "OptionalPassphrase");

    let acc0 = DerivedAccount::derive_account(&master_seed, 0, 0);
    assert_eq!(acc0.path, format!("m/44'/{AURION_COIN_TYPE}'/0'/0'/0'"));
    assert!(acc0.bech32m_address.starts_with("aur1"));
    assert_eq!(acc0.bech32m_address.len(), 62);

    let acc1 = DerivedAccount::derive_account(&master_seed, 0, 1);
    assert_eq!(acc1.path, format!("m/44'/{AURION_COIN_TYPE}'/0'/0'/1'"));
    assert_ne!(acc0.address, acc1.address);
    assert_ne!(acc0.bech32m_address, acc1.bech32m_address);

    let acc_different_account = DerivedAccount::derive_account(&master_seed, 1, 0);
    assert_ne!(acc0.address, acc_different_account.address);
}

#[test]
fn test_keystore_password_encryption_and_tamper_protection() {
    let signing_key = SigningKey::from_bytes(&[0x88u8; 32]);
    let address = derive_address_from_pubkey(signing_key.verifying_key().as_bytes());
    let bech32m = encode_address_bech32m(&address, "aur").unwrap();
    let password = "MySecretMasterPassword!2026";

    let keystore = Keystore::encrypt(&signing_key, password, &bech32m)
        .expect("Enkripsi keystore V2 harus berhasil");
    let json_output = keystore.to_json_string();

    let loaded = Keystore::from_json_str(&json_output).expect("Parsing json harus berhasil");
    assert_eq!(loaded.address, bech32m);

    // 1. Dekripsi sukses dengan password yang benar
    let decrypted_key = loaded.decrypt(password).expect("Dekripsi harus berhasil");
    assert_eq!(decrypted_key.to_bytes(), signing_key.to_bytes());

    // 2. Dekripsi gagal jika password salah
    let err = loaded.decrypt("WrongPassword!").unwrap_err();
    assert_eq!(err, KeystoreError::InvalidPassword);

    // 3. Modifikasi ciphertext harus gagal verifikasi MAC
    let mut tampered_keystore = loaded.clone();
    let mut bad_ciphertext = tampered_keystore.crypto.ciphertext;
    // Ubah 2 karakter pertama
    bad_ciphertext.replace_range(0..2, "ff");
    tampered_keystore.crypto.ciphertext = bad_ciphertext;
    let tamper_err = tampered_keystore.decrypt(password).unwrap_err();
    assert_eq!(tamper_err, KeystoreError::InvalidPassword);
}

#[test]
fn test_clear_signing_mandate_fee_split_and_validation() {
    let key_sender = SigningKey::from_bytes(&[0x11u8; 32]);
    let addr_sender = derive_address_from_pubkey(key_sender.verifying_key().as_bytes());
    let sender_bech = encode_address_bech32m(&addr_sender, "aur").unwrap();

    let key_recip = SigningKey::from_bytes(&[0x22u8; 32]);
    let addr_recip = derive_address_from_pubkey(key_recip.verifying_key().as_bytes());
    let recip_bech = encode_address_bech32m(&addr_recip, "aur").unwrap();

    // 1. Validasi sukses
    let details = ClearSigningDetails::new(
        &sender_bech,
        &recip_bech,
        1_000_000_000, // 10 AUR
        50_000,        // 0.0005 AUR
        42,
        "Payment for infrastructure hosting",
    )
    .expect("Validasi transaksi harus berhasil");

    // Kanonik AUR-MON-003: 0% burn, 100% ke validator BFT.
    // Skema lama "20% burn / 80% miner" telah dicabut (lihat AUD-BFT-001).
    let (burn, validator) = details.fee_split();
    assert_eq!(burn, Quantum::ZERO, "0% burn adalah kanonik Aurion");
    assert_eq!(validator, Quantum::new(50_000), "100% fee ke validator BFT");
    assert_eq!(
        burn.checked_add(validator).unwrap(),
        Quantum::new(50_000),
        "konservasi fee harus terjaga"
    );

    // Clear signing prompt format
    let prompt = details.format_clear_signing_prompt();
    assert!(prompt.contains("AURION CLEAR SIGNING VERIFICATION PROMPT"));
    assert!(prompt.contains(&sender_bech));
    assert!(prompt.contains(&recip_bech));
    assert!(prompt.contains("10.00000000 AUR"));
    // Prompt harus jujur: tidak boleh menampilkan skema yang sudah dicabut.
    assert!(prompt.contains("BFT Validator Reward (100%):   50000 Quantum"));
    assert!(prompt.contains("Protocol Burn (0%):           0 Quantum"));
    assert!(!prompt.contains("Miner"), "prompt tidak boleh memakai istilah miner");

    // Penandatanganan
    let (tx, raw_hex) = details.sign(&key_sender, 1, 999_999);
    assert!(!raw_hex.is_empty());
    assert_ne!(tx.signature, Signature::ZERO);

    // Verifikasi keabsahan signature terhadap preimage kanonikal
    let preimage = tx.signing_preimage();
    let res = ed25519_verify_strict(
        key_sender.verifying_key().as_bytes(),
        &preimage,
        &tx.signature,
    );
    assert!(res.is_ok());

    // 2. Larangan self-transfer
    let self_res = ClearSigningDetails::new(&sender_bech, &sender_bech, 1000, 10_000, 0, "");
    assert_eq!(self_res.unwrap_err(), SigningError::SelfTransferProhibited);

    // 3. Larangan fee di bawah minimum 10.000 Q
    let low_fee = ClearSigningDetails::new(&sender_bech, &recip_bech, 1000, 9_999, 0, "");
    assert_eq!(low_fee.unwrap_err(), SigningError::FeeBelowMinimum(9_999));

    // 4. Larangan amount = 0
    let zero_amount = ClearSigningDetails::new(&sender_bech, &recip_bech, 0, 10_000, 0, "");
    assert_eq!(zero_amount.unwrap_err(), SigningError::ZeroAmount);
}
