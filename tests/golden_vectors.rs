//! Golden byte-level vectors for the canonical transaction schema and signing preimage.
//! Memvalidasi ISSUE-003: schema tunggal, encoding kanonikal tunggal, preimage tunggal.

use aurion::codec::CanonicalEncode;
use aurion::consensus::header::{BlockHeader, BLOCK_HEADER_BYTES};
use aurion::core::{Address, Hash256, Quantum, Signature};
use aurion::crypto::{blake3_hash, Keypair};
use aurion::transaction::types::{Transaction, TRANSACTION_BASE_BYTES};
use aurion::transaction::validator::validate_transaction_stateless;
use aurion::transaction::TxType;

const EXPECTED_SEED: [u8; 32] = [1u8; 32];
const EXPECTED_RECIPIENT: [u8; 32] = [2u8; 32];
const EXPECTED_SENDER_HEX: &str =
    "cb095697ccc5acbf23e176ee4f05e7e77ddfe54236bfe8c8b42e78c3d5a95aad";
const EXPECTED_CANONICAL_HEX: &str = "0001000003e90100cb095697ccc5acbf23e176ee4f05e7e77ddfe54236bfe8c8b42e78c3d5a95aad020202020202020202020202020202020202020202020202020202020202020200000000000000000000000000000000000000001dcd65000000000000000000000000000098968000000000000003e80000000077506b9564af9209c9f67b12874cb13b760cc19137143f211b9f3c3d127b0b889a20edd754de317da90631a755f41c8dc83fe3a27b415ed0397b73a4d87f0c09";
const EXPECTED_PREIMAGE_HEX: &str = "415552494f4e2d54582d5631000001000003e90100cb095697ccc5acbf23e176ee4f05e7e77ddfe54236bfe8c8b42e78c3d5a95aad020202020202020202020202020202020202020202020202020202020202020200000000000000000000000000000000000000001dcd65000000000000000000000000000098968000000000000003e800000000";
const EXPECTED_PREIMAGE_HASH: &str =
    "807fa6f843afc7e23cbc927aaf2d86556d05ca9690a304d8e2c1a293430fb7ff";
const EXPECTED_SIGNATURE_HEX: &str = "77506b9564af9209c9f67b12874cb13b760cc19137143f211b9f3c3d127b0b889a20edd754de317da90631a755f41c8dc83fe3a27b415ed0397b73a4d87f0c09";
const EXPECTED_TXID_HEX: &str = "9da0583445cf37016058bb104b83e80626e3bcf6c92629eada1ea7fcc8e6e2f9";
const EXPECTED_HEADER_HEX: &str = "00000001000000000000000100000000000000000000000069b6880c00000000000000000000000000000000000000000000000000000000000000009da0583445cf37016058bb104b83e80626e3bcf6c92629eada1ea7fcc8e6e2f9af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";
const EXPECTED_BLOCKHASH_HEX: &str =
    "eed74fb70c5cf061b51688acc25f7f08d3445d08e4b40a0c24eccefa56024701";

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn canonical_transaction() -> Transaction {
    let keypair = Keypair::from_seed(&EXPECTED_SEED);
    let mut tx = Transaction {
        version: 1,
        chain_id: 1001,
        tx_type: TxType::Transfer,
        flags: 0,
        sender: keypair.derive_address(),
        recipient: Address::from_bytes(EXPECTED_RECIPIENT),
        nonce: 0,
        amount: Quantum::new(500_000_000),
        fee: Quantum::new(10_000_000),
        valid_until: 1000,
        payload: vec![],
        signature: Signature::from_bytes([0u8; 64]),
    };
    let preimage = tx.signing_preimage();
    tx.signature = keypair.sign(&preimage);
    tx
}

#[test]
fn golden_transaction_canonical_bytes_and_signing_identity() {
    let keypair = Keypair::from_seed(&EXPECTED_SEED);
    let tx = canonical_transaction();

    assert_eq!(tx.sender.to_hex(), EXPECTED_SENDER_HEX);
    assert_eq!(tx.chain_id, 1001);

    let canonical = tx.to_canonical_bytes();
    assert_eq!(canonical.len(), TRANSACTION_BASE_BYTES + 4);
    assert_eq!(hex(&canonical), EXPECTED_CANONICAL_HEX);

    let preimage = tx.signing_preimage();
    assert_eq!(hex(&preimage), EXPECTED_PREIMAGE_HEX);
    assert_eq!(blake3_hash(&preimage).to_hex(), EXPECTED_PREIMAGE_HASH);

    assert_eq!(tx.signature.to_hex(), EXPECTED_SIGNATURE_HEX);
    assert_eq!(tx.compute_tx_id().to_hex(), EXPECTED_TXID_HEX);

    assert!(validate_transaction_stateless(&tx, &keypair.public_key_bytes()).is_ok());
}

#[test]
fn golden_transaction_preimage_is_stable_across_wallet_and_validator() {
    let tx = canonical_transaction();
    let preimage = tx.signing_preimage();

    assert!(preimage.starts_with(b"AURION-TX-V1\x00"));
    assert_eq!(
        preimage.len(),
        12 + 1 + 2 + 4 + 1 + 1 + 32 + 32 + 8 + 16 + 16 + 8 + 4
    );
}

#[test]
fn golden_block_header_bytes_and_block_hash() {
    let tx = canonical_transaction();
    let header = BlockHeader {
        version: 1,
        height: 1,
        round: 0,
        timestamp: 1_773_570_060,
        prev_block_hash: Hash256::ZERO,
        tx_merkle_root: tx.compute_tx_id(),
        state_root: blake3_hash(b""),
    };

    let bytes = header.to_canonical_bytes();
    assert_eq!(bytes.len(), BLOCK_HEADER_BYTES);
    assert_eq!(hex(&bytes), EXPECTED_HEADER_HEX);
    assert_eq!(header.compute_block_hash().to_hex(), EXPECTED_BLOCKHASH_HEX);
}
