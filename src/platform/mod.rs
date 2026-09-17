//! Domain Platform, Layanan Host, & Kontrol Klien Aurion.
//! Memuat penyimpanan ACID redb, transport jaringan P2P Zenoh, server RPC & WebSocket,
//! manajemen dompet BIP-39/SLIP-0010, supervisor node runtime, dan antarmuka CLI.

pub mod audit;
pub mod cli;
pub mod conformance;
pub mod gateway;
pub mod runtime;
pub mod storage;
pub mod telemetry;
pub mod wallet;
pub mod wire;
