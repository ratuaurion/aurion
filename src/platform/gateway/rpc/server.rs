//! Server JSON-RPC 2.0 dan WebSocket Simpul Aurion.
//! Mematuhi Dokumen 02 (02-RPC-API-RULES.md) dan Invariant AUR-ARCH-011 & AUR-ARCH-012.

use crate::gateway::rpc::methods::RpcContext;
use crate::gateway::rpc::pubsub::{SubscriptionManager, SubscriptionTopic};
use crate::gateway::rpc::types::{
    parse_json_rpc_request, serialize_json_rpc_response, JsonRpcError, JsonRpcId, JsonRpcResponse,
};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};

/// Server RPC dan WebSocket terpadu untuk Simpul Aurion.
pub struct RpcServer {
    pub context: Arc<RpcContext>,
    pub pubsub: Arc<SubscriptionManager>,
    pub bind_addr: String,
    shutdown_rx: Option<watch::Receiver<bool>>,
}

impl RpcServer {
    pub fn new(
        context: Arc<RpcContext>,
        pubsub: Arc<SubscriptionManager>,
        bind_addr: &str,
    ) -> Self {
        Self {
            context,
            pubsub,
            bind_addr: bind_addr.to_string(),
            shutdown_rx: None,
        }
    }

    pub fn with_shutdown(mut self, rx: watch::Receiver<bool>) -> Self {
        self.shutdown_rx = Some(rx);
        self
    }

    /// Menjalankan loop server TCP untuk melayani permintaan HTTP dan WebSocket.
    pub async fn run(self) -> Result<(), io::Error> {
        let listener = TcpListener::bind(&self.bind_addr).await?;
        let context = self.context;
        let pubsub = self.pubsub;
        let mut shutdown = self.shutdown_rx;

        loop {
            tokio::select! {
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((stream, addr)) => {
                            let ctx_clone = Arc::clone(&context);
                            let ps_clone = Arc::clone(&pubsub);
                            tokio::spawn(async move {
                                if let Err(_e) = handle_connection(stream, addr, ctx_clone, ps_clone).await {
                                    // Koneksi ditutup atau error jaringan biasa
                                }
                            });
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    }
                }
                _ = async {
                    if let Some(rx) = &mut shutdown {
                        let _ = rx.changed().await;
                    } else {
                        futures_pending().await;
                    }
                } => {
                    break;
                }
            }
        }

        Ok(())
    }
}

async fn futures_pending() {
    std::future::pending::<()>().await;
}

