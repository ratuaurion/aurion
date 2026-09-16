//! Set Instruksi Aurion Virtual Machine (AVM ISA).
//! Mematuhi Invariant AUR-VM-001 (Deterministik) & AUR-VM-002 (Zero-Float).

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OpcodeError {
    #[error("Unknown or invalid opcode: 0x{0:02x}")]
    InvalidOpcode(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    // 0x00 - 0x0F: Stop & Aritmetika Integer
    Stop = 0x00,
    Add = 0x01,
    Sub = 0x02,
    Mul = 0x03,
    Div = 0x04,
    Mod = 0x05,
    Not = 0x06,

    // 0x10 - 0x1F: Logika & Perbandingan
    Lt = 0x10,
    Gt = 0x11,
    Eq = 0x12,
    IsZero = 0x13,
    And = 0x14,
    Or = 0x15,
    Xor = 0x16,
    Shl = 0x17,
    Shr = 0x18,

    // 0x20 - 0x2F: Kriptografi
    Blake3 = 0x20,

    // 0x30 - 0x3F: Informasi Konteks
    Address = 0x30,
    Caller = 0x31,
    Origin = 0x32,
    CallValue = 0x33,
    GasLimit = 0x34,
    BlockHeight = 0x35,
    Timestamp = 0x36,

    // 0x50 - 0x5F: Stack, Memori, Storage, & Aliran Kontrol
    Pop = 0x50,
    MLoad = 0x51,
    MStore = 0x52,
    MStore8 = 0x53,
    SLoad = 0x54,
    SStore = 0x55,
    Jump = 0x56,
    Jumpi = 0x57,
    Pc = 0x58,
    MSize = 0x59,
    Gas = 0x5A,
    JumpDest = 0x5B,

    // 0x60 - 0x7F: Push Konstanta
    Push1 = 0x60,
    Push2 = 0x61,
    Push4 = 0x62,
    Push8 = 0x63,
    Push16 = 0x64,
    Push32 = 0x65,

    // 0x80 - 0x8F: Duplikasi Stack
    Dup1 = 0x80,
    Dup2 = 0x81,
    Dup3 = 0x82,
    Dup4 = 0x83,

    // 0x90 - 0x9F: Penukaran Stack
    Swap1 = 0x90,
    Swap2 = 0x91,
    Swap3 = 0x92,
    Swap4 = 0x93,

    // 0xA0 - 0xA4: Logging / Events
    Log0 = 0xA0,
    Log1 = 0xA1,
    Log2 = 0xA2,

    // 0xF0 - 0xFF: Terminasi
    Return = 0xF3,
    Revert = 0xFD,
    Invalid = 0xFE,
}

impl Opcode {
    pub fn from_u8(byte: u8) -> Result<Self, OpcodeError> {
        match byte {
            0x00 => Ok(Self::Stop),
            0x01 => Ok(Self::Add),
            0x02 => Ok(Self::Sub),
            0x03 => Ok(Self::Mul),
            0x04 => Ok(Self::Div),
            0x05 => Ok(Self::Mod),
            0x06 => Ok(Self::Not),

            0x10 => Ok(Self::Lt),
            0x11 => Ok(Self::Gt),
            0x12 => Ok(Self::Eq),
            0x13 => Ok(Self::IsZero),
            0x14 => Ok(Self::And),
            0x15 => Ok(Self::Or),
            0x16 => Ok(Self::Xor),
            0x17 => Ok(Self::Shl),
            0x18 => Ok(Self::Shr),

            0x20 => Ok(Self::Blake3),

            0x30 => Ok(Self::Address),
            0x31 => Ok(Self::Caller),
            0x32 => Ok(Self::Origin),
            0x33 => Ok(Self::CallValue),
            0x34 => Ok(Self::GasLimit),
            0x35 => Ok(Self::BlockHeight),
            0x36 => Ok(Self::Timestamp),

            0x50 => Ok(Self::Pop),
            0x51 => Ok(Self::MLoad),
            0x52 => Ok(Self::MStore),
            0x53 => Ok(Self::MStore8),
            0x54 => Ok(Self::SLoad),
            0x55 => Ok(Self::SStore),
            0x56 => Ok(Self::Jump),
            0x57 => Ok(Self::Jumpi),
            0x58 => Ok(Self::Pc),
            0x59 => Ok(Self::MSize),
            0x5A => Ok(Self::Gas),
            0x5B => Ok(Self::JumpDest),

            0x60 => Ok(Self::Push1),
            0x61 => Ok(Self::Push2),
            0x62 => Ok(Self::Push4),
            0x63 => Ok(Self::Push8),
            0x64 => Ok(Self::Push16),
            0x65 => Ok(Self::Push32),

            0x80 => Ok(Self::Dup1),
            0x81 => Ok(Self::Dup2),
            0x82 => Ok(Self::Dup3),
            0x83 => Ok(Self::Dup4),

            0x90 => Ok(Self::Swap1),
            0x91 => Ok(Self::Swap2),
            0x92 => Ok(Self::Swap3),
            0x93 => Ok(Self::Swap4),

            0xA0 => Ok(Self::Log0),
            0xA1 => Ok(Self::Log1),
            0xA2 => Ok(Self::Log2),

            0xF3 => Ok(Self::Return),
            0xFD => Ok(Self::Revert),
            0xFE => Ok(Self::Invalid),

            _ => Err(OpcodeError::InvalidOpcode(byte)),
        }
    }

    /// Biaya gas dasar untuk eksekusi instruksi.
    pub fn base_gas_cost(&self) -> u64 {
        match self {
            Self::Stop => 0,
            Self::Add | Self::Sub | Self::Not | Self::Lt | Self::Gt | Self::Eq | Self::IsZero
            | Self::And | Self::Or | Self::Xor | Self::Shl | Self::Shr | Self::Pop
            | Self::Pc | Self::MSize | Self::Gas | Self::JumpDest => 3,

            Self::Mul | Self::Div | Self::Mod => 5,

            Self::Push1 | Self::Push2 | Self::Push4 | Self::Push8 | Self::Push16 | Self::Push32
            | Self::Dup1 | Self::Dup2 | Self::Dup3 | Self::Dup4
            | Self::Swap1 | Self::Swap2 | Self::Swap3 | Self::Swap4 => 3,

            Self::MLoad | Self::MStore | Self::MStore8 => 3,

            Self::Address | Self::Caller | Self::Origin | Self::CallValue | Self::GasLimit
            | Self::BlockHeight | Self::Timestamp => 2,

            Self::Jump => 8,
            Self::Jumpi => 10,

            Self::Blake3 => 30,

            Self::SLoad => 100,
            Self::SStore => 500,

            Self::Log0 => 375,
            Self::Log1 => 750,
            Self::Log2 => 1125,

            Self::Return => 0,
            Self::Revert => 0,
            Self::Invalid => 0,
        }
    }
}
