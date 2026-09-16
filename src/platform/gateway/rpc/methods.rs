//! Router dan Dispatcher Metode RPC Namespace "aur_*" Aurion.
//! Mematuhi Dokumen 02 (02-RPC-API-RULES.md Bagian 4).

use crate::codec::CanonicalDecode;
use crate::consensus::certificate::CommitCertificate;
use crate::consensus::header::BlockHeader;
use crate::core::{Address, Hash256, Quantum};
use crate::crypto::decode_address_bech32m;
use crate::gateway::rpc::consistency::ConsistencySelector;
use crate::gateway::rpc::errors::*;
use crate::gateway::rpc::types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::mempool::MempoolEngine;
use crate::state::account::Account;
use crate::transaction::types::Transaction;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Konteks state bersama untuk melayani query dan submit JSON-RPC 2.0.
pub struct RpcContext {
    pub chain_id: u64,
    pub current_height: Arc<AtomicU64>,
    pub finalized_height: Arc<AtomicU64>,
    pub mempool: Arc<Mutex<MempoolEngine>>,
    pub accounts: Arc<Mutex<HashMap<Address, Account>>>,
    pub headers: Arc<Mutex<HashMap<u64, BlockHeader>>>,
    pub certificates: Arc<Mutex<HashMap<u64, CommitCertificate>>>,
}

impl RpcContext {
    pub fn new(chain_id: u64) -> Self {
        Self {
            chain_id,
            current_height: Arc::new(AtomicU64::new(0)),
            finalized_height: Arc::new(AtomicU64::new(0)),
            mempool: Arc::new(Mutex::new(MempoolEngine::default())),
            accounts: Arc::new(Mutex::new(HashMap::new())),
            headers: Arc::new(Mutex::new(HashMap::new())),
            certificates: Arc::new(Mutex::new(HashMap::new())),
        }
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

        let mempool = self.mempool.lock().unwrap();
        if let Some(entry) = mempool.entries.get(&target_id) {
            Ok(format!(
                r#"{{"tx_id":"{}","status":"MEMPOOL","nonce":{},"amount":"{}","fee":"{}"}}"#,
                tx_id_hex,
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

        Ok(format!(
            r#"{{"address":"{}","balance":"{}","nonce":{},"consistency":"{consistency:?}"}}"#,
            params[0],
            acc.balance.as_u128(),
            acc.nonce
        ))
    }
}
