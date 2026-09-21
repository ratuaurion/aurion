//! Lapisan Transport Jaringan P2P Berbasis Zenoh 1.1 untuk Aurion.
//! Menghubungkan Frame Wire Kanonikal 52-Byte Aurion ke Mesh Pub/Sub Zenoh.
//! Mendukung Peer-to-Peer, NAT-Traversal, dan Interoperabilitas Bootnode.

use crate::codec::{CanonicalDecode, CanonicalEncode};
use crate::core::Hash256;
use crate::crypto::Keypair;
use crate::genesis::builder::GENESIS_CHAIN_ID;
use crate::wire::frame::{parse_network_frame, serialize_network_frame, WireError, WireFrameHeader};
use crate::wire::handshake::{
    validate_handshake_ack, HandshakeAck, HandshakeError, HandshakeHello,
};
use crate::wire::messages::{
    MSG_BFT_COMMIT_CERT, MSG_BFT_PRECOMMIT, MSG_BFT_PREVOTE, MSG_BFT_PROPOSAL, MSG_GET_PEERS,
    MSG_HANDSHAKE_ACK, MSG_HANDSHAKE_HELLO, MSG_PEERS_ADDR, MSG_SYNC_BLOCK, MSG_SYNC_GET_BLOCK,
    MSG_SYNC_GET_HEADERS, MSG_SYNC_HEADERS, MSG_TX_GOSSIP,
};
use rand::RngCore;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use zenoh::config::Config as ZenohConfig;
use zenoh::key_expr::KeyExpr;
use zenoh::Session;

/// Batas waktu tunggu balasan `HANDSHAKE_ACK` dari bootnode (detik).
pub const HANDSHAKE_ACK_TIMEOUT_SECS: u64 = 10;

/// Peran node kanonikal (selaras `NodeRole::FullNode` pada bootnode).
pub const PEER_ROLE_FULL_NODE: u8 = 0x02;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Zenoh network session error: {0}")]
    Zenoh(#[from] zenoh::Error),
    #[error("Canonical wire protocol framing error: {0}")]
    Wire(#[from] WireError),
    #[error("Invalid key expression format: {0}")]
    KeyExpr(String),
    #[error("Mutual handshake failed: {0}")]
    Handshake(#[from] HandshakeError),
    #[error("Timed out waiting for bootnode HANDSHAKE_ACK")]
    HandshakeTimeout,
    #[error("Node identity key I/O error: {0}")]
    IdentityIo(String),
    #[error("Invalid node identity key: {0}")]
    InvalidIdentity(String),
    #[error("Peer announce serialization error: {0}")]
    Serialization(String),
}

/// Memuat keypair identitas node yang persisten, atau membangkitkan dan
/// menyimpannya ke disk pada proses pertama (seed 32-byte heksadesimal).
pub fn load_or_create_identity(path: &Path) -> Result<Keypair, TransportError> {
    if path.exists() {
        let contents =
            std::fs::read_to_string(path).map_err(|e| TransportError::IdentityIo(e.to_string()))?;
        let mut seed = [0u8; 32];
        hex::decode_to_slice(contents.trim(), &mut seed)
            .map_err(|e| TransportError::InvalidIdentity(e.to_string()))?;
        return Ok(Keypair::from_seed(&seed));
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| TransportError::IdentityIo(e.to_string()))?;
        }
    }

    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let keypair = Keypair::from_seed(&seed);
    std::fs::write(path, hex::encode(seed))
        .map_err(|e| TransportError::IdentityIo(e.to_string()))?;
    Ok(keypair)
}

fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Konfigurasi endpoint dan mode transport simpul Aurion.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub chain_id: u32,
    pub is_peer: bool,
    pub listen_endpoints: Vec<String>,
    pub connect_endpoints: Vec<String>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            chain_id: GENESIS_CHAIN_ID, // Canonical Mainnet Chain ID
            is_peer: true,
            listen_endpoints: vec!["tcp/0.0.0.0:9000".to_string()],
            connect_endpoints: vec![], // Diisi alamat aurion-bootnode jika ada
        }
    }
}

