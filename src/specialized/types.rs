//! Tipe Data Primitif Layer-3 (L3) Specialized Networks & Serialisasi Kanonikal.
//! Mematuhi Invariant AUR-ARCH-011 (#![forbid(unsafe_code)]), AUR-ARCH-012 (Zero-Float Quantum u128),
//! AUR-L3-ARCH-002 (Monetary Conservation), dan AUR-L3-ARCH-003 (Zero Float Mandate).

use crate::core::{Address, Hash256, Quantum, Signature};
use crate::crypto::{blake3_hash, ed25519_verify_strict};
use std::fmt;

/// Batas maksimum payload transaksi L3 (64 KB)
pub const MAX_L3_TX_PAYLOAD_SIZE: usize = 64 * 1024;

/// Batas maksimum proof data checkpoint L3 (128 KB)
pub const MAX_L3_PROOF_SIZE: usize = 128 * 1024;

/// Ukuran basis transaksi L3 sebelum payload: 32 + 32 + 32 + 16 + 16 + 8 + 64 + 4 = 204 byte
pub const L3_TX_BASE_SIZE: usize = 204;

/// Ukuran header blok L3: 32 + 8 + 32 + 32 + 32 + 32 + 8 + 4 = 148 byte
pub const L3_BLOCK_HEADER_SIZE: usize = 148;

/// Ukuran basis struk L3 (receipt): 32 + 1 + 8 + 16 + 4 + 4 = 65 byte
pub const L3_RECEIPT_BASE_SIZE: usize = 65;

/// Ukuran basis checkpoint L3 sebelum proof_data: 32 + 8 + 8 + 8 + 32 + 32 + 8 + 64 + 4 = 196 byte
pub const L3_CHECKPOINT_BASE_SIZE: usize = 196;

/// Domain Separation Tag untuk transaksi L3
pub const DST_L3_TX: &str = "AURION-L3-TX-V1";

/// Domain Separation Tag untuk checkpoint L3
pub const DST_L3_CHECKPOINT: &str = "AURION-L3-CHECKPOINT-V1";

/// Kesalahan Serialisasi / Deserialisasi Codec L3
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum L3CodecError {
    UnexpectedEof { expected: usize, found: usize },
    PayloadTooLarge { max: usize, actual: usize },
    InvalidSecurityModel(u8),
    InvalidFormat(&'static str),
}

impl fmt::Display for L3CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof { expected, found } => {
                write!(f, "Data biner L3 tidak lengkap: butuh {expected} byte, hanya ada {found} byte")
            }
            Self::PayloadTooLarge { max, actual } => {
                write!(f, "Payload L3 melebihi batas maksimum {max} byte: aktual {actual} byte")
            }
            Self::InvalidSecurityModel(v) => {
                write!(f, "Security model L3 tidak valid: {v} (harus bernilai 1..=5)")
            }
            Self::InvalidFormat(msg) => write!(f, "Format data L3 tidak valid: {msg}"),
        }
    }
}

impl std::error::Error for L3CodecError {}

/// Identifier Unik 32-Byte Domain Eksekusi Terspesialisasi (L3)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DomainId([u8; 32]);

impl DomainId {
    /// Domain bawaan untuk Sovereign App-Chain
    pub const APP_CHAIN_DEFAULT: Self = Self([0x31; 32]);
    /// Domain bawaan untuk Microsecond Order-Book DEX
    pub const DEX_DEFAULT: Self = Self([0x32; 32]);
    /// Domain bawaan untuk High-Frequency Ephemeral Gaming
    pub const GAMING_DEFAULT: Self = Self([0x33; 32]);
    /// Domain bawaan untuk Confidential ZK Shielded Pool
    pub const PRIVACY_DEFAULT: Self = Self([0x34; 32]);
    /// Domain bawaan untuk Verifiable AI & Compute
    pub const COMPUTE_DEFAULT: Self = Self([0x35; 32]);

    /// Membuat DomainId dari 32 byte mentah
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Mengambil referensi byte mentah
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Membuat DomainId deterministik dari nama string berbasis Blake3
    #[must_use]
    pub fn named(name: &str) -> Self {
        let mut data = Vec::with_capacity(16 + name.len());
        data.extend_from_slice(b"AURION-DOMAIN-ID");
        data.extend_from_slice(name.as_bytes());
        Self(*blake3_hash(&data).as_bytes())
    }

