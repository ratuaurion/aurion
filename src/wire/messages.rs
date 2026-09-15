//! Katalog tipe pesan protokol jaringan P2P Aurion.

pub const MSG_PING: u16 = 0x0001;
pub const MSG_PONG: u16 = 0x0002;
pub const MSG_STATUS: u16 = 0x0003;
pub const MSG_TX: u16 = 0x0010;
pub const MSG_BLOCK_PROPOSAL: u16 = 0x0020;
pub const MSG_BLOCK_VOTE: u16 = 0x0021;
pub const MSG_COMMIT_CERT: u16 = 0x0022;
pub const MSG_GET_BLOCKS: u16 = 0x0030;
pub const MSG_BLOCKS_RESPONSE: u16 = 0x0031;
