//! Mesin Eksekusi Interpreter Aurion VM (AVM Engine).
//! Mematuhi Invariant AUR-VM-001 (Deterministik), AUR-VM-002 (Zero-Float), & AUR-VM-004 (Rollback atomik).

use std::collections::HashMap;
use thiserror::Error;

use crate::core::Hash256;
use crate::crypto::blake3_hash;
use crate::vm::context::{Event, ExecutionContext};
use crate::vm::gas::{GasError, GasTracker};
use crate::vm::memory::{Memory, MemoryError};
use crate::vm::opcode::Opcode;
use crate::vm::stack::{Stack, StackError};
use crate::vm::verifier::{VerifiedContract, VerifierError};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VmError {
    #[error("Verification error: {0}")]
    Verification(#[from] VerifierError),
    #[error("Stack error: {0}")]
    Stack(#[from] StackError),
    #[error("Memory error: {0}")]
    Memory(#[from] MemoryError),
    #[error("Gas error: {0}")]
    Gas(#[from] GasError),
    #[error("Invalid jump destination to PC {0}")]
    InvalidJump(usize),
    #[error("Instruction pointer out of bounds: PC {pc}, len {len}")]
    PcOutOfBounds { pc: usize, len: usize },
    #[error("Explicit invalid instruction encountered")]
    InvalidInstruction,
    #[error("Execution reverted: {0}")]
    Reverted(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionResult {
    Success {
        gas_used: u64,
        return_data: Vec<u8>,
        events: Vec<Event>,
        storage_changes: HashMap<Hash256, Hash256>,
    },
    Revert {
        gas_used: u64,
        reason: String,
    },
    OutOfGas,
    Error(String),
}

pub struct AvmEngine;

impl AvmEngine {
    /// Eksekusi bytecode kontrak terverifikasi dalam konteks yang diberikan.
    pub fn execute(
        contract: &VerifiedContract,
        mut ctx: ExecutionContext,
        initial_storage: &HashMap<Hash256, Hash256>,
    ) -> ExecutionResult {
        let mut gas = GasTracker::new(ctx.gas_limit);
        let mut stack = Stack::new();
        let mut memory = Memory::new();
        let mut storage_changes: HashMap<Hash256, Hash256> = HashMap::new();
        let mut pc = 0;

        let bytecode = &contract.bytecode;

        while pc < bytecode.len() {
            let byte = bytecode[pc];
            let opcode = match Opcode::from_u8(byte) {
                Ok(op) => op,
                Err(_) => return ExecutionResult::Error(format!("Illegal opcode at PC {pc}")),
            };

            // 1. Konsumsi base gas
            if let Err(e) = gas.consume(opcode.base_gas_cost()) {
                return match e {
                    GasError::OutOfGas { .. } => ExecutionResult::OutOfGas,
                    _ => ExecutionResult::Error(e.to_string()),
                };
            }

            // 2. Eksekusi instruksi
            match opcode {
                Opcode::Stop => {
                    return ExecutionResult::Success {
                        gas_used: gas.gas_consumed(),
                        return_data: Vec::new(),
                        events: ctx.events,
                        storage_changes,
                    };
                }

                // Aritmetika Integer Murni
                Opcode::Add => {
                    let a = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let b = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let res = a.wrapping_add(b);
                    if let Err(e) = stack.push_u128(res) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Sub => {
                    let a = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let b = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let res = a.wrapping_sub(b);
                    if let Err(e) = stack.push_u128(res) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Mul => {
                    let a = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let b = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let res = a.wrapping_mul(b);
                    if let Err(e) = stack.push_u128(res) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Div => {
                    let a = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let b = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let res = a.checked_div(b).unwrap_or(0);
                    if let Err(e) = stack.push_u128(res) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Mod => {
                    let a = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let b = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let res = a.checked_rem(b).unwrap_or(0);
                    if let Err(e) = stack.push_u128(res) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Not => {
                    let a = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let mut res = [0u8; 32];
                    for i in 0..32 { res[i] = !a[i]; }
                    if let Err(e) = stack.push(res) { return ExecutionResult::Error(e.to_string()); }
                }

                // Logika & Perbandingan
                Opcode::Lt => {
                    let a = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let b = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let res = if a < b { 1u64 } else { 0u64 };
                    if let Err(e) = stack.push_u64(res) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Gt => {
                    let a = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let b = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let res = if a > b { 1u64 } else { 0u64 };
                    if let Err(e) = stack.push_u64(res) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Eq => {
                    let a = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let b = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let res = if a == b { 1u64 } else { 0u64 };
                    if let Err(e) = stack.push_u64(res) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::IsZero => {
                    let a = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let res = if a == [0u8; 32] { 1u64 } else { 0u64 };
                    if let Err(e) = stack.push_u64(res) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::And => {
                    let a = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let b = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let mut res = [0u8; 32];
                    for i in 0..32 { res[i] = a[i] & b[i]; }
                    if let Err(e) = stack.push(res) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Or => {
                    let a = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let b = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let mut res = [0u8; 32];
                    for i in 0..32 { res[i] = a[i] | b[i]; }
                    if let Err(e) = stack.push(res) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Xor => {
                    let a = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let b = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let mut res = [0u8; 32];
                    for i in 0..32 { res[i] = a[i] ^ b[i]; }
                    if let Err(e) = stack.push(res) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Shl => {
                    let shift = match stack.pop_u64() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let val = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let res = if shift >= 128 { 0 } else { val << shift };
                    if let Err(e) = stack.push_u128(res) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Shr => {
                    let shift = match stack.pop_u64() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let val = match stack.pop_u128() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let res = if shift >= 128 { 0 } else { val >> shift };
                    if let Err(e) = stack.push_u128(res) { return ExecutionResult::Error(e.to_string()); }
                }

                // Kriptografi: Blake3
                Opcode::Blake3 => {
                    let offset = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let len = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };

                    let extra_gas = (len as u64).div_ceil(32) * 6;
                    if let Err(e) = gas.consume(extra_gas) {
                        return match e {
                            GasError::OutOfGas { .. } => ExecutionResult::OutOfGas,
                            _ => ExecutionResult::Error(e.to_string()),
                        };
                    }

                    let data = match memory.load(offset, len) {
                        Ok(d) => d,
                        Err(e) => return ExecutionResult::Error(e.to_string()),
                    };
                    let hash = blake3_hash(data);
                    if let Err(e) = stack.push(*hash.as_bytes()) { return ExecutionResult::Error(e.to_string()); }
                }

                // Konteks Eksekusi
                Opcode::Address => {
                    let mut bytes = [0u8; 32];
                    bytes.copy_from_slice(ctx.contract_address.as_bytes());
                    if let Err(e) = stack.push(bytes) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Caller => {
                    let mut bytes = [0u8; 32];
                    bytes.copy_from_slice(ctx.caller.as_bytes());
                    if let Err(e) = stack.push(bytes) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Origin => {
                    let mut bytes = [0u8; 32];
                    bytes.copy_from_slice(ctx.origin.as_bytes());
                    if let Err(e) = stack.push(bytes) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::CallValue => {
                    if let Err(e) = stack.push_u128(ctx.value.as_u128()) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::GasLimit => {
                    if let Err(e) = stack.push_u64(ctx.gas_limit) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::BlockHeight => {
                    if let Err(e) = stack.push_u64(ctx.block_height) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Timestamp => {
                    if let Err(e) = stack.push_u64(ctx.timestamp) { return ExecutionResult::Error(e.to_string()); }
                }

                // Stack, Memori, & Storage
                Opcode::Pop => {
                    if let Err(e) = stack.pop() { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::MLoad => {
                    let offset = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let old_size = memory.size();
                    let word = match memory.load_word(offset) {
                        Ok(w) => w,
                        Err(e) => return ExecutionResult::Error(e.to_string()),
                    };
                    let exp_gas = GasTracker::calculate_memory_expansion_gas(old_size, memory.size());
                    if gas.consume(exp_gas).is_err() { return ExecutionResult::OutOfGas; }
                    if let Err(e) = stack.push(word) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::MStore => {
                    let offset = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let val = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let old_size = memory.size();
                    if let Err(e) = memory.store(offset, &val) { return ExecutionResult::Error(e.to_string()); }
                    let exp_gas = GasTracker::calculate_memory_expansion_gas(old_size, memory.size());
                    if gas.consume(exp_gas).is_err() { return ExecutionResult::OutOfGas; }
                }

                Opcode::MStore8 => {
                    let offset = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let val = match stack.pop_u64() { Ok(v) => (v & 0xFF) as u8, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let old_size = memory.size();
                    if let Err(e) = memory.store(offset, &[val]) { return ExecutionResult::Error(e.to_string()); }
                    let exp_gas = GasTracker::calculate_memory_expansion_gas(old_size, memory.size());
                    if gas.consume(exp_gas).is_err() { return ExecutionResult::OutOfGas; }
                }

                Opcode::SLoad => {
                    let key_bytes = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let key = Hash256::from_bytes(key_bytes);
                    let val = storage_changes
                        .get(&key)
                        .copied()
                        .or_else(|| initial_storage.get(&key).copied())
                        .unwrap_or(Hash256::ZERO);
                    if let Err(e) = stack.push(*val.as_bytes()) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::SStore => {
                    let key_bytes = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let val_bytes = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let key = Hash256::from_bytes(key_bytes);
                    let val = Hash256::from_bytes(val_bytes);
                    storage_changes.insert(key, val);
                }

                // Aliran Kontrol: Jump & Jumpi
                Opcode::Jump => {
                    let target = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    if !contract.valid_jump_dests.contains(&target) {
                        return ExecutionResult::Error(format!("Invalid jump to non-JUMPDEST PC {target}"));
                    }
                    pc = target;
                    continue;
                }

                Opcode::Jumpi => {
                    let target = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let cond = match stack.pop() { Ok(v) => v, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    if cond != [0u8; 32] {
                        if !contract.valid_jump_dests.contains(&target) {
                            return ExecutionResult::Error(format!("Invalid jumpi to non-JUMPDEST PC {target}"));
                        }
                        pc = target;
                        continue;
                    }
                }

                Opcode::Pc => {
                    if let Err(e) = stack.push_u64(pc as u64) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::MSize => {
                    if let Err(e) = stack.push_u64(memory.size() as u64) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::Gas => {
                    if let Err(e) = stack.push_u64(gas.gas_remaining()) { return ExecutionResult::Error(e.to_string()); }
                }

                Opcode::JumpDest => {
                    // No-op marker
                }

                // Push Konstanta
                Opcode::Push1 | Opcode::Push2 | Opcode::Push4 | Opcode::Push8 | Opcode::Push16 | Opcode::Push32 => {
                    let bytes_len = match opcode {
                        Opcode::Push1 => 1,
                        Opcode::Push2 => 2,
                        Opcode::Push4 => 4,
                        Opcode::Push8 => 8,
                        Opcode::Push16 => 16,
                        Opcode::Push32 => 32,
                        _ => 0,
                    };
                    let slice = &bytecode[pc + 1..pc + 1 + bytes_len];
                    let mut val = [0u8; 32];
                    val[32 - bytes_len..].copy_from_slice(slice);
                    if let Err(e) = stack.push(val) { return ExecutionResult::Error(e.to_string()); }
                    pc += bytes_len;
                }

                // DUP
                Opcode::Dup1 => { if let Err(e) = stack.dup(1) { return ExecutionResult::Error(e.to_string()); } }
                Opcode::Dup2 => { if let Err(e) = stack.dup(2) { return ExecutionResult::Error(e.to_string()); } }
                Opcode::Dup3 => { if let Err(e) = stack.dup(3) { return ExecutionResult::Error(e.to_string()); } }
                Opcode::Dup4 => { if let Err(e) = stack.dup(4) { return ExecutionResult::Error(e.to_string()); } }

                // SWAP
                Opcode::Swap1 => { if let Err(e) = stack.swap(1) { return ExecutionResult::Error(e.to_string()); } }
                Opcode::Swap2 => { if let Err(e) = stack.swap(2) { return ExecutionResult::Error(e.to_string()); } }
                Opcode::Swap3 => { if let Err(e) = stack.swap(3) { return ExecutionResult::Error(e.to_string()); } }
                Opcode::Swap4 => { if let Err(e) = stack.swap(4) { return ExecutionResult::Error(e.to_string()); } }

                // Logging / Events
                Opcode::Log0 => {
                    let offset = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let len = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let data = match memory.load(offset, len) { Ok(d) => d.to_vec(), Err(e) => return ExecutionResult::Error(e.to_string()) };
                    ctx.emit_event(Vec::new(), data);
                }

                Opcode::Log1 => {
                    let offset = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let len = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let topic0 = match stack.pop() { Ok(v) => Hash256::from_bytes(v), Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let data = match memory.load(offset, len) { Ok(d) => d.to_vec(), Err(e) => return ExecutionResult::Error(e.to_string()) };
                    ctx.emit_event(vec![topic0], data);
                }

                Opcode::Log2 => {
                    let offset = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let len = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let topic0 = match stack.pop() { Ok(v) => Hash256::from_bytes(v), Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let topic1 = match stack.pop() { Ok(v) => Hash256::from_bytes(v), Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let data = match memory.load(offset, len) { Ok(d) => d.to_vec(), Err(e) => return ExecutionResult::Error(e.to_string()) };
                    ctx.emit_event(vec![topic0, topic1], data);
                }

                // Terminasi
                Opcode::Return => {
                    let offset = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let len = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let return_data = match memory.load(offset, len) { Ok(d) => d.to_vec(), Err(e) => return ExecutionResult::Error(e.to_string()) };
                    return ExecutionResult::Success {
                        gas_used: gas.gas_consumed(),
                        return_data,
                        events: ctx.events,
                        storage_changes,
                    };
                }

                Opcode::Revert => {
                    let offset = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let len = match stack.pop_u64() { Ok(v) => v as usize, Err(e) => return ExecutionResult::Error(e.to_string()) };
                    let reason_bytes = memory.load(offset, len).unwrap_or(&[]);
                    let reason = String::from_utf8_lossy(reason_bytes).to_string();
                    return ExecutionResult::Revert {
                        gas_used: gas.gas_consumed(),
                        reason,
                    };
                }

                Opcode::Invalid => {
                    return ExecutionResult::Error("Invalid opcode executed".to_string());
                }
            }

            pc += 1;
        }

        // Implicit STOP
        ExecutionResult::Success {
            gas_used: gas.gas_consumed(),
            return_data: Vec::new(),
            events: ctx.events,
            storage_changes,
        }
    }
}
