//! Router dan Dispatcher Metode RPC Namespace "aur_*" Aurion.
//! Mematuhi Dokumen 02 (02-RPC-API-RULES.md Bagian 4).

use crate::codec::CanonicalDecode;
use crate::consensus::bft::Block;
use crate::consensus::certificate::CommitCertificate;
use crate::consensus::header::BlockHeader;
use crate::core::{Address, Hash256, Quantum};
use crate::crypto::{decode_address_bech32m, encode_address_bech32m};
use crate::gateway::faucet::FaucetDispenser;
use crate::gateway::rpc::consistency::ConsistencySelector;
use crate::gateway::rpc::errors::*;
use crate::gateway::rpc::types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::mempool::MempoolEngine;
use crate::state::account::Account;
use crate::transaction::types::Transaction;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Kapasitas buffer transaksi terkonfirmasi terkini untuk endpoint `/api/v1/transactions/recent`.
pub const RECENT_TX_CAPACITY: usize = 256;

/// Ringkasan transaksi terkonfirmasi untuk telemetri explorer (Zero-Float, integer murni).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedTxSummary {
    pub height: u64,
    pub tx_id: Hash256,
    pub tx: Transaction,
    pub received_at: u64,
}

/// Konteks state bersama untuk melayani query dan submit JSON-RPC 2.0.
pub struct RpcContext {
    pub chain_id: u32,
    pub current_height: Arc<AtomicU64>,
    pub finalized_height: Arc<AtomicU64>,
    pub mempool: Arc<Mutex<MempoolEngine>>,
    pub accounts: Arc<Mutex<HashMap<Address, Account>>>,
    pub headers: Arc<Mutex<HashMap<u64, BlockHeader>>>,
    pub certificates: Arc<Mutex<HashMap<u64, CommitCertificate>>>,
    pub faucet: Arc<Mutex<Option<FaucetDispenser>>>,
    pub metrics: Arc<crate::platform::telemetry::MetricsRegistry>,
    pub health: Arc<crate::platform::telemetry::HealthReporter>,
    pub process_started_at: Arc<AtomicU64>,
    pub tx_counts: Arc<Mutex<HashMap<u64, u64>>>,
    pub recent_transactions: Arc<Mutex<VecDeque<CommittedTxSummary>>>,
}

