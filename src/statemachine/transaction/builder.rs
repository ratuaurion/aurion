//! Signing Preimage and TxID construction for Aurion Transactions.

use crate::codec::CanonicalEncode;
use crate::core::Hash256;
use crate::crypto::{blake3_derive_key, blake3_hash, DST_TX, DST_TX_ID};
use crate::transaction::types::Transaction;

impl Transaction {
    /// Konstruksi byte signing preimage dengan prefix domain DST "AURION-TX-V1".
    pub fn signing_preimage(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Domain separation header
        buf.extend_from_slice(DST_TX.as_bytes());
        buf.push(0x00);

        self.version.encode_canonical(&mut buf);
        self.chain_id.encode_canonical(&mut buf);
        self.tx_type.encode_canonical(&mut buf);
        self.flags.encode_canonical(&mut buf);
        self.sender.encode_canonical(&mut buf);
        self.recipient.encode_canonical(&mut buf);
        self.nonce.encode_canonical(&mut buf);
        self.amount.encode_canonical(&mut buf);
        self.fee.encode_canonical(&mut buf);
        self.valid_until.encode_canonical(&mut buf);
        self.payload.encode_canonical(&mut buf);

        buf
    }

    /// Hitung TxID kanonikal transaksi menggunakan Blake3 derive-key "AURION-TX-ID-V1".
    pub fn compute_tx_id(&self) -> Hash256 {
        let mut raw = Vec::new();
        self.encode_canonical(&mut raw);
        blake3_derive_key(DST_TX_ID, &raw)
    }

    /// Hitung hash signing preimage.
    pub fn preimage_hash(&self) -> Hash256 {
        blake3_hash(&self.signing_preimage())
    }
}