    /// Mengubah DomainId menjadi representasi hex string
    #[must_use]
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Display for DomainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

/// 5 Model Keamanan Kanonikal Layer-3 (Rule 18 §4.6)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum L3SecurityModel {
    /// Proof-Secured L3: Keamanan dijamin penuh oleh ZK-Validity Proofs di L2/L1
    ProofSecured = 1,
    /// Attestation-Secured L3: Keamanan dijamin komite validator DAC terpercaya
    AttestationSecured = 2,
    /// L2-Secured L3: Sequencer L2 bertindak langsung sebagai sequencer L3
    L2Secured = 3,
    /// Sovereign L3: Konsensus lokal independen, checkpoint di L2
    Sovereign = 4,
    /// Hybrid L3: Attestation DAC lokal dipadukan dengan periodic ZK state rollups
    Hybrid = 5,
}

impl L3SecurityModel {
    /// Konversi dari u8
    pub fn from_u8(v: u8) -> Result<Self, L3CodecError> {
        match v {
            1 => Ok(Self::ProofSecured),
            2 => Ok(Self::AttestationSecured),
            3 => Ok(Self::L2Secured),
            4 => Ok(Self::Sovereign),
            5 => Ok(Self::Hybrid),
            other => Err(L3CodecError::InvalidSecurityModel(other)),
        }
    }

    /// Konversi ke u8
    #[must_use]
    pub const fn as_u8(&self) -> u8 {
        *self as u8
    }
}

/// Metadata Pendaftaran Domain L3
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainMetadata {
    pub domain_id: DomainId,
    pub name: String,
    pub security_model: L3SecurityModel,
    pub sequencer_pubkey: [u8; 32],
    pub settlement_cadence_blocks: u64,
    pub max_gas_per_block: u64,
    pub created_at: u64,
}

/// Transaksi Terspesialisasi Layer-3
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L3Transaction {
    pub domain_id: DomainId,
    pub sender: Address,
    pub recipient: Address,
    pub amount: Quantum,
    pub fee: Quantum,
    pub nonce: u64,
    pub signature: Signature,
    pub payload: Vec<u8>,
}

