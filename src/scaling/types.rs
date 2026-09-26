//! Tipe Data Primitif Layer-2 (L2) Aurion & Serialisasi Kanonikal.
//! Mematuhi Invariant L2-ARCH-003 (Zero-Float Quantum u128), L2-ARCH-005 (Blake3/Ed25519), dan AUR-ARCH-011 (#![forbid(unsafe_code)]).

use crate::core::{Address, Hash256, Quantum, Signature};
use crate::crypto::{blake3_hash, ed25519_verify_strict, DST_TX};
use crate::l2::codec::L2CodecError;

/// Batas maksimum ukuran payload transaksi L2 (64 KB)
pub const MAX_L2_TX_PAYLOAD_SIZE: usize = 64 * 1024;

/// Ukuran basis transaksi L2 sebelum payload: 32 + 32 + 16 + 16 + 8 + 64 + 4 = 172 byte
pub const L2_TX_BASE_SIZE: usize = 172;

/// Ukuran tetap header blok L2: 8 + 32 + 32 + 32 + 8 = 112 byte
pub const L2_BLOCK_HEADER_SIZE: usize = 112;

/// Ukuran tetap struk eksekusi L2 (Receipt): 32 + 1 + 8 + 16 = 57 byte
pub const L2_RECEIPT_SIZE: usize = 57;

/// Identitas Transaksi L2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2Transaction {
    pub sender: Address,
    pub recipient: Address,
    pub amount: Quantum,
    pub fee: Quantum,
    pub nonce: u64,
    pub signature: Signature,
    pub payload: Vec<u8>,
}

impl L2Transaction {
    /// Membuat transaksi L2 baru
    #[must_use]
    pub fn new(
        sender: Address,
        recipient: Address,
        amount: Quantum,
        fee: Quantum,
        nonce: u64,
        signature: Signature,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            sender,
            recipient,
            amount,
            fee,
            nonce,
            signature,
            payload,
        }
    }

    /// Menghitung hash unik transaksi L2 berbasis Blake3
    #[must_use]
    pub fn compute_hash(&self) -> Hash256 {
        let mut data = Vec::with_capacity(128 + self.payload.len());
        data.extend_from_slice(self.sender.as_bytes());
        data.extend_from_slice(self.recipient.as_bytes());
        data.extend_from_slice(&self.amount.as_u128().to_be_bytes());
        data.extend_from_slice(&self.fee.as_u128().to_be_bytes());
        data.extend_from_slice(&self.nonce.to_be_bytes());
        data.extend_from_slice(self.signature.as_bytes());
        data.extend_from_slice(&self.payload);
        blake3_hash(&data)
    }

    /// Konstruksi data signing preimage yang ditandatangani oleh pengirim
    #[must_use]
    pub fn signing_preimage(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(16 + 32 + 32 + 16 + 16 + 8 + self.payload.len());
        data.extend_from_slice(DST_TX.as_bytes());
        data.push(0x02); // Identifier L2
        data.extend_from_slice(self.sender.as_bytes());
        data.extend_from_slice(self.recipient.as_bytes());
        data.extend_from_slice(&self.amount.as_u128().to_be_bytes());
        data.extend_from_slice(&self.fee.as_u128().to_be_bytes());
        data.extend_from_slice(&self.nonce.to_be_bytes());
        data.extend_from_slice(&self.payload);
        data
    }

    /// Verifikasi tanda tangan digital Ed25519 kanonikal
    #[must_use]
    pub fn verify_signature(&self, sender_pubkey: &[u8; 32]) -> bool {
        let preimage = self.signing_preimage();
        ed25519_verify_strict(sender_pubkey, &preimage, &self.signature).is_ok()
    }

    /// Mengodekan transaksi L2 ke dalam format biner kanonikal Big-Endian
    #[must_use]
    pub fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(L2_TX_BASE_SIZE + self.payload.len());
        buf.extend_from_slice(self.sender.as_bytes());
        buf.extend_from_slice(self.recipient.as_bytes());
        buf.extend_from_slice(&self.amount.as_u128().to_be_bytes());
        buf.extend_from_slice(&self.fee.as_u128().to_be_bytes());
        buf.extend_from_slice(&self.nonce.to_be_bytes());
        buf.extend_from_slice(self.signature.as_bytes());
        let payload_len = u32::try_from(self.payload.len()).unwrap_or(u32::MAX);
        buf.extend_from_slice(&payload_len.to_be_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Mendekode byte biner kanonikal menjadi L2Transaction
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, L2CodecError> {
        if bytes.len() < L2_TX_BASE_SIZE {
            return Err(L2CodecError::HeaderTooShort {
                expected: L2_TX_BASE_SIZE,
                actual: bytes.len(),
            });
        }

        let mut sender_bytes = [0u8; 32];
        sender_bytes.copy_from_slice(&bytes[0..32]);
        let sender = Address::from_bytes(sender_bytes);

        let mut recipient_bytes = [0u8; 32];
        recipient_bytes.copy_from_slice(&bytes[32..64]);
        let recipient = Address::from_bytes(recipient_bytes);

        let mut amount_bytes = [0u8; 16];
        amount_bytes.copy_from_slice(&bytes[64..80]);
        let amount = Quantum::new(u128::from_be_bytes(amount_bytes));

        let mut fee_bytes = [0u8; 16];
        fee_bytes.copy_from_slice(&bytes[80..96]);
        let fee = Quantum::new(u128::from_be_bytes(fee_bytes));

        let mut nonce_bytes = [0u8; 8];
        nonce_bytes.copy_from_slice(&bytes[96..104]);
        let nonce = u64::from_be_bytes(nonce_bytes);

        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(&bytes[104..168]);
        let signature = Signature::from_bytes(sig_bytes);

        let mut payload_len_bytes = [0u8; 4];
        payload_len_bytes.copy_from_slice(&bytes[168..172]);
        let payload_len = usize::try_from(u32::from_be_bytes(payload_len_bytes)).unwrap_or(0);

        if payload_len > MAX_L2_TX_PAYLOAD_SIZE {
            return Err(L2CodecError::CorruptedPayload(
                "Ukuran payload melebihi batas 64 KB",
            ));
        }

        if bytes.len() != L2_TX_BASE_SIZE + payload_len {
            return Err(L2CodecError::PayloadLengthMismatch {
                expected: payload_len,
                actual: bytes.len().saturating_sub(L2_TX_BASE_SIZE),
            });
        }

        let payload = bytes[172..172 + payload_len].to_vec();

        Ok(Self {
            sender,
            recipient,
            amount,
            fee,
            nonce,
            signature,
            payload,
        })
    }
}

