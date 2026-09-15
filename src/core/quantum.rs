//! Quantum: Unit moneter atomik terkecil Aurion.
//! Mematuhi Invariant AUR-ARCH-012: Absolute Zero Floating-Point Arithmetic.

use std::fmt;
use thiserror::Error;

/// Unit atomik per 1 AUR (10^8 Quanta).
pub const QUANTA_PER_AUR: u128 = 100_000_000;

/// Batas maksimum suplai dalam satuan AUR (66 Juta).
pub const MAX_SUPPLY_AUR: u128 = 66_000_000;

/// Batas maksimum suplai dalam satuan Quantum (6,6 Kuadriliun).
pub const MAX_SUPPLY_QUANTA: u128 = 6_600_000_000_000_000;

/// Alokasi Creator dalam satuan Quantum (30%).
pub const CREATOR_ALLOCATION_QUANTA: u128 = 1_980_000_000_000_000;

/// Alokasi Developer dalam satuan Quantum (5%).
pub const DEVELOPER_ALLOCATION_QUANTA: u128 = 330_000_000_000_000;

/// Alokasi Community / Public (Pure Mining) dalam satuan Quantum (65%).
pub const COMMUNITY_MINING_QUANTA: u128 = 4_290_000_000_000_000;

/// Subsidi blok awal (Era 0) dalam satuan Quantum (10 AUR = 1.000.000.000 Q).
pub const INITIAL_BLOCK_SUBSIDY_QUANTA: u128 = 1_000_000_000;

/// Interval blok per era halving (~4 tahun pada target 60 detik).
pub const HALVING_INTERVAL_BLOCKS: u64 = 2_145_000;

/// Batas maksimum era halving sebelum subsidi menjadi nol.
pub const MAX_HALVING_ERAS: u64 = 30;

/// Waktu target produksi per blok dalam detik.
pub const TARGET_BLOCK_TIME_SECONDS: u64 = 60;

/// Ambang batas kematangan hadiah coinbase dalam jumlah blok.
pub const COINBASE_MATURITY_BLOCKS: u64 = 100;

/// Rasio pembakaran fee transaksi protokol (20%).
pub const FEE_BURN_PERCENTAGE: u128 = 20;

/// Rasio fee yang dialokasikan kepada produser blok (80%).
pub const FEE_MINER_PERCENTAGE: u128 = 80;

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
    #[error("Invalid precision: input exceeds 8 decimal places")]
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
        if quanta > MAX_SUPPLY_QUANTA {
            return Err(MonetaryError::SupplyCapExceeded(quanta));
        }
        Ok(Quantum(quanta))
    }

    #[inline]
    pub const fn as_u128(&self) -> u128 {
        self.0
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
        if frac_str.len() > 8 {
            return Err(MonetaryError::InvalidPrecision);
        }

        let mut padded = [b'0'; 8];
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
        format!("{}.{:08} AUR", whole, frac)
    }
}

impl fmt::Display for Quantum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_aur_string())
    }
}