impl L3Transaction {
    /// Membuat transaksi L3 baru
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain_id: DomainId,
        sender: Address,
        recipient: Address,
        amount: Quantum,
        fee: Quantum,
        nonce: u64,
        signature: Signature,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            domain_id,
            sender,
            recipient,
            amount,
            fee,
            nonce,
            signature,
            payload,
        }
    }

    /// Menghitung hash transaksi L3 (Blake3 256-bit)
    #[must_use]
    pub fn compute_hash(&self) -> Hash256 {
        let mut data = Vec::with_capacity(L3_TX_BASE_SIZE + self.payload.len());
        data.extend_from_slice(self.domain_id.as_bytes());
        data.extend_from_slice(self.sender.as_bytes());
        data.extend_from_slice(self.recipient.as_bytes());
        data.extend_from_slice(&self.amount.as_u128().to_be_bytes());
        data.extend_from_slice(&self.fee.as_u128().to_be_bytes());
        data.extend_from_slice(&self.nonce.to_be_bytes());
        data.extend_from_slice(self.signature.as_bytes());
        data.extend_from_slice(&self.payload);
        blake3_hash(&data)
    }

    /// Konstruksi data signing preimage yang wajib ditandatangani pengirim Ed25519
    #[must_use]
    pub fn signing_preimage(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(32 + 32 + 32 + 32 + 16 + 16 + 8 + self.payload.len());
        data.extend_from_slice(DST_L3_TX.as_bytes());
        data.extend_from_slice(self.domain_id.as_bytes());
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

    /// Mengodekan transaksi L3 ke dalam format biner kanonikal Big-Endian
    #[must_use]
    pub fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(L3_TX_BASE_SIZE + self.payload.len());
        buf.extend_from_slice(self.domain_id.as_bytes());
        buf.extend_from_slice(self.sender.as_bytes());
        buf.extend_from_slice(self.recipient.as_bytes());
        buf.extend_from_slice(&self.amount.as_u128().to_be_bytes());
        buf.extend_from_slice(&self.fee.as_u128().to_be_bytes());
        buf.extend_from_slice(&self.nonce.to_be_bytes());
        buf.extend_from_slice(self.signature.as_bytes());
        let payload_len: u32 = self.payload.len() as u32;
        buf.extend_from_slice(&payload_len.to_be_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Mendekode format biner kanonikal menjadi L3Transaction
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, L3CodecError> {
        if bytes.len() < L3_TX_BASE_SIZE {
            return Err(L3CodecError::UnexpectedEof {
                expected: L3_TX_BASE_SIZE,
                found: bytes.len(),
            });
        }

        let mut domain_bytes = [0u8; 32];
        domain_bytes.copy_from_slice(&bytes[0..32]);
        let domain_id = DomainId::from_bytes(domain_bytes);

        let mut sender_bytes = [0u8; 32];
        sender_bytes.copy_from_slice(&bytes[32..64]);
        let sender = Address::from_bytes(sender_bytes);

        let mut recipient_bytes = [0u8; 32];
        recipient_bytes.copy_from_slice(&bytes[64..96]);
        let recipient = Address::from_bytes(recipient_bytes);

        let mut amount_bytes = [0u8; 16];
        amount_bytes.copy_from_slice(&bytes[96..112]);
        let amount = Quantum::new(u128::from_be_bytes(amount_bytes));

        let mut fee_bytes = [0u8; 16];
        fee_bytes.copy_from_slice(&bytes[112..128]);
        let fee = Quantum::new(u128::from_be_bytes(fee_bytes));

        let mut nonce_bytes = [0u8; 8];
        nonce_bytes.copy_from_slice(&bytes[128..136]);
        let nonce = u64::from_be_bytes(nonce_bytes);

        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(&bytes[136..200]);
        let signature = Signature::from_bytes(sig_bytes);

        let mut payload_len_bytes = [0u8; 4];
        payload_len_bytes.copy_from_slice(&bytes[200..204]);
        let payload_len = u32::from_be_bytes(payload_len_bytes) as usize;

        if payload_len > MAX_L3_TX_PAYLOAD_SIZE {
            return Err(L3CodecError::PayloadTooLarge {
                max: MAX_L3_TX_PAYLOAD_SIZE,
                actual: payload_len,
            });
        }

        let expected_total = L3_TX_BASE_SIZE + payload_len;
        if bytes.len() < expected_total {
            return Err(L3CodecError::UnexpectedEof {
                expected: expected_total,
                found: bytes.len(),
            });
        }

        let payload = bytes[204..expected_total].to_vec();

        Ok(Self {
            domain_id,
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

/// Struk Bukti Eksekusi Transaksi L3 (Receipt)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L3Receipt {
    pub tx_id: Hash256,
    pub success: bool,
    pub gas_used: u64,
    pub fee_paid: Quantum,
    pub output_data: Vec<u8>,
    pub logs: Vec<Vec<u8>>,
}

impl L3Receipt {
    /// Membuat struk eksekusi L3
    #[must_use]
    pub fn new(
        tx_id: Hash256,
        success: bool,
        gas_used: u64,
        fee_paid: Quantum,
        output_data: Vec<u8>,
        logs: Vec<Vec<u8>>,
    ) -> Self {
        Self {
            tx_id,
            success,
            gas_used,
            fee_paid,
            output_data,
            logs,
        }
    }

    /// Menghitung hash struk eksekusi L3
    #[must_use]
    pub fn compute_hash(&self) -> Hash256 {
        let mut data = Vec::new();
        data.extend_from_slice(self.tx_id.as_bytes());
        data.push(if self.success { 1 } else { 0 });
        data.extend_from_slice(&self.gas_used.to_be_bytes());
        data.extend_from_slice(&self.fee_paid.as_u128().to_be_bytes());
        data.extend_from_slice(&self.output_data);
        for log in &self.logs {
            data.extend_from_slice(log);
        }
        blake3_hash(&data)
    }

    /// Mengodekan struk ke format biner kanonikal Big-Endian
    #[must_use]
    pub fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(L3_RECEIPT_BASE_SIZE + self.output_data.len());
        buf.extend_from_slice(self.tx_id.as_bytes());
        buf.push(if self.success { 1 } else { 0 });
        buf.extend_from_slice(&self.gas_used.to_be_bytes());
        buf.extend_from_slice(&self.fee_paid.as_u128().to_be_bytes());

        let out_len: u32 = self.output_data.len() as u32;
        buf.extend_from_slice(&out_len.to_be_bytes());
        buf.extend_from_slice(&self.output_data);

        let logs_count: u32 = self.logs.len() as u32;
        buf.extend_from_slice(&logs_count.to_be_bytes());
        for log in &self.logs {
            let log_len: u32 = log.len() as u32;
            buf.extend_from_slice(&log_len.to_be_bytes());
            buf.extend_from_slice(log);
        }
        buf
    }

    /// Mendekode format biner kanonikal menjadi L3Receipt
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, L3CodecError> {
        if bytes.len() < L3_RECEIPT_BASE_SIZE {
            return Err(L3CodecError::UnexpectedEof {
                expected: L3_RECEIPT_BASE_SIZE,
                found: bytes.len(),
            });
        }

        let mut tx_bytes = [0u8; 32];
        tx_bytes.copy_from_slice(&bytes[0..32]);
        let tx_id = Hash256::from_bytes(tx_bytes);

        let success = bytes[32] != 0;

        let mut gas_bytes = [0u8; 8];
        gas_bytes.copy_from_slice(&bytes[33..41]);
        let gas_used = u64::from_be_bytes(gas_bytes);

        let mut fee_bytes = [0u8; 16];
        fee_bytes.copy_from_slice(&bytes[41..57]);
        let fee_paid = Quantum::new(u128::from_be_bytes(fee_bytes));

        let mut out_len_bytes = [0u8; 4];
        out_len_bytes.copy_from_slice(&bytes[57..61]);
        let out_len = u32::from_be_bytes(out_len_bytes) as usize;

        let mut offset = 61;
        if bytes.len() < offset + out_len {
            return Err(L3CodecError::UnexpectedEof {
                expected: offset + out_len,
                found: bytes.len(),
            });
        }
        let output_data = bytes[offset..offset + out_len].to_vec();
        offset += out_len;

        if bytes.len() < offset + 4 {
            return Err(L3CodecError::UnexpectedEof {
                expected: offset + 4,
                found: bytes.len(),
            });
        }
        let mut logs_count_bytes = [0u8; 4];
        logs_count_bytes.copy_from_slice(&bytes[offset..offset + 4]);
        let logs_count = u32::from_be_bytes(logs_count_bytes) as usize;
        offset += 4;

        let mut logs = Vec::with_capacity(logs_count);
        for _ in 0..logs_count {
            if bytes.len() < offset + 4 {
                return Err(L3CodecError::UnexpectedEof {
                    expected: offset + 4,
                    found: bytes.len(),
                });
            }
            let mut log_len_bytes = [0u8; 4];
            log_len_bytes.copy_from_slice(&bytes[offset..offset + 4]);
            let log_len = u32::from_be_bytes(log_len_bytes) as usize;
            offset += 4;

            if bytes.len() < offset + log_len {
                return Err(L3CodecError::UnexpectedEof {
                    expected: offset + log_len,
                    found: bytes.len(),
                });
            }
            logs.push(bytes[offset..offset + log_len].to_vec());
            offset += log_len;
        }

        Ok(Self {
            tx_id,
            success,
            gas_used,
            fee_paid,
            output_data,
            logs,
        })
    }
}

/// Blok Eksekusi Layer-3
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L3Block {
    pub domain_id: DomainId,
    pub block_number: u64,
    pub previous_block_hash: Hash256,
    pub state_root: Hash256,
    pub transactions_root: Hash256,
    pub receipts_root: Hash256,
    pub timestamp: u64,
    pub transactions: Vec<L3Transaction>,
}

impl L3Block {
    /// Membuat blok L3 baru
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain_id: DomainId,
        block_number: u64,
        previous_block_hash: Hash256,
        state_root: Hash256,
        transactions_root: Hash256,
        receipts_root: Hash256,
        timestamp: u64,
        transactions: Vec<L3Transaction>,
    ) -> Self {
        Self {
            domain_id,
            block_number,
            previous_block_hash,
            state_root,
            transactions_root,
            receipts_root,
            timestamp,
            transactions,
        }
    }

    /// Menghitung hash unik blok L3 dari atribut headernya
    #[must_use]
    pub fn compute_block_hash(&self) -> Hash256 {
        let mut data = Vec::with_capacity(L3_BLOCK_HEADER_SIZE);
        data.extend_from_slice(self.domain_id.as_bytes());
        data.extend_from_slice(&self.block_number.to_be_bytes());
        data.extend_from_slice(self.previous_block_hash.as_bytes());
        data.extend_from_slice(self.state_root.as_bytes());
        data.extend_from_slice(self.transactions_root.as_bytes());
        data.extend_from_slice(self.receipts_root.as_bytes());
        data.extend_from_slice(&self.timestamp.to_be_bytes());
        let txs_count: u32 = self.transactions.len() as u32;
        data.extend_from_slice(&txs_count.to_be_bytes());
        blake3_hash(&data)
    }

    /// Menghitung Merkle root transaksi L3
    #[must_use]
    pub fn compute_transactions_root(transactions: &[L3Transaction]) -> Hash256 {
        if transactions.is_empty() {
            return Hash256::ZERO;
        }
        let mut hashes: Vec<Hash256> = transactions.iter().map(L3Transaction::compute_hash).collect();
        while hashes.len() > 1 {
            let mut next = Vec::with_capacity(hashes.len().div_ceil(2));
            for chunk in hashes.chunks(2) {
                if chunk.len() == 2 {
                    let mut data = [0u8; 64];
                    data[0..32].copy_from_slice(chunk[0].as_bytes());
                    data[32..64].copy_from_slice(chunk[1].as_bytes());
                    next.push(blake3_hash(&data));
                } else {
                    let mut data = [0u8; 64];
                    data[0..32].copy_from_slice(chunk[0].as_bytes());
                    data[32..64].copy_from_slice(chunk[0].as_bytes());
                    next.push(blake3_hash(&data));
                }
            }
            hashes = next;
        }
        hashes[0]
    }

    /// Mengodekan blok L3 ke format biner kanonikal Big-Endian
    #[must_use]
    pub fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(L3_BLOCK_HEADER_SIZE + self.transactions.len() * L3_TX_BASE_SIZE);
        buf.extend_from_slice(self.domain_id.as_bytes());
        buf.extend_from_slice(&self.block_number.to_be_bytes());
        buf.extend_from_slice(self.previous_block_hash.as_bytes());
        buf.extend_from_slice(self.state_root.as_bytes());
        buf.extend_from_slice(self.transactions_root.as_bytes());
        buf.extend_from_slice(self.receipts_root.as_bytes());
        buf.extend_from_slice(&self.timestamp.to_be_bytes());

        let txs_count: u32 = self.transactions.len() as u32;
        buf.extend_from_slice(&txs_count.to_be_bytes());
        for tx in &self.transactions {
            let encoded_tx = tx.encode_canonical();
            let len: u32 = encoded_tx.len() as u32;
            buf.extend_from_slice(&len.to_be_bytes());
            buf.extend_from_slice(&encoded_tx);
        }
        buf
    }

    /// Mendekode format biner kanonikal menjadi L3Block
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, L3CodecError> {
        if bytes.len() < L3_BLOCK_HEADER_SIZE {
            return Err(L3CodecError::UnexpectedEof {
                expected: L3_BLOCK_HEADER_SIZE,
                found: bytes.len(),
            });
        }

        let mut domain_bytes = [0u8; 32];
        domain_bytes.copy_from_slice(&bytes[0..32]);
        let domain_id = DomainId::from_bytes(domain_bytes);

        let mut block_num_bytes = [0u8; 8];
        block_num_bytes.copy_from_slice(&bytes[32..40]);
        let block_number = u64::from_be_bytes(block_num_bytes);

        let mut prev_bytes = [0u8; 32];
        prev_bytes.copy_from_slice(&bytes[40..72]);
        let previous_block_hash = Hash256::from_bytes(prev_bytes);

        let mut state_bytes = [0u8; 32];
        state_bytes.copy_from_slice(&bytes[72..104]);
        let state_root = Hash256::from_bytes(state_bytes);

        let mut tx_root_bytes = [0u8; 32];
        tx_root_bytes.copy_from_slice(&bytes[104..136]);
        let transactions_root = Hash256::from_bytes(tx_root_bytes);

        let mut rcpt_root_bytes = [0u8; 32];
        rcpt_root_bytes.copy_from_slice(&bytes[136..168]);
        let receipts_root = Hash256::from_bytes(rcpt_root_bytes);

        let mut time_bytes = [0u8; 8];
        time_bytes.copy_from_slice(&bytes[168..176]);
        let timestamp = u64::from_be_bytes(time_bytes);

        let mut count_bytes = [0u8; 4];
        count_bytes.copy_from_slice(&bytes[176..180]);
        let txs_count = u32::from_be_bytes(count_bytes) as usize;

        let mut offset = 180;
        let mut transactions = Vec::with_capacity(txs_count);
        for _ in 0..txs_count {
            if bytes.len() < offset + 4 {
                return Err(L3CodecError::UnexpectedEof {
                    expected: offset + 4,
                    found: bytes.len(),
                });
            }
            let mut tx_len_bytes = [0u8; 4];
            tx_len_bytes.copy_from_slice(&bytes[offset..offset + 4]);
            let tx_len = u32::from_be_bytes(tx_len_bytes) as usize;
            offset += 4;

            if bytes.len() < offset + tx_len {
                return Err(L3CodecError::UnexpectedEof {
                    expected: offset + tx_len,
                    found: bytes.len(),
                });
            }
            let tx = L3Transaction::decode_canonical(&bytes[offset..offset + tx_len])?;
            transactions.push(tx);
            offset += tx_len;
        }

        Ok(Self {
            domain_id,
            block_number,
            previous_block_hash,
            state_root,
            transactions_root,
            receipts_root,
            timestamp,
            transactions,
        })
    }
}

/// Komitmen Checkpoint State Periodik L3 untuk Penyelesaian di L2 (Settlement Commitment)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L3Checkpoint {
    pub domain_id: DomainId,
    pub checkpoint_id: u64,
    pub start_block: u64,
    pub end_block: u64,
    pub previous_state_root: Hash256,
    pub new_state_root: Hash256,
    pub transactions_count: u64,
    pub signature: Signature,
    pub proof_data: Vec<u8>,
}