/// Header Blok Layer-2 (112 Byte)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2BlockHeader {
    pub block_number: u64,
    pub prev_hash: Hash256,
    pub state_root: Hash256,
    pub txs_root: Hash256,
    pub timestamp: u64,
}

impl L2BlockHeader {
    /// Menghitung hash blok L2 berbasis Blake3
    #[must_use]
    pub fn compute_hash(&self) -> Hash256 {
        let raw = self.encode_canonical();
        blake3_hash(&raw)
    }

    /// Mengodekan header blok ke dalam 112 byte kanonikal
    #[must_use]
    pub fn encode_canonical(&self) -> [u8; L2_BLOCK_HEADER_SIZE] {
        let mut buf = [0u8; L2_BLOCK_HEADER_SIZE];
        buf[0..8].copy_from_slice(&self.block_number.to_be_bytes());
        buf[8..40].copy_from_slice(self.prev_hash.as_bytes());
        buf[40..72].copy_from_slice(self.state_root.as_bytes());
        buf[72..104].copy_from_slice(self.txs_root.as_bytes());
        buf[104..112].copy_from_slice(&self.timestamp.to_be_bytes());
        buf
    }

    /// Mendekode 112 byte biner menjadi L2BlockHeader
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, L2CodecError> {
        if bytes.len() < L2_BLOCK_HEADER_SIZE {
            return Err(L2CodecError::HeaderTooShort {
                expected: L2_BLOCK_HEADER_SIZE,
                actual: bytes.len(),
            });
        }

        let mut block_num_bytes = [0u8; 8];
        block_num_bytes.copy_from_slice(&bytes[0..8]);
        let block_number = u64::from_be_bytes(block_num_bytes);

        let mut prev_h_bytes = [0u8; 32];
        prev_h_bytes.copy_from_slice(&bytes[8..40]);
        let prev_hash = Hash256::from_bytes(prev_h_bytes);

        let mut state_r_bytes = [0u8; 32];
        state_r_bytes.copy_from_slice(&bytes[40..72]);
        let state_root = Hash256::from_bytes(state_r_bytes);

        let mut txs_r_bytes = [0u8; 32];
        txs_r_bytes.copy_from_slice(&bytes[72..104]);
        let txs_root = Hash256::from_bytes(txs_r_bytes);

        let mut ts_bytes = [0u8; 8];
        ts_bytes.copy_from_slice(&bytes[104..112]);
        let timestamp = u64::from_be_bytes(ts_bytes);

        Ok(Self {
            block_number,
            prev_hash,
            state_root,
            txs_root,
            timestamp,
        })
    }
}

