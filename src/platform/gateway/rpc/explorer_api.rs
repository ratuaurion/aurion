#![forbid(unsafe_code)]

//! Emulasi Handler REST `/api/v1/*` untuk Gateway Terpadu Aurion (Opsi B).
//!
//! Melayani kontrak endpoint yang dikonsumsi `aurion-explorer` tanpa memaksa
//! simpul validator menjadi blind broker: seluruh metrik bootnode dipetakan
//! secara semantik (`semantic bridging`) dari state internal Aurion.
//!
//! Mematuhi Invariant:
//! - AUR-ARCH-011: Absolute Zero Unsafe Code (`#![forbid(unsafe_code)]`).
//! - AUR-ARCH-012: Absolute Zero Float Arithmetic — seluruh angka di serialisasi
//!   sebagai integer murni (`u64`/`u32`) atau string, tanpa `f32`/`f64`.
//!
//! Endpoint didukung:
//! - `GET /api/v1/network/stats`
//! - `GET /api/v1/peers`
//! - `GET /api/v1/blocks/latest?limit=N`
//! - `GET /api/v1/transactions/recent?limit=N`

use crate::codec::CanonicalEncode;
use crate::consensus::bft::header::BlockHeader;
use crate::gateway::rpc::methods::{CommittedTxSummary, RpcContext};
use crate::runtime::config::NodeConfig;
use crate::transaction::types::{transaction_wire_size, Transaction};
use std::sync::atomic::Ordering;

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;

/// Parsing nilai `limit` dari query string path, dibatasi [1, MAX_LIMIT].
pub fn parse_limit(path: &str) -> usize {
    let raw = path.split('?').nth(1).and_then(|query| {
        query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            if key == "limit" {
                value.parse::<usize>().ok()
            } else {
                None
            }
        })
    });
    clamp_limit(raw)
}

fn clamp_limit(raw: Option<usize>) -> usize {
    match raw {
        Some(limit) if limit > 0 => limit.min(MAX_LIMIT),
        _ => DEFAULT_LIMIT,
    }
}

/// Render `GET /api/v1/network/stats` dengan pemetaan semantik bootnode.
pub fn render_network_stats(ctx: &RpcContext) -> String {
    let peers = ctx.metrics.connected_peers.load(Ordering::SeqCst) as u64;
    let height = ctx.current_height.load(Ordering::SeqCst);
    let total_tx = ctx.metrics.transactions_processed_total.load(Ordering::SeqCst);
    let votes = height.saturating_mul(4);

    format!(
        r#"{{"active_peers":{},"authenticated_peers":{},"total_handshakes_received":0,"total_handshakes_authenticated":0,"total_handshakes_rejected":0,"total_tx_relayed":{},"total_tx_dropped":0,"total_proposals_relayed":{},"total_votes_relayed":{},"pex_announces_received":0,"pex_queries_received":0,"uptime_secs":{}}}"#,
        peers,
        peers,
        total_tx,
        height,
        votes,
        ctx.node_uptime_secs()
    )
}

/// Render `GET /api/v1/peers` dari topologi genesis validator & bootnode resmi.
pub fn render_peers(ctx: &RpcContext) -> String {
    let connected = ctx.metrics.connected_peers.load(Ordering::SeqCst);
    let authenticated = connected > 0;
    let auth_str = if authenticated { "true" } else { "false" };
    let protocol_version = ctx.metrics.active_protocol_version.load(Ordering::SeqCst);

    let bootnodes = NodeConfig::mainnet_bootnodes();
    let mut entries = Vec::with_capacity(bootnodes.len());
    for (idx, peer) in bootnodes.iter().enumerate() {
        let role = if idx == 0 { "bootnode" } else { "validator" };
        entries.push(format!(
            r#"{{"peer_id":"{}","role":"{}","p2p_locator":"{}","protocol_version":{},"last_seen_unix_secs":0,"latency_ms":0,"reputation_score":0,"authenticated":{}}}"#,
            peer.public_key_hex,
            role,
            to_multiaddr(&peer.endpoint),
            protocol_version,
            auth_str
        ));
    }

    format!("[{}]", entries.join(","))
}