impl L3Checkpoint {
    /// Membuat checkpoint L3 baru
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain_id: DomainId,
        checkpoint_id: u64,
        start_block: u64,
        end_block: u64,
        previous_state_root: Hash256,
        new_state_root: Hash256,
        transactions_count: u64,
        signature: Signature,
        proof_data: Vec<u8>,
    ) -> Self {
        Self {
            domain_id,
            checkpoint_id,
            start_block,
            end_block,
            previous_state_root,
            new_state_root,
            transactions_count,
            signature,
            proof_data,
        }
    }

    /// Menghitung hash unik komitmen checkpoint
    #[must_use]
    pub fn compute_checkpoint_hash(&self) -> Hash256 {
        let mut data = Vec::with_capacity(L3_CHECKPOINT_BASE_SIZE + self.proof_data.len());
        data.extend_from_slice(self.domain_id.as_bytes());
        data.extend_from_slice(&self.checkpoint_id.to_be_bytes());
        data.extend_from_slice(&self.start_block.to_be_bytes());
        data.extend_from_slice(&self.end_block.to_be_bytes());
        data.extend_from_slice(self.previous_state_root.as_bytes());
        data.extend_from_slice(self.new_state_root.as_bytes());
        data.extend_from_slice(&self.transactions_count.to_be_bytes());
        data.extend_from_slice(self.signature.as_bytes());
        data.extend_from_slice(&self.proof_data);
        blake3_hash(&data)
    }

    /// Konstruksi data signing preimage yang ditandatangani sequencer/komite L3
    #[must_use]
    pub fn signing_preimage(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(32 + 32 + 8 + 8 + 8 + 32 + 32 + 8 + self.proof_data.len());
        data.extend_from_slice(DST_L3_CHECKPOINT.as_bytes());
        data.extend_from_slice(self.domain_id.as_bytes());
        data.extend_from_slice(&self.checkpoint_id.to_be_bytes());
        data.extend_from_slice(&self.start_block.to_be_bytes());
        data.extend_from_slice(&self.end_block.to_be_bytes());
        data.extend_from_slice(self.previous_state_root.as_bytes());
        data.extend_from_slice(self.new_state_root.as_bytes());
        data.extend_from_slice(&self.transactions_count.to_be_bytes());
        data.extend_from_slice(&self.proof_data);
        data
    }

    /// Verifikasi tanda tangan digital sequencer atau komite L3
    #[must_use]
    pub fn verify_signature(&self, sequencer_pubkey: &[u8; 32]) -> bool {
        let preimage = self.signing_preimage();
        ed25519_verify_strict(sequencer_pubkey, &preimage, &self.signature).is_ok()
    }

    /// Mengodekan checkpoint L3 ke format biner kanonikal Big-Endian
    #[must_use]
    pub fn encode_canonical(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(L3_CHECKPOINT_BASE_SIZE + self.proof_data.len());
        buf.extend_from_slice(self.domain_id.as_bytes());
        buf.extend_from_slice(&self.checkpoint_id.to_be_bytes());
        buf.extend_from_slice(&self.start_block.to_be_bytes());
        buf.extend_from_slice(&self.end_block.to_be_bytes());
        buf.extend_from_slice(self.previous_state_root.as_bytes());
        buf.extend_from_slice(self.new_state_root.as_bytes());
        buf.extend_from_slice(&self.transactions_count.to_be_bytes());
        buf.extend_from_slice(self.signature.as_bytes());

        let proof_len: u32 = self.proof_data.len() as u32;
        buf.extend_from_slice(&proof_len.to_be_bytes());
        buf.extend_from_slice(&self.proof_data);
        buf
    }

    /// Mendekode format biner kanonikal menjadi L3Checkpoint
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, L3CodecError> {
        if bytes.len() < L3_CHECKPOINT_BASE_SIZE {
            return Err(L3CodecError::UnexpectedEof {
                expected: L3_CHECKPOINT_BASE_SIZE,
                found: bytes.len(),
            });
        }

        let mut domain_bytes = [0u8; 32];
        domain_bytes.copy_from_slice(&bytes[0..32]);
        let domain_id = DomainId::from_bytes(domain_bytes);

        let mut cp_bytes = [0u8; 8];
        cp_bytes.copy_from_slice(&bytes[32..40]);
        let checkpoint_id = u64::from_be_bytes(cp_bytes);

        let mut start_bytes = [0u8; 8];
        start_bytes.copy_from_slice(&bytes[40..48]);
        let start_block = u64::from_be_bytes(start_bytes);

        let mut end_bytes = [0u8; 8];
        end_bytes.copy_from_slice(&bytes[48..56]);
        let end_block = u64::from_be_bytes(end_bytes);

        let mut prev_bytes = [0u8; 32];
        prev_bytes.copy_from_slice(&bytes[56..88]);
        let previous_state_root = Hash256::from_bytes(prev_bytes);

        let mut new_bytes = [0u8; 32];
        new_bytes.copy_from_slice(&bytes[88..120]);
        let new_state_root = Hash256::from_bytes(new_bytes);

        let mut count_bytes = [0u8; 8];
        count_bytes.copy_from_slice(&bytes[120..128]);
        let transactions_count = u64::from_be_bytes(count_bytes);

        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(&bytes[128..192]);
        let signature = Signature::from_bytes(sig_bytes);

        let mut proof_len_bytes = [0u8; 4];
        proof_len_bytes.copy_from_slice(&bytes[192..196]);
        let proof_len = u32::from_be_bytes(proof_len_bytes) as usize;

        if proof_len > MAX_L3_PROOF_SIZE {
            return Err(L3CodecError::PayloadTooLarge {
                max: MAX_L3_PROOF_SIZE,
                actual: proof_len,
            });
        }

        let expected_total = L3_CHECKPOINT_BASE_SIZE + proof_len;
        if bytes.len() < expected_total {
            return Err(L3CodecError::UnexpectedEof {
                expected: expected_total,
                found: bytes.len(),
            });
        }

        let proof_data = bytes[196..expected_total].to_vec();

        Ok(Self {
            domain_id,
            checkpoint_id,
            start_block,
            end_block,
            previous_state_root,
            new_state_root,
            transactions_count,
            signature,
            proof_data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::Keypair;

    #[test]
    fn test_domain_id_named_and_predefined() {
        assert_eq!(DomainId::APP_CHAIN_DEFAULT.as_bytes(), &[0x31; 32]);
        assert_eq!(DomainId::DEX_DEFAULT.as_bytes(), &[0x32; 32]);

        let dex_named = DomainId::named("dex-microsecond-v1");
        let dex_named_2 = DomainId::named("dex-microsecond-v1");
        assert_eq!(dex_named, dex_named_2);
        assert_ne!(dex_named, DomainId::named("gaming-high-freq"));
    }

    #[test]
    fn test_l3_security_model_roundtrip() {
        for (code, model) in [
            (1, L3SecurityModel::ProofSecured),
            (2, L3SecurityModel::AttestationSecured),
            (3, L3SecurityModel::L2Secured),
            (4, L3SecurityModel::Sovereign),
            (5, L3SecurityModel::Hybrid),
        ] {
            assert_eq!(L3SecurityModel::from_u8(code).unwrap(), model);
            assert_eq!(model.as_u8(), code);
        }
        assert!(L3SecurityModel::from_u8(0).is_err());
        assert!(L3SecurityModel::from_u8(6).is_err());
    }

    #[test]
    fn test_l3_transaction_serialization_roundtrip_and_signature() {
        let domain_id = DomainId::DEX_DEFAULT;
        let sender = Address::from_bytes([0x11; 32]);
        let recipient = Address::from_bytes([0x22; 32]);
        let amount = Quantum::new(50_000_000); // 0.5 AUR
        let fee = Quantum::new(100);
        let nonce = 42;
        let payload = b"LIMIT_BUY:AUR/USDT:100:50000000".to_vec();

        // Sign transaction
        let sk = [0x55; 32];
        let kp = Keypair::from_seed(&sk);
        let pk = kp.public_key_bytes();
        let unsigned_tx = L3Transaction::new(
            domain_id,
            sender,
            recipient,
            amount,
            fee,
            nonce,
            Signature::from_bytes([0u8; 64]),
            payload.clone(),
        );

        let preimage = unsigned_tx.signing_preimage();
        let sig = kp.sign(&preimage);

        let tx = L3Transaction::new(
            domain_id,
            sender,
            recipient,
            amount,
            fee,
            nonce,
            sig,
            payload,
        );

        assert!(tx.verify_signature(&pk));

        // Canonical encode/decode
        let encoded = tx.encode_canonical();
        let decoded = L3Transaction::decode_canonical(&encoded).unwrap();
        assert_eq!(tx, decoded);
        assert_eq!(tx.compute_hash(), decoded.compute_hash());
    }

    #[test]
    fn test_l3_receipt_serialization_roundtrip() {
        let tx_id = Hash256::from_bytes([0x77; 32]);
        let receipt = L3Receipt::new(
            tx_id,
            true,
            21_000,
            Quantum::new(500),
            b"ORDER_FILLED_AT_INDEX_12".to_vec(),
            vec![b"LOG_TRADE_EXEC".to_vec(), b"LOG_FEE_BURN".to_vec()],
        );

        let encoded = receipt.encode_canonical();
        let decoded = L3Receipt::decode_canonical(&encoded).unwrap();
        assert_eq!(receipt, decoded);
        assert_eq!(receipt.compute_hash(), decoded.compute_hash());
    }

    #[test]
    fn test_l3_block_serialization_roundtrip_and_merkle_root() {
        let domain_id = DomainId::GAMING_DEFAULT;
        let tx1 = L3Transaction::new(
            domain_id,
            Address::from_bytes([0x01; 32]),
            Address::from_bytes([0x02; 32]),
            Quantum::new(10),
            Quantum::new(1),
            1,
            Signature::from_bytes([0xaa; 64]),
            vec![1, 2, 3],
        );
        let tx2 = L3Transaction::new(
            domain_id,
            Address::from_bytes([0x03; 32]),
            Address::from_bytes([0x04; 32]),
            Quantum::new(20),
            Quantum::new(2),
            2,
            Signature::from_bytes([0xbb; 64]),
            vec![4, 5, 6],
        );

        let txs = vec![tx1, tx2];
        let tx_root = L3Block::compute_transactions_root(&txs);
        assert_ne!(tx_root, Hash256::ZERO);

        let block = L3Block::new(
            domain_id,
            1001,
            Hash256::from_bytes([0x33; 32]),
            Hash256::from_bytes([0x44; 32]),
            tx_root,
            Hash256::from_bytes([0x55; 32]),
            1_700_000_000,
            txs,
        );

        let encoded = block.encode_canonical();
        let decoded = L3Block::decode_canonical(&encoded).unwrap();
        assert_eq!(block, decoded);
        assert_eq!(block.compute_block_hash(), decoded.compute_block_hash());
    }

    #[test]
    fn test_l3_checkpoint_serialization_roundtrip_and_signature() {
        let domain_id = DomainId::PRIVACY_DEFAULT;
        let sk = [0x99; 32];
        let kp = Keypair::from_seed(&sk);
        let pk = kp.public_key_bytes();

        let mut unsigned_cp = L3Checkpoint::new(
            domain_id,
            50,
            5000,
            5100,
            Hash256::from_bytes([0x12; 32]),
            Hash256::from_bytes([0x34; 32]),
            2450,
            Signature::from_bytes([0u8; 64]),
            vec![0xde, 0xad, 0xbe, 0xef],
        );

        let preimage = unsigned_cp.signing_preimage();
        let sig = kp.sign(&preimage);
        unsigned_cp.signature = sig;

        assert!(unsigned_cp.verify_signature(&pk));

        let encoded = unsigned_cp.encode_canonical();
        let decoded = L3Checkpoint::decode_canonical(&encoded).unwrap();
        assert_eq!(unsigned_cp, decoded);
        assert_eq!(unsigned_cp.compute_checkpoint_hash(), decoded.compute_checkpoint_hash());
    }
}

