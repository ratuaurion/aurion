//! Lapisan Transport Jaringan P2P Berbasis Zenoh 1.1 untuk Aurion.
//! Menghubungkan Frame Wire Kanonikal 52-Byte Aurion ke Mesh Pub/Sub Zenoh.
//! Mendukung Peer-to-Peer, NAT-Traversal, dan Interoperabilitas Bootnode.

use crate::genesis::builder::GENESIS_CHAIN_ID;
use crate::wire::frame::{parse_network_frame, serialize_network_frame, WireError, WireFrameHeader};
use crate::wire::messages::{
    MSG_BFT_COMMIT_CERT, MSG_BFT_PRECOMMIT, MSG_BFT_PREVOTE, MSG_BFT_PROPOSAL, MSG_HANDSHAKE_ACK,
    MSG_HANDSHAKE_HELLO, MSG_SYNC_BLOCK, MSG_SYNC_GET_BLOCK, MSG_SYNC_GET_HEADERS,
    MSG_SYNC_HEADERS, MSG_TX_GOSSIP,
};
use std::sync::Arc;
use thiserror::Error;
use zenoh::config::Config as ZenohConfig;
use zenoh::key_expr::KeyExpr;
use zenoh::Session;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Zenoh network session error: {0}")]
    Zenoh(#[from] zenoh::Error),
    #[error("Canonical wire protocol framing error: {0}")]
    Wire(#[from] WireError),
    #[error("Invalid key expression format: {0}")]
    KeyExpr(String),
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
    pub peer_announce: String,
    pub peer_list: String,
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
            peer_announce: "aurion/net/v1/peers/announce".to_string(),
            peer_list: "aurion/net/v1/peers/list".to_string(),
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

    /// Mendaftarkan node ke bootnode via PEX (Peer Exchange) heartbeat announce.
    pub async fn announce_peer(&self, locator: &str, role: &str) -> Result<(), TransportError> {
        let payload = serde_json::json!({
            "locator": locator,
            "role": role,
        });
        let payload_bytes = payload.to_string().into_bytes();
        let key_expr: KeyExpr = self.keys.peer_announce
            .as_str()
            .try_into()
            .map_err(|e| TransportError::KeyExpr(format!("{e:?}")))?;

        self.session.put(key_expr, payload_bytes).await?;
        Ok(())
    }

    /// Mengambil daftar peer aktif dari bootnode.
    pub async fn query_active_peers(&self) -> Result<Vec<String>, TransportError> {
        let key_expr: KeyExpr = self.keys.peer_list
            .as_str()
            .try_into()
            .map_err(|e| TransportError::KeyExpr(format!("{e:?}")))?;

        let replies = self.session.get(key_expr).await?;
        let mut result = Vec::new();
        while let Ok(reply) = replies.recv_async().await {
            if let Ok(sample) = reply.result() {
                if let Ok(peers) = serde_json::from_slice::<Vec<serde_json::Value>>(&sample.payload().to_bytes()) {
                    for p in peers {
                        if let Some(loc) = p.get("locator").and_then(|l| l.as_str()) {
                            result.push(loc.to_string());
                        }
                    }
                }
            }
        }
        Ok(result)
    }

    /// Dekode dan verifikasi integritas data frame jaringan yang masuk dari Zenoh.
    pub fn unpack_incoming_frame(raw_bytes: &[u8]) -> Result<(WireFrameHeader, &[u8]), WireError> {
        parse_network_frame(raw_bytes)
    }
}
