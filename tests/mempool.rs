//! Pengujian Integrasi Mempool Engine, Mandat RBF, dan Tanda Terima Transaksi.
//! Memvalidasi Kepatuhan Dokumen 03 (03-TRANSACTION-LIFECYCLE.md).

use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::crypto::Keypair;
use aurion::mempool::{
    MempoolEngine, MempoolError, TransactionReceipt, TransactionState,
};
use aurion::state::account::Account;
use aurion::transaction::types::{Transaction, TxType};

fn create_signed_tx(
    keypair: &Keypair,
    nonce: u64,
    amount: Quantum,
    fee: Quantum,
) -> (Transaction, [u8; 32]) {
    let sender = keypair.derive_address();
    let recipient = Address([0x02; 32]);
    let mut tx = Transaction {
        version: 1,
        chain_id: 1,
        tx_type: TxType::Transfer,
        flags: 0,
        sender,
        recipient,
        nonce,
        amount,
        fee,
        valid_until: 1000,
        payload: Vec::new(),
        signature: Signature([0u8; 64]),
    };

    let preimage = tx.signing_preimage();
    tx.signature = keypair.sign(&preimage);
    (tx, keypair.public_key_bytes())
}

#[test]
fn test_mempool_admission_success() {
    let keypair = Keypair::from_seed(&[50u8; 32]);
    let account = Account::new(Quantum::new(1_000_000_000), 0);
    let mut mempool = MempoolEngine::new(100, 3600);

    let (tx, pubkey) = create_signed_tx(&keypair, 0, Quantum::new(100_000_000), Quantum::new(10_000));
    let res = mempool.submit_transaction(tx, &pubkey, 1000, &account);
    assert!(res.is_ok(), "Transaksi sah dengan saldo cukup wajib diterima");
    assert_eq!(mempool.len(), 1);
}

#[test]
fn test_mempool_rejection_insufficient_balance() {
    let keypair = Keypair::from_seed(&[51u8; 32]);
    // Saldo hanya 500, butuh 1000 + 10 = 1010
    let account = Account::new(Quantum::new(500), 0);
    let mut mempool = MempoolEngine::new(100, 3600);

    let (tx, pubkey) = create_signed_tx(&keypair, 0, Quantum::new(1_000), Quantum::new(10));
    let res = mempool.submit_transaction(tx, &pubkey, 1000, &account);
    assert!(matches!(res, Err(MempoolError::InsufficientBalance { .. })));
    assert_eq!(mempool.len(), 0);
}

#[test]
fn test_mempool_rbf_mandate_enforcement() {
    let keypair = Keypair::from_seed(&[52u8; 32]);
    let account = Account::new(Quantum::new(5_000_000_000), 0);
    let mut mempool = MempoolEngine::new(100, 3600);

    // 1. Kirim transaksi awal dengan fee = 100_000 Quanta
    let (tx1, pubkey) = create_signed_tx(&keypair, 0, Quantum::new(10_000_000), Quantum::new(100_000));
    let tx1_id = mempool.submit_transaction(tx1, &pubkey, 1000, &account).unwrap();
    assert_eq!(mempool.len(), 1);

    // 2. Coba ganti dengan nonce sama tapi kenaikan fee hanya +5% (105_000) -> WAJIB DITOLAK
    let (tx2_fail, _) = create_signed_tx(&keypair, 0, Quantum::new(10_000_000), Quantum::new(105_000));
    let res_fail = mempool.submit_transaction(tx2_fail, &pubkey, 1010, &account);
    assert!(matches!(
        res_fail,
        Err(MempoolError::InsufficientFeeForReplacement { .. })
    ));
    assert_eq!(mempool.len(), 1);

    // 3. Coba ganti dengan nonce sama dan kenaikan fee +10% (110_000) -> WAJIB DITERIMA
    let (tx2_ok, _) = create_signed_tx(&keypair, 0, Quantum::new(10_000_000), Quantum::new(110_000));
    let tx2_id = mempool.submit_transaction(tx2_ok, &pubkey, 1020, &account).unwrap();
    assert_eq!(mempool.len(), 1);
    assert_ne!(tx1_id, tx2_id);
    assert!(mempool.entries.contains_key(&tx2_id));
    assert!(!mempool.entries.contains_key(&tx1_id));
}

#[test]
fn test_mempool_capacity_eviction_anti_dos() {
    let mut mempool = MempoolEngine::new(2, 3600); // Kapasitas maks hanya 2 transaksi
    let account = Account::new(Quantum::new(10_000_000_000), 0);

    let k1 = Keypair::from_seed(&[61u8; 32]);
    let k2 = Keypair::from_seed(&[62u8; 32]);
    let k3 = Keypair::from_seed(&[63u8; 32]);

    let (tx1, p1) = create_signed_tx(&k1, 0, Quantum::new(100), Quantum::new(10_000));
    let (tx2, p2) = create_signed_tx(&k2, 0, Quantum::new(100), Quantum::new(20_000));
    mempool.submit_transaction(tx1, &p1, 1000, &account).unwrap();
    mempool.submit_transaction(tx2, &p2, 1000, &account).unwrap();
    assert_eq!(mempool.len(), 2);

    // Transaksi ke-3 dengan fee 50_000 (lebih tinggi dari tx1=10_000) -> tx1 tergusur!
    let (tx3, p3) = create_signed_tx(&k3, 0, Quantum::new(100), Quantum::new(50_000));
    let tx3_id = mempool.submit_transaction(tx3, &p3, 1000, &account).unwrap();
    assert_eq!(mempool.len(), 2);
    assert!(mempool.entries.contains_key(&tx3_id));
}

#[test]
fn test_transaction_receipt_canonical_split() {
    let keypair = Keypair::from_seed(&[70u8; 32]);
    let (tx, _) = create_signed_tx(&keypair, 0, Quantum::new(1_000_000), Quantum::new(10_000));

    let block_hash = Hash256([0xAA; 32]);
    let receipt = TransactionReceipt::from_finalized_tx(&tx, 1042, block_hash, 0).unwrap();

    assert_eq!(receipt.tx_id, tx.compute_tx_id());
    assert_eq!(receipt.fee_quanta, Quantum::new(10_000));
    // Kanonik AUR-MON-003: 0% burn, 100% ke validator BFT.
    assert_eq!(receipt.fee_burned_quanta, Quantum::ZERO);
    assert_eq!(receipt.fee_validator_quanta, Quantum::new(10_000));
    assert_eq!(receipt.status, TransactionState::Finalized);
}
