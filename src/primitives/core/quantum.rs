//! Quantum: Unit moneter atomik terkecil Aurion.
//! Mematuhi Invariant AUR-ARCH-012: Absolute Zero Floating-Point Arithmetic.

use std::fmt;
use thiserror::Error;

/// Unit atomik per 1 AUR (10^9 Quanta, 9 desimal).
pub const QUANTA_PER_AUR: u128 = 1_000_000_000;

/// Batas maksimum suplai dalam satuan AUR (66 Juta).
pub const MAX_SUPPLY_AUR: u128 = 66_000_000;

/// Batas maksimum suplai dalam satuan Quantum (66 Juta * 10^9 = 66 Kuadriliun).
pub const MAX_SUPPLY_QUANTA: u128 = 66_000_000_000_000_000;

/// Alokasi awal Master Treasury di Blok 0 (100% Pasokan Genesis = 66.000.000 AUR).
pub const MASTER_TREASURY_ALLOCATION_QUANTA: u128 = 66_000_000_000_000_000;

/// Hadiah blok tetap kanonikal Aurion-BFT (1 AUR = 1.000.000.000 Quantum).
pub const BLOCK_REWARD_QUANTA: u128 = 1_000_000_000;

/// Rasio hadiah blok untuk Proposer pembuat blok (20%).
pub const BLOCK_REWARD_PROPOSER_PERCENT: u128 = 20;

/// Rasio hadiah blok untuk Validator penandatangan Precommit QC (80%).
pub const BLOCK_REWARD_VOTERS_PERCENT: u128 = 80;

/// Rasio alokasi fee transaksi kepada validator pembuat blok (100%).
pub const FEE_VALIDATOR_PERCENTAGE: u128 = 100;

/// Rasio pembakaran fee transaksi protokol (0% Burn).
pub const FEE_BURN_PERCENTAGE: u128 = 0;

/// Waktu target produksi per blok dalam detik (Round-Based BFT).
pub const TARGET_BLOCK_TIME_SECONDS: u64 = 60;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MonetaryError {
    #[error("Arithmetic overflow in Quantum operation")]
    Overflow,
    #[error("Arithmetic underflow in Quantum operation")]
    Underflow,
    #[error("Supply hard cap of 66,000,000 AUR exceeded: {0}")]
    SupplyCapExceeded(u128),
    #[error("Division by zero in monetary calculation")]
    DivisionByZero,
    #[error("Invalid precision: input exceeds 9 decimal places")]
    InvalidPrecision,
}

/// Tipe data moneter berpresisi tetap berukuran u128.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Quantum(pub u128);

impl Quantum {
    pub const ZERO: Quantum = Quantum(0);
    pub const ONE: Quantum = Quantum(1);
    pub const ONE_AUR: Quantum = Quantum(QUANTA_PER_AUR);
    pub const MAX_SUPPLY: Quantum = Quantum(MAX_SUPPLY_QUANTA);

    #[inline]
    pub const fn new(val: u128) -> Self {
        Quantum(val)
    }

    #[inline]
    pub fn from_aur(aur: u64) -> Result<Self, MonetaryError> {
        let quanta = (aur as u128)
            .checked_mul(QUANTA_PER_AUR)
            .ok_or(MonetaryError::Overflow)?;
        Ok(Quantum(quanta))
    }