/// Peta ekspresi kunci (Key Expressions) kanonikal untuk topik jaringan Aurion.
#[derive(Debug, Clone)]
pub struct AurionKeyExpressions {
    pub prefix: String,
    pub handshake_hello: String,
    pub handshake_ack: String,
    pub bft_proposal: String,
    pub bft_votes: String,
    pub bft_commit_cert: String,
    pub mempool_tx: String,
    pub sync_headers: String,
    pub sync_blocks: String,
    pub pex_announce: String,
    pub pex_query: String,
    pub pex_response: String,
}

impl AurionKeyExpressions {
    pub fn new(chain_id: u32) -> Self {
        let prefix = format!("aurion/net/{chain_id}");
        Self {
            handshake_hello: format!("{prefix}/handshake/hello"),
            handshake_ack: format!("{prefix}/handshake/ack"),
            bft_proposal: format!("{prefix}/bft/proposal"),
            bft_votes: format!("{prefix}/bft/votes"),
            bft_commit_cert: format!("{prefix}/bft/cert"),
            mempool_tx: format!("{prefix}/mempool/tx"),
            sync_headers: format!("{prefix}/sync/headers"),
            sync_blocks: format!("{prefix}/sync/blocks"),
            pex_announce: format!("{prefix}/pex/announce"),
            pex_query: format!("{prefix}/pex/query"),
            pex_response: format!("{prefix}/pex/response"),
            prefix,
        }
    }

    pub fn key_for_message_type(&self, msg_type: u16) -> &str {
        match msg_type {
            MSG_HANDSHAKE_HELLO => &self.handshake_hello,
            MSG_HANDSHAKE_ACK => &self.handshake_ack,
            MSG_BFT_PROPOSAL => &self.bft_proposal,
            MSG_BFT_PREVOTE | MSG_BFT_PRECOMMIT => &self.bft_votes,
            MSG_BFT_COMMIT_CERT => &self.bft_commit_cert,
            MSG_TX_GOSSIP => &self.mempool_tx,
            MSG_SYNC_GET_HEADERS | MSG_SYNC_HEADERS => &self.sync_headers,
            MSG_SYNC_GET_BLOCK | MSG_SYNC_BLOCK => &self.sync_blocks,
            MSG_PEERS_ADDR => &self.pex_announce,
            MSG_GET_PEERS => &self.pex_query,
            _ => &self.prefix,
        }
    }
}

/// Manajer transport Zenoh untuk simpul P2P Aurion.
pub struct ZenohTransport {
    pub config: TransportConfig,
    pub keys: AurionKeyExpressions,
    pub session: Arc<Session>,
}