impl RpcContext {
    pub fn new(chain_id: u32) -> Self {
        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        Self {
            chain_id,
            current_height: Arc::new(AtomicU64::new(0)),
            finalized_height: Arc::new(AtomicU64::new(0)),
            mempool: Arc::new(Mutex::new(MempoolEngine::with_chain_id(
                crate::mempool::DEFAULT_MAX_MEMPOOL_CAPACITY,
                crate::mempool::DEFAULT_MEMPOOL_TTL_SECS,
                chain_id,
            ))),
            accounts: Arc::new(Mutex::new(HashMap::new())),
            headers: Arc::new(Mutex::new(HashMap::new())),
            certificates: Arc::new(Mutex::new(HashMap::new())),
            faucet: Arc::new(Mutex::new(None)),
            metrics: Arc::new(crate::platform::telemetry::MetricsRegistry::new(chain_id)),
            health: Arc::new(crate::platform::telemetry::HealthReporter::new(
                chain_id,
                crate::runtime::config::NodeRole::FullNode,
                0,
            )),
            process_started_at: Arc::new(AtomicU64::new(started_at)),
            tx_counts: Arc::new(Mutex::new(HashMap::new())),
            recent_transactions: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Durasi uptime proses simpul dalam detik (basis integer murni, Zero-Float).
    pub fn node_uptime_secs(&self) -> u64 {
        let started = self.process_started_at.load(Ordering::SeqCst);
        if started == 0 {
            return 0;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        now.saturating_sub(started)
    }

    /// Rekam ringkasan blok terkomit (tx_count & transaksi terbaru) untuk endpoint explorer.
    ///
    /// Idempoten: blok dengan tinggi yang sudah tercatat tidak diproses ulang,
    /// sehingga `sync_rpc_context` yang dijalankan berulang tidak menimbulkan duplikasi.
    pub fn record_committed_block(&self, block: &Block) {
        let height = block.height();
        let mut counts = self.tx_counts.lock().unwrap();
        if counts.contains_key(&height) {
            return;
        }
        counts.insert(height, block.transactions.len() as u64);
        drop(counts);

        let timestamp = block.header.timestamp;
        let mut recent = self.recent_transactions.lock().unwrap();
        for tx in &block.transactions {
            recent.push_back(CommittedTxSummary {
                height,
                tx_id: tx.compute_tx_id(),
                tx: tx.clone(),
                received_at: timestamp,
            });
        }
        while recent.len() > RECENT_TX_CAPACITY {
            recent.pop_front();
        }
    }

    pub fn attach_faucet(&self, dispenser: FaucetDispenser) {
        let mut f = self.faucet.lock().unwrap();
        *f = Some(dispenser);
    }

    /// Dispatcher tunggal memproses request JSON-RPC 2.0 dan mengembalikan response.
    pub fn dispatch(&self, request: &JsonRpcRequest, current_time: u64) -> JsonRpcResponse {
        let result = match request.method.as_str() {
            // Modul Rantai & Blok
            "aur_chainId" => self.handle_chain_id(),
            "aur_blockHeight" => self.handle_block_height(),
            "aur_getBlockByHeight" => self.handle_get_block_by_height(&request.params),
            "aur_getBlockByHash" => self.handle_get_block_by_hash(&request.params),
            "aur_getCommitCertificate" => self.handle_get_commit_certificate(&request.params),

            // Modul Transaksi & Mempool
            "aur_sendRawTransaction" => {
                self.handle_send_raw_transaction(&request.params, current_time)
            }
            "aur_getTransactionByHash" => self.handle_get_transaction_by_hash(&request.params),
            "aur_estimateFee" => self.handle_estimate_fee(),
            "aur_getMempool" => self.handle_get_mempool(),

            // Modul State Akun
            "aur_getBalance" => self.handle_get_balance(&request.params),
            "aur_getNonce" => self.handle_get_nonce(&request.params),
            "aur_getAccount" => self.handle_get_account(&request.params),

            // Modul Komunitas & Testnet (NET-012)
            "aur_getNetworkStats" => self.handle_get_network_stats(),
            "aur_requestFaucet" => self.handle_request_faucet(&request.params, current_time),

            _ => Err(method_not_found(&request.method)),
        };

        match result {
            Ok(res_json) => JsonRpcResponse::success(request.id.clone(), res_json),
            Err(err) => JsonRpcResponse::error(request.id.clone(), err),
        }
    }

    // --- Handlers Rantai & Blok ---

    fn handle_chain_id(&self) -> Result<String, JsonRpcError> {
        Ok(format!("{}", self.chain_id))
    }

    fn handle_block_height(&self) -> Result<String, JsonRpcError> {
        let h = self.current_height.load(Ordering::SeqCst);
        Ok(format!("{h}"))
    }

    fn handle_get_block_by_height(&self, params: &[String]) -> Result<String, JsonRpcError> {
        if params.is_empty() {
            return Err(invalid_params("Missing block height parameter"));
        }
        let height = params[0].parse::<u64>().map_err(|_| {
            invalid_params(format!("Invalid integer block height: '{}'", params[0]))
        })?;

        let consistency = if params.len() > 1 {
            ConsistencySelector::parse(&params[1])?
        } else {
            ConsistencySelector::Finalized
        };

        if consistency == ConsistencySelector::Finalized {
            let fin_h = self.finalized_height.load(Ordering::SeqCst);
            if height > fin_h {
                return Err(finality_not_reached(height));
            }
        }

        let headers = self.headers.lock().unwrap();
        if let Some(header) = headers.get(&height) {
            Ok(format!(
                r#"{{"height":{},"round":{},"timestamp":{},"state_root":"{}","prev_hash":"{}"}}"#,
                header.height,
                header.round,
                header.timestamp,
                hex::encode(header.state_root.as_bytes()),
                hex::encode(header.prev_block_hash.as_bytes())
            ))
        } else {
            Err(resource_not_found(format!("Block at height {height} not found")))
        }
    }

    fn handle_get_block_by_hash(&self, params: &[String]) -> Result<String, JsonRpcError> {
        if params.is_empty() {
            return Err(invalid_params("Missing block hash parameter"));
        }
        let hash_hex = &params[0];
        let headers = self.headers.lock().unwrap();

        for header in headers.values() {
            if hex::encode(header.compute_block_hash().as_bytes()) == *hash_hex {
                return Ok(format!(
                    r#"{{"height":{},"round":{},"timestamp":{}}}"#,
                    header.height, header.round, header.timestamp
                ));
            }
        }
        Err(resource_not_found(format!("Block with hash {hash_hex} not found")))
    }

    fn handle_get_commit_certificate(&self, params: &[String]) -> Result<String, JsonRpcError> {
        if params.is_empty() {
            return Err(invalid_params("Missing block height parameter"));
        }
        let height = params[0].parse::<u64>().map_err(|_| {
            invalid_params(format!("Invalid integer block height: '{}'", params[0]))
        })?;

        let certs = self.certificates.lock().unwrap();
        if let Some(cert) = certs.get(&height) {
            Ok(format!(
                r#"{{"height":{},"round":{},"signatures_count":{}}}"#,
                cert.height,
                cert.round,
                cert.precommits.len()
            ))
        } else {
            Err(resource_not_found(format!(
                "Commit certificate for height {height} not found"
            )))
        }
    }

    // --- Handlers Transaksi & Mempool ---

    fn handle_send_raw_transaction(
        &self,
        params: &[String],
        current_time: u64,
    ) -> Result<String, JsonRpcError> {
        if params.len() < 2 {
            return Err(invalid_params(
                "Missing parameters: expected [raw_tx_hex, sender_pubkey_hex]",
            ));
        }

        let raw_tx_bytes = hex::decode(&params[0])
            .map_err(|e| invalid_params(format!("Invalid raw_tx hex encoding: {e}")))?;
        let pubkey_bytes = hex::decode(&params[1])
            .map_err(|e| invalid_params(format!("Invalid sender_pubkey hex encoding: {e}")))?;

        if pubkey_bytes.len() != 32 {
            return Err(invalid_params("Sender public key must be exactly 32 bytes"));
        }
        let mut sender_pubkey = [0u8; 32];
        sender_pubkey.copy_from_slice(&pubkey_bytes);

        let mut cursor = 0;
        let tx = Transaction::decode_canonical(&raw_tx_bytes, &mut cursor).map_err(|e| {
            invalid_params(format!("Canonical transaction decoding failed: {e:?}"))
        })?;

        if tx.chain_id != self.chain_id {
            return Err(invalid_params(format!(
                "InvalidChainId: expected {}, got {}",
                self.chain_id, tx.chain_id
            )));
        }
        if tx.valid_until != 0 && tx.valid_until <= current_time {
            return Err(invalid_params(format!(
                "TransactionExpired: valid_until {} is not after current time {}",
                tx.valid_until, current_time
            )));
        }

        // Ambil atau inisialisasi state akun pengirim
        let accounts = self.accounts.lock().unwrap();
        let default_account = Account::default();
        let account_state = accounts.get(&tx.sender).unwrap_or(&default_account);

        let mut mempool = self.mempool.lock().unwrap();
        let tx_id = mempool
            .submit_transaction(tx, &sender_pubkey, current_time, account_state)
            .map_err(|e| tx_rejected(&format!("{e}"), None))?;

        Ok(format!("\"{}\"", hex::encode(tx_id.as_bytes())))
    }

    fn handle_get_transaction_by_hash(&self, params: &[String]) -> Result<String, JsonRpcError> {
        if params.is_empty() {
            return Err(invalid_params("Missing tx_id parameter"));
        }
        let tx_id_hex = &params[0];
        let tx_id_bytes = hex::decode(tx_id_hex)
            .map_err(|_| invalid_params("Invalid tx_id hex string"))?;
        if tx_id_bytes.len() != 32 {
            return Err(invalid_params("TxID must be exactly 32 bytes"));
        }

        let mut tx_id_arr = [0u8; 32];
        tx_id_arr.copy_from_slice(&tx_id_bytes);
        let target_id = Hash256(tx_id_arr);

        // 1. Cek apakah transaksi sudah difinalisasi dan masuk blok (recent_transactions)
        let recent = self.recent_transactions.lock().unwrap();
        if let Some(summary) = recent.iter().find(|s| s.tx_id == target_id) {
            let sender = encode_address_bech32m(&summary.tx.sender, "aur")
                .unwrap_or_else(|_| summary.tx.sender.to_hex());
            let recipient = encode_address_bech32m(&summary.tx.recipient, "aur")
                .unwrap_or_else(|_| summary.tx.recipient.to_hex());
            return Ok(format!(
                r#"{{"tx_id":"{}","status":"CONFIRMED","block_height":{},"timestamp":{},"sender":"{}","recipient":"{}","nonce":{},"amount":"{}","fee":"{}"}}"#,
                tx_id_hex,
                summary.height,
                summary.received_at,
                sender,
                recipient,
                summary.tx.nonce,
                summary.tx.amount.as_u128(),
                summary.tx.fee.as_u128()
            ));
        }
        drop(recent);

        // 2. Cek apakah transaksi masih antre di mempool (Pending)
        let mempool = self.mempool.lock().unwrap();
        if let Some(entry) = mempool.entries.get(&target_id) {
            let sender = encode_address_bech32m(&entry.tx.sender, "aur")
                .unwrap_or_else(|_| entry.tx.sender.to_hex());
            let recipient = encode_address_bech32m(&entry.tx.recipient, "aur")
                .unwrap_or_else(|_| entry.tx.recipient.to_hex());
            Ok(format!(
                r#"{{"tx_id":"{}","status":"MEMPOOL","sender":"{}","recipient":"{}","nonce":{},"amount":"{}","fee":"{}"}}"#,
                tx_id_hex,
                sender,
                recipient,
                entry.tx.nonce,
                entry.tx.amount.as_u128(),
                entry.tx.fee.as_u128()
            ))
        } else {
            Err(resource_not_found(format!(
                "Transaction {tx_id_hex} not found in mempool or finalized state"
            )))
        }
    }

    fn handle_estimate_fee(&self) -> Result<String, JsonRpcError> {
        // Biaya transfer standar minimum: 10.000 Quanta (0.0001 AUR)
        Ok(r#"{"recommended_fee_quanta":"10000"}"#.to_string())
    }

    fn handle_get_mempool(&self) -> Result<String, JsonRpcError> {
        let mempool = self.mempool.lock().unwrap();
        let tx_ids: Vec<String> = mempool
            .entries
            .keys()
            .map(|id| format!("\"{}\"", hex::encode(id.as_bytes())))
            .collect();

        Ok(format!("[{}]", tx_ids.join(",")))
    }

    // --- Handlers State Akun ---

    fn parse_address(addr_str: &str) -> Result<Address, JsonRpcError> {
        let trimmed = addr_str.trim();
        // Coba decode bech32m terlebih dahulu
        if let Ok(addr) = decode_address_bech32m(trimmed, "aur") {
            return Ok(addr);
        }
        // Coba decode raw hex jika ada
        if let Ok(raw_bytes) = hex::decode(trimmed) {
            if raw_bytes.len() == 32 {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&raw_bytes);
                return Ok(Address(arr));
            }
        }
        Err(invalid_params(format!("Invalid Aurion address: '{trimmed}'")))
    }

    fn handle_get_balance(&self, params: &[String]) -> Result<String, JsonRpcError> {
        if params.is_empty() {
            return Err(invalid_params("Missing address parameter"));
        }
        let address = Self::parse_address(&params[0])?;

        let _consistency = if params.len() > 1 {
            ConsistencySelector::parse(&params[1])?
        } else {
            ConsistencySelector::Finalized
        };

        let accounts = self.accounts.lock().unwrap();
        let balance = accounts
            .get(&address)
            .map(|acc| acc.balance)
            .unwrap_or(Quantum::ZERO);

        // Nilai moneter wajib berupa String integer murni (Zero-Float Invariant)
        Ok(format!(r#""{}""#, balance.as_u128()))
    }

    fn handle_get_nonce(&self, params: &[String]) -> Result<String, JsonRpcError> {
        if params.is_empty() {
            return Err(invalid_params("Missing address parameter"));
        }
        let address = Self::parse_address(&params[0])?;

        let accounts = self.accounts.lock().unwrap();
        let nonce = accounts.get(&address).map(|acc| acc.nonce).unwrap_or(0);

        Ok(format!("{nonce}"))
    }

    fn handle_get_account(&self, params: &[String]) -> Result<String, JsonRpcError> {
        if params.is_empty() {
            return Err(invalid_params("Missing address parameter"));
        }
        let address = Self::parse_address(&params[0])?;

        let consistency = if params.len() > 1 {
            ConsistencySelector::parse(&params[1])?
        } else {
            ConsistencySelector::Finalized
        };

        let accounts = self.accounts.lock().unwrap();
        let acc = accounts
            .get(&address)
            .cloned()
            .unwrap_or_else(Account::default);

        // Field `code_hash`/`is_contract` menopang Contract SDK melakukan
        // binding metadata kontrak ke kode on-chain (AUR-VM-006).
        let code_hash_json = match acc.code_hash {
            Some(h) => format!("\"{}\"", h.to_hex()),
            None => "null".to_string(),
        };
        Ok(format!(
            r#"{{"address":"{}","balance":"{}","nonce":{},"is_contract":{},"code_hash":{},"consistency":"{consistency:?}"}}"#,
            params[0],
            acc.balance.as_u128(),
            acc.nonce,
            acc.is_contract(),
            code_hash_json
        ))
    }

    // --- Handlers Komunitas & Testnet (NET-012) ---

    fn handle_get_network_stats(&self) -> Result<String, JsonRpcError> {
        let height = self.current_height.load(Ordering::SeqCst);
        let finalized = self.finalized_height.load(Ordering::SeqCst);
        let mempool_size = self.mempool.lock().unwrap().entries.len();
        let accounts_count = self.accounts.lock().unwrap().len();
        let headers_count = self.headers.lock().unwrap().len();
        let faucet_active = self.faucet.lock().unwrap().is_some();

        Ok(format!(
            r#"{{"chain_id":{},"current_height":{},"finalized_height":{},"mempool_size":{},"accounts_count":{},"headers_count":{},"faucet_active":{}}}"#,
            self.chain_id, height, finalized, mempool_size, accounts_count, headers_count, faucet_active
        ))
    }

    fn handle_request_faucet(
        &self,
        params: &[String],
        current_time: u64,
    ) -> Result<String, JsonRpcError> {
        if params.is_empty() {
            return Err(invalid_params("Missing recipient address parameter"));
        }
        let recipient_addr = decode_address_bech32m(&params[0], "aur")
            .map_err(|e| invalid_params(format!("Invalid bech32m address '{}': {e}", params[0])))?;

        let mut faucet_guard = self.faucet.lock().unwrap();
        if let Some(faucet) = faucet_guard.as_mut() {
            let accounts = self.accounts.lock().unwrap();
            let mut mempool = self.mempool.lock().unwrap();
            let (tx_hash, _tx) = faucet
                .dispense(&recipient_addr, &accounts, &mut mempool, current_time)
                .map_err(|e| internal_error(format!("Faucet error: {e}")))?;

            Ok(format!("\"0x{}\"", hex::encode(tx_hash.as_bytes())))
        } else {
            Err(internal_error("Faucet is not enabled on this node"))
        }
    }
}