/// Render `GET /api/v1/blocks/latest?limit=N` dari katalog header in-memory.
///
/// Mutex `headers` dan `tx_counts` hanya di-lock sesaat untuk mengkloning data
/// kecil (header + tx_count), lalu dilepas sebelum serialisasi JSON agar tidak
/// menahan worker Tokio / bersaing lama dengan jalur konsensus (AUR-ISSUE-011).
pub fn render_latest_blocks(ctx: &RpcContext, limit: usize) -> String {
    let height = ctx.current_height.load(Ordering::SeqCst);
    if height == 0 {
        return r#"{"status":"ok","count":0,"blocks":[]}"#.to_string();
    }

    let count = limit.min(height.saturating_add(1) as usize);
    let start = height.saturating_sub(count.saturating_sub(1) as u64);

    let mut items: Vec<(u64, BlockHeader, u64)> = Vec::with_capacity(count);
    {
        let headers_guard = ctx.headers.lock().unwrap();
        let tx_counts_guard = ctx.tx_counts.lock().unwrap();
        for h in start..=height {
            if let Some(header) = headers_guard.get(&h) {
                let tx_count = tx_counts_guard.get(&h).copied().unwrap_or(0);
                items.push((h, header.clone(), tx_count));
            }
        }
    }

    let mut blocks_json = Vec::with_capacity(items.len());
    for (h, header, tx_count) in items {
        let block_hash = hex::encode(header.compute_block_hash().as_bytes());
        let prev_hash = hex::encode(header.prev_block_hash.as_bytes());
        blocks_json.push(format!(
            r#"{{"height":{},"hash":"0x{}","prev_hash":"0x{}","timestamp":{},"tx_count":{},"received_at":{}}}"#,
            h, block_hash, prev_hash, header.timestamp, tx_count, header.timestamp
        ));
    }

    format!(
        r#"{{"status":"ok","count":{},"blocks":[{}]}}"#,
        blocks_json.len(),
        blocks_json.join(",")
    )
}

/// Render `GET /api/v1/transactions/recent?limit=N` dari buffer transaksi
/// terkonfirmasi terkini, lalu dilengkapi transaksi pending mempool.
pub fn render_recent_transactions(ctx: &RpcContext, limit: usize) -> String {
    let mut txs_json = Vec::new();

    // Lock hanya sesaat untuk mengkloning ringkasan terbaru; serialisasi dilakukan
    // setelah mutex dilepas agar jalur konsensus tidak tertahan (AUR-ISSUE-011).
    let summaries: Vec<CommittedTxSummary> = {
        let recent = ctx.recent_transactions.lock().unwrap();
        let start_idx = recent.len().saturating_sub(limit);
        recent.iter().skip(start_idx).cloned().collect()
    };
    for summary in &summaries {
        txs_json.push(render_tx_summary(summary));
    }

    let remaining = limit.saturating_sub(txs_json.len());
    if remaining > 0 {
        let pending: Vec<(crate::core::Hash256, Transaction, u64)> = {
            let mempool = ctx.mempool.lock().unwrap();
            mempool
                .entries
                .iter()
                .take(remaining)
                .map(|(tx_id, entry)| (*tx_id, entry.tx.clone(), entry.admitted_timestamp))
                .collect()
        };
        for (tx_id, tx, admitted_timestamp) in pending {
            txs_json.push(format!(
                r#"{{"tx_hash":"0x{}","raw_payload":"{}","size_bytes":{},"received_at":{}}}"#,
                hex::encode(tx_id.as_bytes()),
                canonical_tx_hex(&tx),
                transaction_wire_size(tx.payload.len()),
                admitted_timestamp
            ));
        }
    }

    format!(
        r#"{{"status":"ok","count":{},"transactions":[{}]}}"#,
        txs_json.len(),
        txs_json.join(",")
    )
}

fn render_tx_summary(summary: &CommittedTxSummary) -> String {
    format!(
        r#"{{"tx_hash":"0x{}","raw_payload":"{}","size_bytes":{},"received_at":{},"block_height":{}}}"#,
        hex::encode(summary.tx_id.as_bytes()),
        canonical_tx_hex(&summary.tx),
        transaction_wire_size(summary.tx.payload.len()),
        summary.received_at,
        summary.height
    )
}

fn canonical_tx_hex(tx: &Transaction) -> String {
    let mut buf = Vec::with_capacity(transaction_wire_size(tx.payload.len()));
    tx.encode_canonical(&mut buf);
    hex::encode(buf)
}

/// Konversi endpoint `host:port` kanonikal Aurion ke multiaddr P2P.
fn to_multiaddr(endpoint: &str) -> String {
    if let Some((host, port)) = endpoint.rsplit_once(':') {
        if host.parse::<std::net::IpAddr>().is_ok() {
            format!("/ip4/{host}/tcp/{port}")
        } else {
            format!("/dns4/{host}/tcp/{port}")
        }
    } else {
        endpoint.to_string()
    }
}