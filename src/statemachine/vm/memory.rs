//! Memori Linear Aurion VM.
//! Mematuhi batas maksimum 1 MB (1.048.576 bytes) per konteks eksekusi.

use thiserror::Error;

pub const MAX_MEMORY_BYTES: usize = 1024 * 1024; // 1 MB

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MemoryError {
    #[error("Memory limit exceeded: max 1 MB")]
    LimitExceeded,
    #[error("Memory access out of bounds: offset {offset}, len {len}")]
    OutOfBounds { offset: usize, len: usize },
}

#[derive(Debug, Clone, Default)]
pub struct Memory {
    data: Vec<u8>,
}

impl Memory {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Perluas memori jika offset + len melebihi ukuran saat ini.
    /// Mengembalikan ukuran memori baru dalam bytes jika terjadi ekspansi.
    pub fn ensure_capacity(
        &mut self,
        offset: usize,
        len: usize,
    ) -> Result<Option<(usize, usize)>, MemoryError> {
        if len == 0 {
            return Ok(None);
        }

        let required = offset.checked_add(len).ok_or(MemoryError::LimitExceeded)?;
        if required > MAX_MEMORY_BYTES {
            return Err(MemoryError::LimitExceeded);
        }

        if required > self.data.len() {
            let old_size = self.data.len();
            // Selaraskan ke kelipatan 32 byte (word alignment)
            let aligned = required.div_ceil(32) * 32;
            self.data.resize(aligned, 0);
            Ok(Some((old_size, aligned)))
        } else {
            Ok(None)
        }
    }

    /// Tulis data ke memori pada offset tertentu.
    pub fn store(&mut self, offset: usize, bytes: &[u8]) -> Result<(), MemoryError> {
        self.ensure_capacity(offset, bytes.len())?;
        self.data[offset..offset + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    /// Baca slice dari memori pada offset tertentu.
    pub fn load(&self, offset: usize, len: usize) -> Result<&[u8], MemoryError> {
        let end = offset.checked_add(len).ok_or(MemoryError::LimitExceeded)?;
        if end > self.data.len() {
            return Err(MemoryError::OutOfBounds { offset, len });
        }
        Ok(&self.data[offset..end])
    }

    /// Baca 32-byte word dari offset tertentu (padding dengan nol jika di luar batas).
    pub fn load_word(&mut self, offset: usize) -> Result<[u8; 32], MemoryError> {
        self.ensure_capacity(offset, 32)?;
        let mut word = [0u8; 32];
        word.copy_from_slice(&self.data[offset..offset + 32]);
        Ok(word)
    }
}
