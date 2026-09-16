//! Stack Operan 256-bit Aurion VM.
//! Mematuhi Invariant AUR-VM-001 (Deterministik) & batas kapasitas maksimal 1024 elemen.

use thiserror::Error;

pub const STACK_CAPACITY: usize = 1024;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StackError {
    #[error("Stack overflow: maximum depth of 1024 reached")]
    StackOverflow,
    #[error("Stack underflow: insufficient elements on stack")]
    StackUnderflow,
    #[error("Invalid stack index: {0}")]
    InvalidIndex(usize),
}

#[derive(Debug, Clone, Default)]
pub struct Stack {
    data: Vec<[u8; 32]>,
}

impl Stack {
    pub fn new() -> Self {
        Self {
            data: Vec::with_capacity(64),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn push(&mut self, val: [u8; 32]) -> Result<(), StackError> {
        if self.data.len() >= STACK_CAPACITY {
            return Err(StackError::StackOverflow);
        }
        self.data.push(val);
        Ok(())
    }

    pub fn push_u64(&mut self, val: u64) -> Result<(), StackError> {
        let mut bytes = [0u8; 32];
        bytes[24..32].copy_from_slice(&val.to_be_bytes());
        self.push(bytes)
    }

    pub fn push_u128(&mut self, val: u128) -> Result<(), StackError> {
        let mut bytes = [0u8; 32];
        bytes[16..32].copy_from_slice(&val.to_be_bytes());
        self.push(bytes)
    }

    pub fn pop(&mut self) -> Result<[u8; 32], StackError> {
        self.data.pop().ok_or(StackError::StackUnderflow)
    }

    pub fn pop_u64(&mut self) -> Result<u64, StackError> {
        let bytes = self.pop()?;
        let mut u64_bytes = [0u8; 8];
        u64_bytes.copy_from_slice(&bytes[24..32]);
        Ok(u64::from_be_bytes(u64_bytes))
    }

    pub fn pop_u128(&mut self) -> Result<u128, StackError> {
        let bytes = self.pop()?;
        let mut u128_bytes = [0u8; 16];
        u128_bytes.copy_from_slice(&bytes[16..32]);
        Ok(u128::from_be_bytes(u128_bytes))
    }

    pub fn peek(&self) -> Result<&[u8; 32], StackError> {
        self.data.last().ok_or(StackError::StackUnderflow)
    }

    /// Duplikasi elemen ke-`n` dari atas stack (1-indexed: DUP1 = top stack).
    pub fn dup(&mut self, n: usize) -> Result<(), StackError> {
        if n == 0 || n > 16 || n > self.data.len() {
            return Err(StackError::StackUnderflow);
        }
        if self.data.len() >= STACK_CAPACITY {
            return Err(StackError::StackOverflow);
        }
        let item = self.data[self.data.len() - n];
        self.data.push(item);
        Ok(())
    }

    /// Tukar elemen teratas dengan elemen ke-`n + 1` (1-indexed: SWAP1 = swap top dengan top - 1).
    pub fn swap(&mut self, n: usize) -> Result<(), StackError> {
        if n == 0 || n > 16 || n >= self.data.len() {
            return Err(StackError::StackUnderflow);
        }
        let top_idx = self.data.len() - 1;
        let target_idx = top_idx - n;
        self.data.swap(top_idx, target_idx);
        Ok(())
    }
}
