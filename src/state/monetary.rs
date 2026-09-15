//! State moneter global Aurion: suplai beredar, pembakaran koin, dan subsidi.

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::core::{
    MonetaryError, Quantum, FEE_BURN_PERCENTAGE, FEE_MINER_PERCENTAGE, MAX_SUPPLY_QUANTA,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonetaryState {
    pub total_issued: Quantum,
    pub total_burned: Quantum,
}

impl Default for MonetaryState {
    fn default() -> Self {
        Self {
            total_issued: Quantum::ZERO,
            total_burned: Quantum::ZERO,
        }
    }
}

impl CanonicalEncode for MonetaryState {
    fn encode_canonical(&self, buf: &mut Vec<u8>) {
        self.total_issued.encode_canonical(buf);
        self.total_burned.encode_canonical(buf);
    }
}

impl CanonicalDecode for MonetaryState {
    fn decode_canonical(bytes: &[u8], cursor: &mut usize) -> Result<Self, CodecError> {
        let total_issued = Quantum::decode_canonical(bytes, cursor)?;
        let total_burned = Quantum::decode_canonical(bytes, cursor)?;
        Ok(MonetaryState {
            total_issued,
            total_burned,
        })
    }
}

impl MonetaryState {
    pub fn new(total_issued: Quantum, total_burned: Quantum) -> Self {
        Self {
            total_issued,
            total_burned,
        }
    }

    /// Pasokan efektif yang sedang beredar (Total Issued - Total Burned).
    pub fn circulating_supply(&self) -> Result<Quantum, MonetaryError> {
        self.total_issued.checked_sub(self.total_burned)
    }

    /// Proses pembagian fee transaksi: 20% dibakar permanen, 80% dialokasikan ke produser blok.
    pub fn split_fee(fee: Quantum) -> Result<(Quantum, Quantum), MonetaryError> {
        let burn_amt = fee.checked_mul(FEE_BURN_PERCENTAGE)?.checked_div(100)?;
        let miner_amt = fee.checked_mul(FEE_MINER_PERCENTAGE)?.checked_div(100)?;

        // Pastikan sisa pembagian bulat tidak hilang
        let distributed = burn_amt.checked_add(miner_amt)?;
        let remainder = fee.checked_sub(distributed)?;
        let miner_total = miner_amt.checked_add(remainder)?;

        Ok((burn_amt, miner_total))
    }

    /// Catat pembakaran fee ke dalam state moneter global.
    pub fn apply_burn(&mut self, burn_amount: Quantum) -> Result<(), MonetaryError> {
        self.total_burned = self.total_burned.checked_add(burn_amount)?;
        Ok(())
    }

    /// Catat penerbitan koin baru (misalnya dari coinbase subsidy).
    pub fn apply_issuance(&mut self, issuance: Quantum) -> Result<(), MonetaryError> {
        let next = self.total_issued.checked_add(issuance)?;
        if next.as_u128() > MAX_SUPPLY_QUANTA {
            return Err(MonetaryError::SupplyCapExceeded(next.as_u128()));
        }
        self.total_issued = next;
        Ok(())
    }
}
