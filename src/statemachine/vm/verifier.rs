//! Verifier Statis Bytecode Aurion VM (AVM).
//! Mematuhi Invariant AUR-VM-005 (Pre-Deployment Bytecode Verification).

use crate::vm::opcode::{Opcode, OpcodeError};
use std::collections::HashSet;
use thiserror::Error;

pub const MAX_BYTECODE_SIZE: usize = 24 * 1024; // 24 KB

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VerifierError {
    #[error("Bytecode exceeds maximum size of 24 KB: {0} bytes")]
    BytecodeTooLarge(usize),
    #[error("Bytecode is empty")]
    EmptyBytecode,
    #[error("Invalid opcode at PC {pc}: {source}")]
    InvalidOpcode { pc: usize, source: OpcodeError },
    #[error("Truncated PUSH data at PC {pc}: expected {expected} bytes, got {available}")]
    TruncatedPush {
        pc: usize,
        expected: usize,
        available: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedContract {
    pub bytecode: Vec<u8>,
    pub valid_jump_dests: HashSet<usize>,
}

pub struct BytecodeVerifier;

impl BytecodeVerifier {
    /// Verifikasi bytecode secara statis dan ekstrak tabel `JUMPDEST` yang sah.
    pub fn verify(bytecode: &[u8]) -> Result<VerifiedContract, VerifierError> {
        if bytecode.is_empty() {
            return Err(VerifierError::EmptyBytecode);
        }

        if bytecode.len() > MAX_BYTECODE_SIZE {
            return Err(VerifierError::BytecodeTooLarge(bytecode.len()));
        }

        let mut valid_jump_dests = HashSet::new();
        let mut pc = 0;

        while pc < bytecode.len() {
            let byte = bytecode[pc];
            let opcode = Opcode::from_u8(byte)
                .map_err(|e| VerifierError::InvalidOpcode { pc, source: e })?;

            if opcode == Opcode::JumpDest {
                valid_jump_dests.insert(pc);
            }

            // Tangani instruksi PUSH1 s/d PUSH32
            let push_bytes = match opcode {
                Opcode::Push1 => 1,
                Opcode::Push2 => 2,
                Opcode::Push4 => 4,
                Opcode::Push8 => 8,
                Opcode::Push16 => 16,
                Opcode::Push32 => 32,
                _ => 0,
            };

            if push_bytes > 0 {
                let remaining = bytecode.len().saturating_sub(pc + 1);
                if remaining < push_bytes {
                    return Err(VerifierError::TruncatedPush {
                        pc,
                        expected: push_bytes,
                        available: remaining,
                    });
                }
                pc += push_bytes;
            }

            pc += 1;
        }

        Ok(VerifiedContract {
            bytecode: bytecode.to_vec(),
            valid_jump_dests,
        })
    }
}
