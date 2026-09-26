//! Klien JSON-RPC ringan untuk workflow wallet CLI.

use serde_json::Value;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcClientError(pub String);

impl std::fmt::Display for RpcClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RpcClientError {}

fn parse_http_url(url: &str) -> Result<(&str, u16), RpcClientError> {
    let without_scheme = url
        .strip_prefix("http://")
        .ok_or_else(|| RpcClientError(format!("URL RPC tidak didukung: {url}")))?;
    let authority = without_scheme.split('/').next().unwrap_or(without_scheme);
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or_else(|| RpcClientError(format!("URL RPC harus menyertakan port: {url}")))?;
    let port = port
        .parse::<u16>()
        .map_err(|_| RpcClientError(format!("Port RPC tidak valid: {port}")))?;
    if host.is_empty() {
        return Err(RpcClientError("Host RPC kosong".to_string()));
    }
    Ok((host, port))
}

fn call(rpc_url: &str, method: &str, params: &[&str]) -> Result<Value, RpcClientError> {
    let (host, port) = parse_http_url(rpc_url)?;
    let address = (host, port)
        .to_socket_addrs()
        .map_err(|e| RpcClientError(format!("Gagal terhubung ke RPC node di {rpc_url}: {e}")))?
        .next()
        .ok_or_else(|| RpcClientError(format!("Gagal terhubung ke RPC node di {rpc_url}: alamat tidak ditemukan")))?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))
        .map_err(|e| RpcClientError(format!("Gagal terhubung ke RPC node di {rpc_url}: {e}")))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| RpcClientError(format!("Gagal mengatur timeout RPC: {e}")))?;

    let params_json = serde_json::to_string(params)
        .map_err(|e| RpcClientError(format!("Gagal menyusun parameter RPC: {e}")))?;
    let request_body = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{params_json}}}"#
    );
    let request = format!(
        "POST /rpc HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{request_body}",
        request_body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| RpcClientError(format!("Gagal mengirim request RPC ke {rpc_url}: {e}")))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|e| RpcClientError(format!("Gagal membaca response RPC dari {rpc_url}: {e}")))?;
    let response = String::from_utf8(response)
        .map_err(|e| RpcClientError(format!("Response RPC bukan UTF-8: {e}")))?;
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .ok_or_else(|| RpcClientError("Response HTTP RPC tidak valid".to_string()))?;
    let json: Value = serde_json::from_str(body)
        .map_err(|e| RpcClientError(format!("Response JSON-RPC tidak valid: {e}")))?;
    if let Some(error) = json.get("error") {
        return Err(RpcClientError(format!("RPC {method} gagal: {error}")));
    }
    json.get("result")
        .cloned()
        .ok_or_else(|| RpcClientError("Response RPC tidak memiliki result".to_string()))
}

pub fn get_balance(rpc_url: &str, address: &str) -> Result<u128, RpcClientError> {
    let result = call(rpc_url, "aur_getBalance", &[address])?;
    let value = result
        .as_str()
        .ok_or_else(|| RpcClientError("RPC balance bukan string Quantum".to_string()))?;
    value
        .parse::<u128>()
        .map_err(|_| RpcClientError(format!("RPC mengembalikan saldo tidak valid: {value}")))
}

pub fn get_nonce(rpc_url: &str, address: &str) -> Result<u64, RpcClientError> {
    let result = call(rpc_url, "aur_getNonce", &[address])?;
    if let Some(value) = result.as_u64() {
        return Ok(value);
    }
    result
        .as_str()
        .ok_or_else(|| RpcClientError("RPC nonce bukan integer".to_string()))?
        .parse::<u64>()
        .map_err(|_| RpcClientError("RPC mengembalikan nonce tidak valid".to_string()))
}

pub fn broadcast_raw_tx(
    rpc_url: &str,
    raw_hex: &str,
    sender_pubkey_hex: &str,
) -> Result<String, RpcClientError> {
    let result = call(
        rpc_url,
        "aur_sendRawTransaction",
        &[raw_hex, sender_pubkey_hex],
    )?;
    result
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| RpcClientError("RPC TxID bukan string".to_string()))
}

