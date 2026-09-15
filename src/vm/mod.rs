#![forbid(unsafe_code)]

//! Aurion Native Virtual Machine (AVM) Subsystem.
//! Mematuhi Dokumen Aturan Aplikasi 16 (16-SMART-CONTRACT-EXECUTION-SPECIFICATION.md)
//! dan Invariant AUR-VM-001 s.d AUR-VM-010.

pub mod context;
pub mod engine;
pub mod gas;
pub mod memory;
pub mod opcode;
pub mod stack;
pub mod verifier;

pub use context::{Event, ExecutionContext, MAX_CALL_DEPTH};
pub use engine::{AvmEngine, ExecutionResult, VmError};
pub use gas::{GasError, GasTracker};
pub use memory::{Memory, MemoryError, MAX_MEMORY_BYTES};
pub use opcode::{Opcode, OpcodeError};
pub use stack::{Stack, StackError, STACK_CAPACITY};
pub use verifier::{BytecodeVerifier, VerifiedContract, VerifierError, MAX_BYTECODE_SIZE};