impl ZenohTransport {
    /// Inisialisasi sesi Zenoh dengan konfigurasi endpoints.
    pub async fn new(config: TransportConfig) -> Result<Self, TransportError> {
        let mut zenoh_cfg = ZenohConfig::default();

        if config.is_peer {
            zenoh_cfg
                .insert_json5("mode", r#""peer""#)
                .map_err(|e| TransportError::KeyExpr(format!("{e:?}")))?;
        } else {
            zenoh_cfg
                .insert_json5("mode", r#""client""#)
                .map_err(|e| TransportError::KeyExpr(format!("{e:?}")))?;
        }

        if !config.listen_endpoints.is_empty() {
            let formatted_endpoints: Vec<String> = config
                .listen_endpoints
                .iter()
                .map(|e| format!("{e:?}"))
                .collect();
            let json_endpoints = format!("[{}]", formatted_endpoints.join(","));
            zenoh_cfg
                .insert_json5("listen/endpoints", &json_endpoints)
                .map_err(|e| TransportError::KeyExpr(format!("{e:?}")))?;
        }

        if !config.connect_endpoints.is_empty() {
            let formatted_endpoints: Vec<String> = config
                .connect_endpoints
                .iter()
                .map(|e| format!("{e:?}"))
                .collect();
            let json_endpoints = format!("[{}]", formatted_endpoints.join(","));
            zenoh_cfg
                .insert_json5("connect/endpoints", &json_endpoints)
                .map_err(|e| TransportError::KeyExpr(format!("{e:?}")))?;
        }

        // Nonaktifkan multicast di WAN
        zenoh_cfg
            .insert_json5("scouting/multicast/enabled", "false")
            .map_err(|e| TransportError::KeyExpr(format!("{e:?}")))?;

        let session = zenoh::open(zenoh_cfg).await?;
        let keys = AurionKeyExpressions::new(config.chain_id);

        Ok(Self {
            config,
            keys,
            session: Arc::new(session),
        })
    }

    /// Siarkan pesan ke mesh P2P yang terbungkus oleh Wire Frame Kanonikal 52B Aurion.
    pub async fn broadcast_message(
        &self,
        message_type: u16,
        payload: &[u8],
    ) -> Result<(), TransportError> {
        let frame_bytes = serialize_network_frame(message_type, payload)?;
        let key_str = self.keys.key_for_message_type(message_type);
        let key_expr: KeyExpr = key_str
            .try_into()
            .map_err(|e| TransportError::KeyExpr(format!("{e:?}")))?;

        self.session.put(key_expr, frame_bytes).await?;
        Ok(())
    }

    /// Melakukan mutual handshake kanonikal dengan bootnode: mengirim
    /// `HANDSHAKE_HELLO` ke topik kanonikal dan menunggu `HANDSHAKE_ACK` yang
    /// tervalidasi kriptografis sebelum sesi dianggap terautentikasi.
    pub async fn perform_handshake(
        &self,
        keypair: &Keypair,
        genesis_hash: &Hash256,
        best_height: u64,
    ) -> Result<(), TransportError> {
        let ack_subscriber = self
            .session
            .declare_subscriber(&self.keys.handshake_ack)
            .await?;

        let hello = HandshakeHello::new(
            self.config.chain_id,
            *genesis_hash,
            best_height,
            current_unix_secs(),
            keypair,
        );
        let frame_bytes =
            serialize_network_frame(MSG_HANDSHAKE_HELLO, &hello.to_canonical_bytes())?;
        let hello_key: KeyExpr = self
            .keys
            .handshake_hello
            .as_str()
            .try_into()
            .map_err(|e| TransportError::KeyExpr(format!("{e:?}")))?;
        self.session.put(hello_key, frame_bytes).await?;

        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(HANDSHAKE_ACK_TIMEOUT_SECS);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(TransportError::HandshakeTimeout);
            }
            match tokio::time::timeout(remaining, ack_subscriber.recv_async()).await {
                Ok(Ok(sample)) => {
                    let raw = sample.payload().to_bytes();
                    if let Ok((_, payload)) = parse_network_frame(&raw) {
                        let mut cursor = 0usize;
                        if let Ok(ack) = HandshakeAck::decode_canonical(payload, &mut cursor) {
                            validate_handshake_ack(&ack, current_unix_secs())?;
                            return Ok(());
                        }
                    }
                }
                Ok(Err(_)) => return Err(TransportError::HandshakeTimeout),
                Err(_) => return Err(TransportError::HandshakeTimeout),
            }
        }
    }

    /// Mengumumkan identitas node ke bootnode melalui topik PEX kanonikal.
    /// Hanya diterima bootnode setelah node lolos mutual handshake.
    pub async fn announce_peer_canonical(
        &self,
        keypair: &Keypair,
        locator: &str,
        role: u8,
    ) -> Result<(), TransportError> {
        let payload = serde_json::json!({
            "peer_id": keypair.derive_address().to_hex(),
            "role": role,
            "p2p_locator": locator,
            "protocol_version": 1u16,
            "telemetry_addr": serde_json::Value::Null,
        });
        let payload_bytes = serde_json::to_vec(&payload)
            .map_err(|e| TransportError::Serialization(e.to_string()))?;
        let frame_bytes = serialize_network_frame(MSG_PEERS_ADDR, &payload_bytes)?;
        let key_expr: KeyExpr = self
            .keys
            .pex_announce
            .as_str()
            .try_into()
            .map_err(|e| TransportError::KeyExpr(format!("{e:?}")))?;
        self.session.put(key_expr, frame_bytes).await?;
        Ok(())
    }

    /// Dekode dan verifikasi integritas data frame jaringan yang masuk dari Zenoh.
    pub fn unpack_incoming_frame(raw_bytes: &[u8]) -> Result<(WireFrameHeader, &[u8]), WireError> {
        parse_network_frame(raw_bytes)
    }
}
