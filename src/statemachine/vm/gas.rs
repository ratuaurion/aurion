//! Pelacak Gas & Model Ekonomi Eksekusi Aurion VM.
//! Mematuhi Invariant AUR-VM-003 (Strict Integer Gas Metering).

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GasError {
    #[error("Out of gas: required {required}, available {available}")]
    OutOfGas { required: u64, available: u64 },
    #[error("Gas counter overflow")]
    GasOverflow,
}

#[derive(Debug, Clone)]
pub struct GasTracker {
    limit: u64,
    consumed: u64,
}

impl GasTracker {
    pub fn new(limit: u64) -> Self {
        Self { limit, consumed: 0 }
    }

    #[inline]
    pub fn gas_limit(&self) -> u64 {
        self.limit
    }

    #[inline]
    pub fn gas_consumed(&self) -> u64 {
        self.consumed
    }

    #[inline]
    pub fn gas_remaining(&self) -> u64 {
        self.limit.saturating_sub(self.consumed)
    }

    /// Konsumsi sejumlah gas. Jika tidak mencukupi, kembalikan OutOfGas.
    pub fn consume(&mut self, amount: u64) -> Result<(), GasError> {
        let new_consumed = self
            .consumed
            .checked_add(amount)
            .ok_or(GasError::GasOverflow)?;

        if new_consumed > self.limit {
            return Err(GasError::OutOfGas {
                required: amount,
                available: self.gas_remaining(),
            });
        }

        self.consumed = new_consumed;
        Ok(())
    }

    /// Hitung biaya ekspansi memori kuadratik: (words * 3) + (words^2 / 512).
    pub fn calculate_memory_expansion_gas(current_bytes: usize, new_bytes: usize) -> u64 {
        if new_bytes <= current_bytes {
            return 0;
        }

        let current_words = (current_bytes as u64).div_ceil(32);
        let new_words = (new_bytes as u64).div_ceil(32);

        let current_cost = (current_words * 3) + ((current_words * current_words) / 512);
        let new_cost = (new_words * 3) + ((new_words * new_words) / 512);

        new_cost.saturating_sub(current_cost)
    }
}