/// Menangani setiap koneksi TCP yang masuk.
async fn handle_connection(
    mut stream: TcpStream,
    _addr: SocketAddr,
    context: Arc<RpcContext>,
    pubsub: Arc<SubscriptionManager>,
) -> Result<(), io::Error> {
    let mut buffer = [0u8; 8192];
    let bytes_read = stream.read(&mut buffer).await?;
    if bytes_read == 0 {
        return Ok(());
    }

    let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);
    let mut lines = request_str.lines();
    let request_line = match lines.next() {
        Some(l) => l,
        None => return Ok(()),
    };

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Ok(());
    }
    let method = parts[0];
    let path = parts[1];

    // Deteksi WebSocket Upgrade
    let mut is_ws_upgrade = false;
    let mut ws_key: Option<String> = None;
    let mut content_length: usize = 0;

    for line in lines {
        let lower = line.to_lowercase();
        if lower.starts_with("upgrade:") && lower.contains("websocket") {
            is_ws_upgrade = true;
        } else if lower.starts_with("sec-websocket-key:") {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() == 2 {
                ws_key = Some(parts[1].trim().to_string());
            }
        } else if lower.starts_with("content-length:") {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() == 2 {
                content_length = parts[1].trim().parse::<usize>().unwrap_or(0);
            }
        }
    }

    if is_ws_upgrade {
        if let Some(key) = ws_key {
            return handle_websocket_upgrade(stream, &key, context, pubsub).await;
        }
    }

    // Penanganan CORS Preflight OPTIONS (NET-012)
    if method == "OPTIONS" {
        let response = "HTTP/1.1 204 No Content\r\n\
Access-Control-Allow-Origin: *\r\n\
Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
Access-Control-Allow-Headers: Content-Type, Authorization, X-Requested-With\r\n\
Access-Control-Max-Age: 86400\r\n\
Content-Length: 0\r\n\
Connection: close\r\n\r\n";
        stream.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    // Endpoint Health Check HTTP
    if method == "GET" && (path == "/healthz" || path == "/healthz/deep") {
        let body = r#"{"status":"OK","service":"aurion-rpc"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    // Endpoint Sandbox UI & Explorer Dashboard (NET-012)
    if method == "GET" && (path == "/sandbox" || path == "/explorer") {
        let html = crate::gateway::explorer::render_sandbox_html(context.chain_id);
        let response = format!(
            "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        );
        stream.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    // Endpoint REST Explorer Stats (NET-012)
    if method == "GET" && (path == "/explorer/stats" || path == "/explorer/summary") {
        let body = crate::gateway::explorer::render_explorer_stats(&context);
        let response = format!(
            "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    // Endpoint REST Explorer Block (NET-012)
    if method == "GET" && path.starts_with("/explorer/block/") {
        let target = &path["/explorer/block/".len()..];
        let height = if target == "latest" {
            Some(context.current_height.load(std::sync::atomic::Ordering::SeqCst))
        } else {
            target.parse::<u64>().ok()
        };

        if let Some(h) = height {
            if let Some(body) = crate::gateway::explorer::render_block_by_height(&context, h) {
                let response = format!(
                    "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).await?;
                return Ok(());
            }
        }

        let body = r#"{"error":"Block not found"}"#;
        let response = format!(
            "HTTP/1.1 404 Not Found\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    // Endpoint REST Explorer Transaction (NET-012)
    if method == "GET" && path.starts_with("/explorer/tx/") {
        let tx_hash = &path["/explorer/tx/".len()..];
        if let Some(body) = crate::gateway::explorer::render_tx_by_hash(&context, tx_hash) {
            let response = format!(
                "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await?;
            return Ok(());
        }

        let body = r#"{"error":"Transaction not found in mempool"}"#;
        let response = format!(
            "HTTP/1.1 404 Not Found\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    // Endpoint JSON-RPC 2.0 HTTP POST
    if method == "POST" {
        // Ambil body HTTP
        let body_start = request_str.find("\r\n\r\n").map(|idx| idx + 4).unwrap_or(0);
        let mut body = request_str[body_start..].to_string();

        // Jika body belum lengkap terbaca sesuai Content-Length
        if body.len() < content_length {
            let mut remaining = vec![0u8; content_length - body.len()];
            stream.read_exact(&mut remaining).await?;
            body.push_str(&String::from_utf8_lossy(&remaining));
        }

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let response_payload = match parse_json_rpc_request(&body) {
            Ok(req) => {
                let resp = context.dispatch(&req, current_time);
                serialize_json_rpc_response(&resp)
            }
            Err(_) => {
                let err_resp = JsonRpcResponse::error(
                    JsonRpcId::Null,
                    JsonRpcError::parse_error("Gagal mengurai payload JSON-RPC 2.0"),
                );
                serialize_json_rpc_response(&err_resp)
            }
        };

        let response = format!(
            "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: POST, GET, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization, X-Requested-With\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_payload.len(),
            response_payload
        );
        stream.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    // Default: Method Not Allowed
    let not_allowed = "HTTP/1.1 405 Method Not Allowed\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    stream.write_all(not_allowed.as_bytes()).await?;
    Ok(())
}

/// Menangani handshake WebSocket RFC 6455 dan loop streaming dua arah.
async fn handle_websocket_upgrade(
    mut stream: TcpStream,
    sec_key: &str,
    context: Arc<RpcContext>,
    pubsub: Arc<SubscriptionManager>,
) -> Result<(), io::Error> {
    let accept_key = compute_sec_websocket_accept(sec_key);
    let handshake_resp = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        accept_key
    );
    stream.write_all(handshake_resp.as_bytes()).await?;

    let (mut read_half, mut write_half) = stream.into_split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let mut client_subs = Vec::new();

    // Loop penanganan frame WebSocket
    let mut ws_reader = WsFrameReader::new();

    loop {
        tokio::select! {
            // Data pesan notifikasi yang akan dikirim ke klien
            Some(msg) = rx.recv() => {
                let frame = make_ws_text_frame(&msg);
                if write_half.write_all(&frame).await.is_err() {
                    break;
                }
            }
            // Data frame yang masuk dari klien
            res = ws_reader.read_frame(&mut read_half) => {
                match res {
                    Ok(Some(WsFrame::Text(payload))) => {
                        let current_time = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        match parse_json_rpc_request(&payload) {
                            Ok(req) => {
                                if req.method == "aur_subscribe" {
                                    let topic_name = req.params.first().map(|s| s.as_str()).unwrap_or_default();
                                    let topic_opt = SubscriptionTopic::parse(topic_name, None);

                                    let resp = if let Some(topic) = topic_opt {
                                        let sub_id = pubsub.subscribe(topic, tx.clone());
                                        client_subs.push(sub_id);
                                        JsonRpcResponse::success(req.id, format!("{sub_id}"))
                                    } else {
                                        JsonRpcResponse::error(
                                            req.id,
                                            JsonRpcError::invalid_params(format!("Topik langganan '{topic_name}' tidak valid"))
                                        )
                                    };
                                    let frame = make_ws_text_frame(&serialize_json_rpc_response(&resp));
                                    if write_half.write_all(&frame).await.is_err() {
                                        break;
                                    }
                                } else if req.method == "aur_unsubscribe" {
                                    let sub_id_str = req.params.first().map(|s| s.as_str()).unwrap_or_default();
                                    let sub_id = sub_id_str.parse::<u64>().unwrap_or(0);
                                    let unsubbed = pubsub.unsubscribe(sub_id);
                                    let resp = JsonRpcResponse::success(req.id, if unsubbed { "true".to_string() } else { "false".to_string() });
                                    let frame = make_ws_text_frame(&serialize_json_rpc_response(&resp));
                                    if write_half.write_all(&frame).await.is_err() {
                                        break;
                                    }
                                } else {
                                    // Panggilan RPC standar via WebSocket
                                    let resp = context.dispatch(&req, current_time);
                                    let frame = make_ws_text_frame(&serialize_json_rpc_response(&resp));
                                    if write_half.write_all(&frame).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Err(_) => {
                                let err_resp = JsonRpcResponse::error(
                                    JsonRpcId::Null,
                                    JsonRpcError::parse_error("Payload JSON-RPC 2.0 WS tidak valid"),
                                );
                                let frame = make_ws_text_frame(&serialize_json_rpc_response(&err_resp));
                                if write_half.write_all(&frame).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Ok(Some(WsFrame::Ping(data))) => {
                        let pong = make_ws_pong_frame(&data);
                        if write_half.write_all(&pong).await.is_err() {
                            break;
                        }
                    }
                    Ok(Some(WsFrame::Close)) => {
                        break;
                    }
                    Ok(None) => {
                        // EOF
                        break;
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        }
    }

    // Bersihkan seluruh langganan klien saat disconnect
    for sub_id in client_subs {
        pubsub.unsubscribe(sub_id);
    }

    Ok(())
}

enum WsFrame {
    Text(String),
    Ping(Vec<u8>),
    Close,
}

struct WsFrameReader {
    buf: Vec<u8>,
}

impl WsFrameReader {
    fn new() -> Self {
        Self { buf: Vec::new() }
    }

    async fn read_frame<R: AsyncReadExt + Unpin>(
        &mut self,
        reader: &mut R,
    ) -> Result<Option<WsFrame>, io::Error> {
        loop {
            if let Some(frame) = self.try_parse_frame()? {
                return Ok(Some(frame));
            }

            let mut tmp = [0u8; 4096];
            let n = reader.read(&mut tmp).await?;
            if n == 0 {
                if self.buf.is_empty() {
                    return Ok(None);
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Koneksi WS ditutup prematur",
                    ));
                }
            }
            self.buf.extend_from_slice(&tmp[..n]);
        }
    }

    fn try_parse_frame(&mut self) -> Result<Option<WsFrame>, io::Error> {
        if self.buf.len() < 2 {
            return Ok(None);
        }

        let b0 = self.buf[0];
        let b1 = self.buf[1];

        let opcode = b0 & 0x0F;
        let is_masked = (b1 & 0x80) != 0;
        let len_indicator = b1 & 0x7F;

        let mut offset = 2;
        let payload_len: usize = if len_indicator < 126 {
            len_indicator as usize
        } else if len_indicator == 126 {
            if self.buf.len() < offset + 2 {
                return Ok(None);
            }
            let len_bytes = [self.buf[offset], self.buf[offset + 1]];
            offset += 2;
            u16::from_be_bytes(len_bytes) as usize
        } else {
            if self.buf.len() < offset + 8 {
                return Ok(None);
            }
            let mut len_bytes = [0u8; 8];
            len_bytes.copy_from_slice(&self.buf[offset..offset + 8]);
            offset += 8;
            u64::from_be_bytes(len_bytes) as usize
        };

        let mask_key = if is_masked {
            if self.buf.len() < offset + 4 {
                return Ok(None);
            }
            let key = [
                self.buf[offset],
                self.buf[offset + 1],
                self.buf[offset + 2],
                self.buf[offset + 3],
            ];
            offset += 4;
            Some(key)
        } else {
            None
        };

        if self.buf.len() < offset + payload_len {
            return Ok(None);
        }

        let mut raw_payload = self.buf[offset..offset + payload_len].to_vec();
        let total_frame_size = offset + payload_len;
        self.buf.drain(..total_frame_size);

        if let Some(mask) = mask_key {
            for (i, byte) in raw_payload.iter_mut().enumerate() {
                *byte ^= mask[i % 4];
            }
        }

        match opcode {
            0x01 => {
                let text = String::from_utf8(raw_payload).map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("UTF-8 error: {e}"))
                })?;
                Ok(Some(WsFrame::Text(text)))
            }
            0x08 => Ok(Some(WsFrame::Close)),
            0x09 => Ok(Some(WsFrame::Ping(raw_payload))),
            0x0A => Ok(Some(WsFrame::Ping(Vec::new()))), // Pong received
            _ => Ok(Some(WsFrame::Text(String::new()))),
        }
    }
}

/// Menghasilkan RFC 6455 Text Frame dari string UTF-8.
fn make_ws_text_frame(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut frame = Vec::new();

    // FIN bit set (0x80) | Opcode Text (0x01)
    frame.push(0x81);

    if len < 126 {
        frame.push(len as u8);
    } else if len <= 65535 {
        frame.push(126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }

    frame.extend_from_slice(bytes);
    frame
}

/// Menghasilkan RFC 6455 Pong Frame.
fn make_ws_pong_frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::new();
    frame.push(0x8A); // FIN | Pong
    frame.push(payload.len().min(125) as u8);
    frame.extend_from_slice(payload);
    frame
}

/// Menghitung nilai header `Sec-WebSocket-Accept` sesuai RFC 6455.
fn compute_sec_websocket_accept(key: &str) -> String {
    let mut concatenated = key.trim().to_string();
    concatenated.push_str("258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let sha1_digest = sha1(concatenated.as_bytes());
    base64_encode(&sha1_digest)
}

/// Implementasi mandiri SHA-1 (FIPS PUB 180-4) tanpa dependensi pihak ketiga atau float.
#[allow(clippy::chunks_exact_to_as_chunks)]
fn sha1(input: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    let ml = (input.len() as u64).wrapping_mul(8);
    let mut msg = input.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&ml.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (i, &w_i) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };

            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w_i);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut result = [0u8; 20];
    result[0..4].copy_from_slice(&h0.to_be_bytes());
    result[4..8].copy_from_slice(&h1.to_be_bytes());
    result[8..12].copy_from_slice(&h2.to_be_bytes());
    result[12..16].copy_from_slice(&h3.to_be_bytes());
    result[16..20].copy_from_slice(&h4.to_be_bytes());
    result
}

/// Implementasi mandiri RFC 4648 Base64 Encoder tanpa dependensi eksternal.
fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i < input.len() {
        let b0 = input[i];
        let b1 = if i + 1 < input.len() { input[i + 1] } else { 0 };
        let b2 = if i + 2 < input.len() { input[i + 2] } else { 0 };

        let idx0 = (b0 >> 2) as usize;
        let idx1 = (((b0 & 0x03) << 4) | (b1 >> 4)) as usize;
        let idx2 = (((b1 & 0x0F) << 2) | (b2 >> 6)) as usize;
        let idx3 = (b2 & 0x3F) as usize;

        out.push(TABLE[idx0] as char);
        out.push(TABLE[idx1] as char);

        if i + 1 < input.len() {
            out.push(TABLE[idx2] as char);
        } else {
            out.push('=');
        }

        if i + 2 < input.len() {
            out.push(TABLE[idx3] as char);
        } else {
            out.push('=');
        }

        i += 3;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_accept_rfc6455_vector() {
        // Vektor Uji Resmi RFC 6455 Halaman 24:
        // Client Key: "dGhlIHNhbXBsZSBub25jZQ=="
        // Expected Accept: "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let accept = compute_sec_websocket_accept(key);
        assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }
}
