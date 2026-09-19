//! Katalog tipe pesan protokol jaringan P2P Aurion.
//! Mematuhi Dokumen 06 (AURION-SERIALIZATION-AND-WIRE-PROTOCOL.md Bagian 6).

use crate::transaction::types::MAX_TRANSACTION_WIRE_BYTES;

// Handshake & Heartbeat
pub const MSG_HANDSHAKE_HELLO: u16 = 0x0001;
pub const MSG_HANDSHAKE_ACK: u16 = 0x0002;
pub const MSG_PING: u16 = 0x0003;
pub const MSG_PONG: u16 = 0x0004;

// Peer Discovery & Exchange (PEX)
pub const MSG_GET_PEERS: u16 = 0x0010;
pub const MSG_PEERS_ADDR: u16 = 0x0011;

// BFT Consensus Fast-Path
pub const MSG_BFT_PROPOSAL: u16 = 0x0020;
pub const MSG_BFT_PREVOTE: u16 = 0x0021;
pub const MSG_BFT_PRECOMMIT: u16 = 0x0022;
pub const MSG_BFT_COMMIT_CERT: u16 = 0x0023;

// Mempool & Transaction Propagation
pub const MSG_TX_GOSSIP: u16 = 0x0030;
pub const MSG_MEMPOOL_INV: u16 = 0x0031;

// Block Synchronization
pub const MSG_SYNC_GET_HEADERS: u16 = 0x0040;
pub const MSG_SYNC_HEADERS: u16 = 0x0041;
pub const MSG_SYNC_GET_BLOCK: u16 = 0x0042;
pub const MSG_SYNC_BLOCK: u16 = 0x0043;

// Aliases for compatibility
pub const MSG_STATUS: u16 = MSG_HANDSHAKE_ACK;
pub const MSG_TX: u16 = MSG_TX_GOSSIP;
pub const MSG_BLOCK_PROPOSAL: u16 = MSG_BFT_PROPOSAL;
pub const MSG_BLOCK_VOTE: u16 = MSG_BFT_PREVOTE;
pub const MSG_COMMIT_CERT: u16 = MSG_BFT_COMMIT_CERT;
pub const MSG_GET_BLOCKS: u16 = MSG_SYNC_GET_HEADERS;
pub const MSG_BLOCKS_RESPONSE: u16 = MSG_SYNC_HEADERS;

/// Plafon payload kanonikal untuk pesan `MSG_TX_GOSSIP`.
///
/// Diturunkan langsung dari schema transaksi (`MAX_TRANSACTION_WIRE_BYTES` =
/// 184 B basis + 4 B prefiks `payload_len` + 24.576 B payload = 24.764 B),
/// bukan lagi konstanta 68 KiB yang tidak sinkron.
pub const MAX_TX_WIRE_SIZE: usize = MAX_TRANSACTION_WIRE_BYTES;

/// Mengembalikan `true` bila `message_type` termasuk katalog protokol kanonikal.
pub fn is_known_message_type(message_type: u16) -> bool {
    matches!(
        message_type,
        MSG_HANDSHAKE_HELLO
            | MSG_HANDSHAKE_ACK
            | MSG_PING
            | MSG_PONG
            | MSG_GET_PEERS
            | MSG_PEERS_ADDR
            | MSG_BFT_PROPOSAL
            | MSG_BFT_PREVOTE
            | MSG_BFT_PRECOMMIT
            | MSG_BFT_COMMIT_CERT
            | MSG_TX_GOSSIP
            | MSG_MEMPOOL_INV
            | MSG_SYNC_GET_HEADERS
            | MSG_SYNC_HEADERS
            | MSG_SYNC_GET_BLOCK
            | MSG_SYNC_BLOCK
    )
}

/// Plafon ukuran memori maksimum per tipe pesan (Anti-OOM & Anti-DoS).
///
/// Tipe pesan tidak dikenal mengembalikan `0`; parser menolaknya lebih awal
/// melalui [`is_known_message_type`], sehingga tidak ada rute fallback implisit.
pub fn max_payload_bound(message_type: u16) -> usize {
    match message_type {
        MSG_HANDSHAKE_HELLO | MSG_HANDSHAKE_ACK => 512,
        MSG_PING | MSG_PONG => 8,
        MSG_GET_PEERS => 0,
        MSG_PEERS_ADDR => 64 * 1024,
        MSG_BFT_PROPOSAL => 4 * 1024 * 1024,
        MSG_BFT_PREVOTE | MSG_BFT_PRECOMMIT => 128,
        MSG_BFT_COMMIT_CERT => 256 * 1024,
        MSG_TX_GOSSIP => MAX_TX_WIRE_SIZE,
        MSG_MEMPOOL_INV => 1024 * 1024,
        MSG_SYNC_GET_HEADERS => 128,
        MSG_SYNC_HEADERS => 256 * 1024,
        MSG_SYNC_GET_BLOCK => 32,
        MSG_SYNC_BLOCK => 4 * 1024 * 1024,
        _ => 0, // Tipe tidak dikenal: tanpa alokasi yang diizinkan.
    }
}