/// Panggilan JSON-RPC generik (dipakai Contract SDK untuk `aur_chainId`, dsb.).
pub fn rpc_call(rpc_url: &str, method: &str, params: &[&str]) -> Result<Value, RpcClientError> {
    call(rpc_url, method, params)
}

/// Ambil chain ID simpul (`aur_chainId`).
pub fn get_chain_id(rpc_url: &str) -> Result<u32, RpcClientError> {
    let result = call(rpc_url, "aur_chainId", &[])?;
    if let Some(value) = result.as_str() {
        return value
            .parse::<u32>()
            .map_err(|_| RpcClientError(format!("RPC chain ID tidak valid: {value}")));
    }
    result
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| RpcClientError("RPC chain ID bukan integer".to_string()))
}

/// Snapshot akun on-chain hasil `aur_getAccount` (termasuk binding kontrak).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcAccount {
    /// Saldo dalam Quanta.
    pub balance: u128,
    /// Nonce on-chain.
    pub nonce: u64,
    /// `code_hash` kontrak (hex 32-byte) bila akun adalah kontrak.
    pub code_hash: Option<[u8; 32]>,
    /// Apakah akun adalah kontrak (`code_hash` ada).
    pub is_contract: bool,
}

/// Parse objek hasil `aur_getAccount` (tahan terhadap simpul lama tanpa
/// field `code_hash`/`is_contract`).
///
/// # Errors
/// Field utama (`balance`/`nonce`) tidak dapat diparse.
pub fn parse_account_value(value: &Value) -> Result<RpcAccount, RpcClientError> {
    let balance = match value.get("balance") {
        Some(Value::String(s)) => s
            .parse::<u128>()
            .map_err(|_| RpcClientError(format!("RPC balance tidak valid: {s}")))?,
        Some(Value::Number(n)) => n
            .as_u64()
            .map(u128::from)
            .ok_or_else(|| RpcClientError("RPC balance bukan integer".to_string()))?,
        _ => return Err(RpcClientError("RPC akun tanpa balance".to_string())),
    };
    let nonce = match value.get("nonce") {
        Some(Value::String(s)) => s
            .parse::<u64>()
            .map_err(|_| RpcClientError(format!("RPC nonce tidak valid: {s}")))?,
        Some(Value::Number(n)) => n
            .as_u64()
            .ok_or_else(|| RpcClientError("RPC nonce bukan integer".to_string()))?,
        _ => return Err(RpcClientError("RPC akun tanpa nonce".to_string())),
    };
    let code_hash = match value.get("code_hash") {
        Some(Value::String(s)) => {
            let bytes =
                hex::decode(s.trim_start_matches("0x")).map_err(|e| {
                    RpcClientError(format!("RPC code_hash hex tidak valid: {e}"))
                })?;
            if bytes.len() != 32 {
                return Err(RpcClientError(format!(
                    "RPC code_hash harus 32 byte, diterima {}",
                    bytes.len()
                )));
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            Some(arr)
        }
        _ => None,
    };
    let is_contract = value
        .get("is_contract")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| code_hash.is_some());
    Ok(RpcAccount {
        balance,
        nonce,
        code_hash,
        is_contract,
    })
}

/// Ambil snapshot akun on-chain (`aur_getAccount`).
pub fn get_account(rpc_url: &str, address: &str) -> Result<RpcAccount, RpcClientError> {
    let result = call(rpc_url, "aur_getAccount", &[address])?;
    parse_account_value(&result)
}

#[cfg(test)]
mod tests {
    use super::parse_http_url;

    #[test]
    fn parses_http_rpc_url() {
        assert_eq!(parse_http_url("http://127.0.0.1:8545").unwrap(), ("127.0.0.1", 8545));
    }

    #[test]
    fn rejects_rpc_url_without_port() {
        assert!(parse_http_url("http://127.0.0.1").is_err());
    }
}
