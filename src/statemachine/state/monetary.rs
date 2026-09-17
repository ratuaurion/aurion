//! State moneter global Aurion: suplai beredar, pembakaran koin, dan subsidi.

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::core::{
    MonetaryError, Quantum, FEE_BURN_PERCENTAGE, FEE_MINER_PERCENTAGE, HALVING_INTERVAL_BLOCKS,
    INITIAL_BLOCK_SUBSIDY_QUANTA, MAX_HALVING_ERAS, MAX_SUPPLY_QUANTA,
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

/// Menghitung subsidi blok deterministik pada tinggi blok `height` sesuai jadwal emisi resmi.
///
/// Formula emisi (AURION-MONETARY-POLICY-SPECIFICATION.md Bab 3):
/// - Blok 0 (Genesis): 0 Quantum
/// - Era e = (height - 1) / HALVING_INTERVAL_BLOCKS
/// - Jika e >= MAX_HALVING_ERAS: 0 Quantum
/// - Jika e < MAX_HALVING_ERAS: INITIAL_BLOCK_SUBSIDY_QUANTA >> e
pub fn calculate_block_subsidy(height: u64) -> Quantum {
    if height == 0 {
        return Quantum::ZERO;
    }
    let era = (height - 1) / HALVING_INTERVAL_BLOCKS;
    if era >= MAX_HALVING_ERAS {
        return Quantum::ZERO;
    }
    let subsidy = INITIAL_BLOCK_SUBSIDY_QUANTA >> (era as u32);
    Quantum::new(subsidy)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_block_subsidy_schedule() {
        // Blok 0 (Genesis) tidak menerbitkan subsidi reguler
        assert_eq!(calculate_block_subsidy(0), Quantum::ZERO);

        // Era 0 (1 .. 2_145_000): 10 AUR = 1_000_000_000 Q
        let era0_subsidy = Quantum::new(1_000_000_000);
        assert_eq!(calculate_block_subsidy(1), era0_subsidy);
        assert_eq!(calculate_block_subsidy(100), era0_subsidy);
        assert_eq!(calculate_block_subsidy(2_145_000), era0_subsidy);

        // Era 1 (2_145_001 .. 4_290_000): 5 AUR = 500_000_000 Q
        let era1_subsidy = Quantum::new(500_000_000);
        assert_eq!(calculate_block_subsidy(2_145_001), era1_subsidy);
        assert_eq!(calculate_block_subsidy(4_290_000), era1_subsidy);

        // Era 2: 2.5 AUR = 250_000_000 Q
        assert_eq!(calculate_block_subsidy(4_290_001), Quantum::new(250_000_000));

        // Era 29: 1 Quantum
        let era29_start = 29 * HALVING_INTERVAL_BLOCKS + 1;
        assert_eq!(calculate_block_subsidy(era29_start), Quantum::new(1));

        // Era >= 30: 0 Quantum
        let era30_start = 30 * HALVING_INTERVAL_BLOCKS + 1;
        assert_eq!(calculate_block_subsidy(era30_start), Quantum::ZERO);
        assert_eq!(calculate_block_subsidy(100_000_000), Quantum::ZERO);
    }

    #[test]
    fn test_finite_convergence_under_cap() {
        // Total akumulasi subsidi untuk 30 era wajib <= 4_290_000_000_000_000 Q
        let mut total_mined: u128 = 0;
        for era in 0..30 {
            let subsidy = INITIAL_BLOCK_SUBSIDY_QUANTA >> era;
            let era_total = subsidy * (HALVING_INTERVAL_BLOCKS as u128);
            total_mined += era_total;
        }

        // Nilai terpotong bilangan bulat eksak: 4.289.999.972.115.000 Q
        // Sisa unmintable dust yang tidak pernah dicetak = 27.885.000 Q (0,27885 AUR)
        assert_eq!(total_mined, 4_289_999_972_115_000);
        assert!(total_mined < 4_290_000_000_000_000);
    }
}