/// Menghitung Merkle root transaksi L2 menggunakan Blake3
#[must_use]
pub fn compute_txs_root(txs: &[L2Transaction]) -> Hash256 {
    if txs.is_empty() {
        return Hash256::ZERO;
    }

    let mut current_level: Vec<Hash256> = txs.iter().map(L2Transaction::compute_hash).collect();

    while current_level.len() > 1 {
        let mut next_level = Vec::with_capacity(current_level.len().div_ceil(2));
        for chunk in current_level.chunks(2) {
            let left = &chunk[0];
            let right = if chunk.len() > 1 {
                &chunk[1]
            } else {
                &chunk[0]
            };
            let mut combined = [0u8; 64];
            combined[0..32].copy_from_slice(left.as_bytes());
            combined[32..64].copy_from_slice(right.as_bytes());
            next_level.push(blake3_hash(&combined));
        }
        current_level = next_level;
    }

    current_level[0]
}

/// Blok Penuh Layer-2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2Block {
    pub header: L2BlockHeader,
    pub transactions: Vec<L2Transaction>,
}

impl L2Block {
    /// Membuat blok L2 baru dan secara otomatis menghitung txs_root
    #[must_use]
    pub fn new(
        block_number: u64,
        prev_hash: Hash256,
        state_root: Hash256,
        timestamp: u64,
        transactions: Vec<L2Transaction>,
    ) -> Self {
        let txs_root = compute_txs_root(&transactions);
        let header = L2BlockHeader {
            block_number,
            prev_hash,
            state_root,
            txs_root,
            timestamp,
        };
        Self {
            header,
            transactions,
        }
    }

    /// Mengodekan blok penuh ke format biner kanonikal
    #[must_use]
    pub fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(
            L2_BLOCK_HEADER_SIZE + 4 + self.transactions.len() * L2_TX_BASE_SIZE,
        );
        buf.extend_from_slice(&self.header.encode_canonical());
        let tx_count = u32::try_from(self.transactions.len()).unwrap_or(u32::MAX);
        buf.extend_from_slice(&tx_count.to_be_bytes());
        for tx in &self.transactions {
            let raw_tx = tx.encode_canonical();
            let tx_len = u32::try_from(raw_tx.len()).unwrap_or(u32::MAX);
            buf.extend_from_slice(&tx_len.to_be_bytes());
            buf.extend_from_slice(&raw_tx);
        }
        buf
    }

    /// Mendekode biner kanonikal menjadi L2Block
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, L2CodecError> {
        if bytes.len() < L2_BLOCK_HEADER_SIZE + 4 {
            return Err(L2CodecError::HeaderTooShort {
                expected: L2_BLOCK_HEADER_SIZE + 4,
                actual: bytes.len(),
            });
        }

        let header = L2BlockHeader::decode_canonical(&bytes[0..L2_BLOCK_HEADER_SIZE])?;

        let mut tx_cnt_bytes = [0u8; 4];
        tx_cnt_bytes.copy_from_slice(&bytes[L2_BLOCK_HEADER_SIZE..L2_BLOCK_HEADER_SIZE + 4]);
        let tx_count = usize::try_from(u32::from_be_bytes(tx_cnt_bytes)).unwrap_or(0);

        let mut transactions = Vec::with_capacity(tx_count);
        let mut cursor = L2_BLOCK_HEADER_SIZE + 4;

        for _ in 0..tx_count {
            if cursor + 4 > bytes.len() {
                return Err(L2CodecError::CorruptedPayload(
                    "Calldata terpotong saat membaca panjang tx",
                ));
            }
            let mut tx_len_bytes = [0u8; 4];
            tx_len_bytes.copy_from_slice(&bytes[cursor..cursor + 4]);
            let tx_len = usize::try_from(u32::from_be_bytes(tx_len_bytes)).unwrap_or(0);
            cursor += 4;

            if cursor + tx_len > bytes.len() {
                return Err(L2CodecError::CorruptedPayload(
                    "Calldata terpotong saat membaca isi tx",
                ));
            }

            let tx = L2Transaction::decode_canonical(&bytes[cursor..cursor + tx_len])?;
            transactions.push(tx);
            cursor += tx_len;
        }

        if cursor != bytes.len() {
            return Err(L2CodecError::PayloadLengthMismatch {
                expected: cursor,
                actual: bytes.len(),
            });
        }

        Ok(Self {
            header,
            transactions,
        })
    }
}

