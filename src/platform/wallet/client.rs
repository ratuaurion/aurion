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
