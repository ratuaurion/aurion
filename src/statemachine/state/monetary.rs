//! State moneter global Aurion: suplai beredar, pembakaran koin, dan subsidi.

use crate::codec::{CanonicalDecode, CanonicalEncode, CodecError};
use crate::core::{
    MonetaryError, Quantum, BLOCK_REWARD_PROPOSER_PERCENT, BLOCK_REWARD_QUANTA,
    BLOCK_REWARD_VOTERS_PERCENT, FEE_BURN_PERCENTAGE, FEE_VALIDATOR_PERCENTAGE,
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

/// Menghitung hadiah blok kanonikal Aurion-BFT pada tinggi blok `height`.
///
/// Formula emisi BFT (CONSTITUTION.md Bab I Pasal 1 Ayat 4):
/// - Blok 0 (Genesis): 0 Quantum
/// - Blok >= 1: 1 AUR = 1.000.000.000 Quantum (BLOCK_REWARD_QUANTA)
pub fn calculate_block_reward(height: u64) -> Quantum {
    if height == 0 {
        Quantum::ZERO
    } else {
        Quantum::new(BLOCK_REWARD_QUANTA)
    }
}

/// Alias kompatibilitas untuk menghitung subsidi/hadiah blok kanonikal.
pub fn calculate_block_subsidy(height: u64) -> Quantum {
    calculate_block_reward(height)
}

/// Pembagian hadiah blok kanonikal BFT antara Proposer dan Validator penandatangan QC.
/// - 20% dialokasikan ke Proposer pembuat blok.
/// - 80% dibagi rata ke seluruh validator penandatangan Precommit QC.
/// - Sisa pembulatan integer (remainder dust) diberikan secara deterministik ke Proposer.
pub fn split_block_reward(
    reward: Quantum,
    voter_count: usize,
) -> Result<(Quantum, Quantum), MonetaryError> {
    if voter_count == 0 {
        return Ok((reward, Quantum::ZERO));
    }

    let proposer_base = reward
        .checked_mul(BLOCK_REWARD_PROPOSER_PERCENT)?
        .checked_div(100)?;
    let voters_total = reward
        .checked_mul(BLOCK_REWARD_VOTERS_PERCENT)?
        .checked_div(100)?;

    let per_voter = voters_total.checked_div(voter_count as u128)?;
    let voters_distributed = per_voter.checked_mul(voter_count as u128)?;
    let dust = voters_total.checked_sub(voters_distributed)?;
    let proposer_reward = proposer_base.checked_add(dust)?;

    Ok((proposer_reward, per_voter))
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

    /// Proses pembagian fee transaksi: 100% dialokasikan ke validator pembuat blok (0% burn).
    /// Mengembalikan tuple (fee_burned = 0, fee_validator = fee).
    pub fn split_fee(fee: Quantum) -> Result<(Quantum, Quantum), MonetaryError> {
        let burn_amt = fee.checked_mul(FEE_BURN_PERCENTAGE)?.checked_div(100)?;
        let validator_amt = fee
            .checked_mul(FEE_VALIDATOR_PERCENTAGE)?
            .checked_div(100)?;

        let distributed = burn_amt.checked_add(validator_amt)?;
        let remainder = fee.checked_sub(distributed)?;
        let validator_total = validator_amt.checked_add(remainder)?;

        Ok((burn_amt, validator_total))
    }

    /// Catat pembakaran fee ke dalam state moneter global.
    pub fn apply_burn(&mut self, burn_amount: Quantum) -> Result<(), MonetaryError> {
        self.total_burned = self.total_burned.checked_add(burn_amount)?;
        Ok(())
    }

    /// Catat penerbitan koin baru (misalnya dari emisi blok BFT sesuai INV-MON-03).
    pub fn apply_issuance(&mut self, issuance: Quantum) -> Result<(), MonetaryError> {
        let next = self.total_issued.checked_add(issuance)?;
        self.total_issued = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_block_reward_schedule() {
        assert_eq!(calculate_block_reward(0), Quantum::ZERO);
        assert_eq!(calculate_block_reward(1), Quantum::new(1_000_000_000));
        assert_eq!(calculate_block_reward(100), Quantum::new(1_000_000_000));
        assert_eq!(
            calculate_block_reward(10_000_000),
            Quantum::new(1_000_000_000)
        );
    }

    #[test]
    fn test_split_block_reward_exactness() {
        let reward = Quantum::new(1_000_000_000); // 1 AUR = 10^9 Q

        // 4 Validator QC
        let (proposer, per_voter) = split_block_reward(reward, 4).unwrap();
        assert_eq!(proposer.as_u128(), 200_000_000);
        assert_eq!(per_voter.as_u128(), 200_000_000);
        let total = proposer.as_u128() + per_voter.as_u128() * 4;
        assert_eq!(total, reward.as_u128());

        // 3 Validator QC (800_000_000 / 3 = 266_666_666, sisa 2 Q ke Proposer)
        let (proposer3, per_voter3) = split_block_reward(reward, 3).unwrap();
        assert_eq!(per_voter3.as_u128(), 266_666_666);
        assert_eq!(proposer3.as_u128(), 200_000_002);
        let total3 = proposer3.as_u128() + per_voter3.as_u128() * 3;
        assert_eq!(total3, reward.as_u128());
    }

    #[test]
    fn test_split_fee_100_percent_to_validator() {
        let fee = Quantum::new(50_000_000);
        let (burned, validator) = MonetaryState::split_fee(fee).unwrap();
        assert_eq!(burned, Quantum::ZERO);
        assert_eq!(validator, fee);
    }
}