/// Paket Batch Rollup L2 untuk Diposting ke Layer-1
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2Batch {
    pub batch_index: u64,
    pub prev_state_root: Hash256,
    pub new_state_root: Hash256,
    pub start_block: u64,
    pub end_block: u64,
    pub transactions_calldata: Vec<u8>,
}

impl L2Batch {
    /// Menghitung komitmen hash batch untuk verifikasi di L1
    #[must_use]
    pub fn compute_batch_hash(&self) -> Hash256 {
        let mut data = Vec::with_capacity(8 + 32 + 32 + 8 + 8 + self.transactions_calldata.len());
        data.extend_from_slice(&self.batch_index.to_be_bytes());
        data.extend_from_slice(self.prev_state_root.as_bytes());
        data.extend_from_slice(self.new_state_root.as_bytes());
        data.extend_from_slice(&self.start_block.to_be_bytes());
        data.extend_from_slice(&self.end_block.to_be_bytes());
        data.extend_from_slice(&self.transactions_calldata);
        blake3_hash(&data)
    }
}

/// Bukti Eksekusi Transaksi L2 (Receipt, 57 Byte)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2Receipt {
    pub tx_hash: Hash256,
    pub success: bool,
    pub gas_used: u64,
    pub fee_paid: Quantum,
}

impl L2Receipt {
    /// Mengodekan receipt ke dalam 57 byte kanonikal
    #[must_use]
    pub fn encode_canonical(&self) -> [u8; L2_RECEIPT_SIZE] {
        let mut buf = [0u8; L2_RECEIPT_SIZE];
        buf[0..32].copy_from_slice(self.tx_hash.as_bytes());
        buf[32] = u8::from(self.success);
        buf[33..41].copy_from_slice(&self.gas_used.to_be_bytes());
        buf[41..57].copy_from_slice(&self.fee_paid.as_u128().to_be_bytes());
        buf
    }

