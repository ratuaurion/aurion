//! Struktur data transaksi kanonikal Aurion (Basis 184 Bytes + Payload Opsional).

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::core::{Address, Quantum, Signature};

pub const TRANSACTION_BASE_BYTES: usize = 184;
pub const MAX_TRANSACTION_PAYLOAD_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TxType {
    Transfer = 0x01,
    Stake = 0x02,
    Unstake = 0x03,
    GovernanceVote = 0x04,
}

impl CanonicalEncode for TxType {
    #[inline]
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        buf.push(*self as u8);
    }
}

impl CanonicalDecode for TxType {
    #[inline]
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let b = u8::decode_canonical(bytes, cursor)?;
        match b {
            0x01 => Ok(TxType::Transfer),
            0x02 => Ok(TxType::Stake),
            0x03 => Ok(TxType::Unstake),
            0x04 => Ok(TxType::GovernanceVote),
            other => Err(CodecError::TrailingBytes(other as usize)),
        }
    }
}

/// Transaksi kanonikal Aurion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub version: u16,
    pub chain_id: u32,
    pub tx_type: TxType,
    pub flags: u8,
    pub sender: Address,
    pub recipient: Address,
    pub nonce: u64,
    pub amount: Quantum,
    pub fee: Quantum,
    pub valid_until: u64,
    pub payload: Vec<u8>,
    pub signature: Signature,
}

impl CanonicalEncode for Transaction {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        self.version.encode_canonical(buf);
        self.chain_id.encode_canonical(buf);
        self.tx_type.encode_canonical(buf);
        self.flags.encode_canonical(buf);
        self.sender.encode_canonical(buf);
        self.recipient.encode_canonical(buf);
        self.nonce.encode_canonical(buf);
        self.amount.encode_canonical(buf);
        self.fee.encode_canonical(buf);
        self.valid_until.encode_canonical(buf);
        self.payload.encode_canonical(buf);
        self.signature.encode_canonical(buf);
    }
}

impl CanonicalDecode for Transaction {
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let version = u16::decode_canonical(bytes, cursor)?;
        let chain_id = u32::decode_canonical(bytes, cursor)?;
        let tx_type = TxType::decode_canonical(bytes, cursor)?;
        let flags = u8::decode_canonical(bytes, cursor)?;
        let sender = Address::decode_canonical(bytes, cursor)?;
        let recipient = Address::decode_canonical(bytes, cursor)?;
        let nonce = u64::decode_canonical(bytes, cursor)?;
        let amount = Quantum::decode_canonical(bytes, cursor)?;
        let fee = Quantum::decode_canonical(bytes, cursor)?;
        let valid_until = u64::decode_canonical(bytes, cursor)?;
        let payload = Vec::<u8>::decode_canonical(bytes, cursor)?;
        let signature = Signature::decode_canonical(bytes, cursor)?;

        Ok(Transaction {
            version,
            chain_id,
            tx_type,
            flags,
            sender,
            recipient,
            nonce,
            amount,
            fee,
            valid_until,
            payload,
            signature,
        })
    }
}