    #[inline]
    pub const fn as_u128(&self) -> u128 {
        self.0
    }

    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }


    #[inline]
    pub fn checked_add(self, other: Quantum) -> Result<Quantum, MonetaryError> {
        self.0
            .checked_add(other.0)
            .map(Quantum)
            .ok_or(MonetaryError::Overflow)
    }

    #[inline]
    pub fn checked_add_bounded(self, other: Quantum) -> Result<Quantum, MonetaryError> {
        let res = self.checked_add(other)?;
        if res.0 > MAX_SUPPLY_QUANTA {
            return Err(MonetaryError::SupplyCapExceeded(res.0));
        }
        Ok(res)
    }

    #[inline]
    pub fn checked_sub(self, other: Quantum) -> Result<Quantum, MonetaryError> {
        self.0
            .checked_sub(other.0)
            .map(Quantum)
            .ok_or(MonetaryError::Underflow)
    }

    #[inline]
    pub fn checked_mul(self, rhs: u128) -> Result<Quantum, MonetaryError> {
        self.0
            .checked_mul(rhs)
            .map(Quantum)
            .ok_or(MonetaryError::Overflow)
    }

    #[inline]
    pub fn checked_div(self, rhs: u128) -> Result<Quantum, MonetaryError> {
        if rhs == 0 {
            return Err(MonetaryError::DivisionByZero);
        }
        Ok(Quantum(self.0 / rhs))
    }

    /// Konversi integer aman dari representasi desimal string tanpa floating point.
    pub fn from_aur_str(s: &str) -> Result<Self, MonetaryError> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.is_empty() || parts.len() > 2 {
            return Err(MonetaryError::InvalidPrecision);
        }

        let whole: u128 = parts[0].parse().map_err(|_| MonetaryError::InvalidPrecision)?;
        let whole_quanta = whole.checked_mul(QUANTA_PER_AUR).ok_or(MonetaryError::Overflow)?;

        if parts.len() == 1 {
            return Ok(Quantum(whole_quanta));
        }

        let frac_str = parts[1];
        if frac_str.len() > 9 {
            return Err(MonetaryError::InvalidPrecision);
        }

        let mut padded = [b'0'; 9];
        for (i, &b) in frac_str.as_bytes().iter().enumerate() {
            padded[i] = b;
        }

        let padded_str = std::str::from_utf8(&padded).map_err(|_| MonetaryError::InvalidPrecision)?;
        let frac: u128 = padded_str.parse().map_err(|_| MonetaryError::InvalidPrecision)?;

        let total = whole_quanta.checked_add(frac).ok_or(MonetaryError::Overflow)?;
        Ok(Quantum(total))
    }

    /// Format tampilan ramah manusia tanpa konversi float.
    pub fn to_aur_string(&self) -> String {
        let whole = self.0 / QUANTA_PER_AUR;
        let frac = self.0 % QUANTA_PER_AUR;
        format!("{}.{:09} AUR", whole, frac)
    }
}

impl fmt::Display for Quantum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_aur_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantum_scale_9_decimals() {
        assert_eq!(QUANTA_PER_AUR, 1_000_000_000);
        assert_eq!(MAX_SUPPLY_QUANTA, 66_000_000_000_000_000);
        assert_eq!(Quantum::ONE_AUR.as_u128(), 1_000_000_000);
        assert_eq!(Quantum::MAX_SUPPLY.as_u128(), 66_000_000_000_000_000);
    }

    #[test]
    fn test_from_aur_str_and_to_aur_string() {
        let q = Quantum::from_aur_str("1.500000000").unwrap();
        assert_eq!(q.as_u128(), 1_500_000_000);
        assert_eq!(q.to_aur_string(), "1.500000000 AUR");

        let q_zero = Quantum::from_aur_str("0").unwrap();
        assert_eq!(q_zero, Quantum::ZERO);

        let q_small = Quantum::from_aur_str("0.000000001").unwrap();
        assert_eq!(q_small.as_u128(), 1);

        // Reject 10 decimal places
        assert_eq!(
            Quantum::from_aur_str("1.0000000001"),
            Err(MonetaryError::InvalidPrecision)
        );
    }

    #[test]
    fn test_checked_arithmetic() {
        let a = Quantum::new(100);
        let b = Quantum::new(50);
        assert_eq!(a.checked_add(b).unwrap(), Quantum::new(150));
        assert_eq!(a.checked_sub(b).unwrap(), Quantum::new(50));
        assert_eq!(b.checked_sub(a), Err(MonetaryError::Underflow));
        assert_eq!(a.checked_mul(2).unwrap(), Quantum::new(200));
        assert_eq!(a.checked_div(2).unwrap(), Quantum::new(50));
        assert_eq!(a.checked_div(0), Err(MonetaryError::DivisionByZero));
    }
}