    /// Mendekode 57 byte kanonikal menjadi L2Receipt
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, L2CodecError> {
        if bytes.len() < L2_RECEIPT_SIZE {
            return Err(L2CodecError::HeaderTooShort {
                expected: L2_RECEIPT_SIZE,
                actual: bytes.len(),
            });
        }

        let mut tx_h_bytes = [0u8; 32];
        tx_h_bytes.copy_from_slice(&bytes[0..32]);
        let tx_hash = Hash256::from_bytes(tx_h_bytes);

        let success = bytes[32] != 0;

        let mut gas_bytes = [0u8; 8];
        gas_bytes.copy_from_slice(&bytes[33..41]);
        let gas_used = u64::from_be_bytes(gas_bytes);

        let mut fee_bytes = [0u8; 16];
        fee_bytes.copy_from_slice(&bytes[41..57]);
        let fee_paid = Quantum::new(u128::from_be_bytes(fee_bytes));

        Ok(Self {
            tx_hash,
            success,
            gas_used,
            fee_paid,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::Keypair;

    #[test]
    fn test_l2_transaction_canonical_roundtrip() {
        let sender = Address::from_bytes([1u8; 32]);
        let recipient = Address::from_bytes([2u8; 32]);
        let tx = L2Transaction {
            sender,
            recipient,
            amount: Quantum::new(500_000_000),
            fee: Quantum::new(100_000),
            nonce: 7,
            signature: Signature::from_bytes([0x77; 64]),
            payload: vec![0xCA, 0xFE, 0xBA, 0xBE],
        };

        let encoded = tx.encode_canonical();
        assert_eq!(encoded.len(), L2_TX_BASE_SIZE + 4);

        let decoded =
            L2Transaction::decode_canonical(&encoded).expect("Decode L2Tx harus berhasil");
        assert_eq!(decoded, tx);
    }

    #[test]
    fn test_l2_transaction_ed25519_verification() {
        let keypair = Keypair::from_seed(&[42u8; 32]);
        let sender_pubkey = keypair.public_key_bytes();
        let sender = Address::from_bytes(sender_pubkey);
        let recipient = Address::from_bytes([9u8; 32]);

        let mut tx = L2Transaction {
            sender,
            recipient,
            amount: Quantum::new(1_000_000_000),
            fee: Quantum::new(50_000),
            nonce: 1,
            signature: Signature::ZERO,
            payload: vec![1, 2, 3],
        };

        // Tanda tangani preimage
        let preimage = tx.signing_preimage();
        let sig = keypair.sign(&preimage);
        tx.signature = sig;

        // Verifikasi sah
        assert!(tx.verify_signature(&sender_pubkey));

        // Verifikasi gagal dengan kunci salah
        let wrong_pubkey = [0xFF; 32];
        assert!(!tx.verify_signature(&wrong_pubkey));
    }

    #[test]
    fn test_l2_block_header_canonical_roundtrip() {
        let header = L2BlockHeader {
            block_number: 100,
            prev_hash: Hash256::from_bytes([0xAA; 32]),
            state_root: Hash256::from_bytes([0xBB; 32]),
            txs_root: Hash256::from_bytes([0xCC; 32]),
            timestamp: 1700000000,
        };

        let encoded = header.encode_canonical();
        assert_eq!(encoded.len(), L2_BLOCK_HEADER_SIZE);

        let decoded =
            L2BlockHeader::decode_canonical(&encoded).expect("Decode header harus berhasil");
        assert_eq!(decoded, header);
        assert_eq!(decoded.compute_hash(), header.compute_hash());
    }

    #[test]
    fn test_l2_block_roundtrip_with_txs() {
        let tx = L2Transaction {
            sender: Address::from_bytes([1u8; 32]),
            recipient: Address::from_bytes([2u8; 32]),
            amount: Quantum::new(100),
            fee: Quantum::new(10),
            nonce: 0,
            signature: Signature::ZERO,
            payload: vec![],
        };

        let block = L2Block::new(
            1,
            Hash256::ZERO,
            Hash256::from_bytes([0x55; 32]),
            1700000001,
            vec![tx.clone()],
        );

        let encoded = block.encode_canonical();
        let decoded = L2Block::decode_canonical(&encoded).expect("Decode block harus berhasil");
        assert_eq!(decoded, block);
        assert_eq!(decoded.transactions.len(), 1);
        assert_eq!(decoded.header.txs_root, compute_txs_root(&[tx]));
    }

    #[test]
    fn test_l2_receipt_canonical_roundtrip() {
        let receipt = L2Receipt {
            tx_hash: Hash256::from_bytes([0x42; 32]),
            success: true,
            gas_used: 21_000,
            fee_paid: Quantum::new(50_000),
        };

        let encoded = receipt.encode_canonical();
        assert_eq!(encoded.len(), L2_RECEIPT_SIZE);

        let decoded = L2Receipt::decode_canonical(&encoded).expect("Decode receipt harus berhasil");
        assert_eq!(decoded, receipt);
    }

    #[test]
    fn test_compute_txs_root_empty_and_multiple() {
        assert_eq!(compute_txs_root(&[]), Hash256::ZERO);

        let tx1 = L2Transaction {
            sender: Address::from_bytes([1u8; 32]),
            recipient: Address::from_bytes([2u8; 32]),
            amount: Quantum::new(100),
            fee: Quantum::new(10),
            nonce: 0,
            signature: Signature::ZERO,
            payload: vec![],
        };
        let root1 = compute_txs_root(std::slice::from_ref(&tx1));
        assert_eq!(root1, tx1.compute_hash());

        let tx2 = L2Transaction {
            sender: Address::from_bytes([3u8; 32]),
            recipient: Address::from_bytes([4u8; 32]),
            amount: Quantum::new(200),
            fee: Quantum::new(20),
            nonce: 1,
            signature: Signature::ZERO,
            payload: vec![],
        };
        let root2 = compute_txs_root(&[tx1, tx2]);
        assert_ne!(root2, Hash256::ZERO);
    }
}
