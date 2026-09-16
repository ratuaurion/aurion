//! Definisi Tipe dan Struktur Data JSON-RPC 2.0 Kanonikal Aurion.
//! Mematuhi Spesifikasi Resmi JSON-RPC 2.0 dan Dokumen 02 (02-RPC-API-RULES.md).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcId {
    Number(u64),
    String(String),
    Null,
}

impl fmt::Display for RpcId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RpcId::Number(n) => write!(f, "{n}"),
            RpcId::String(s) => write!(f, "\"{s}\""),
            RpcId::Null => write!(f, "null"),
        }
    }
}

/// Objek Galat Standar JSON-RPC 2.0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<String>,
}

pub type JsonRpcId = RpcId;

impl JsonRpcError {
    pub fn new(code: i32, message: impl Into<String>, data: Option<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data,
        }
    }

    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::new(-32700, message, None)
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(-32600, message, None)
    }

    pub fn method_not_found(message: impl Into<String>) -> Self {
        Self::new(-32601, message, None)
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(-32602, message, None)
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(-32603, message, None)
    }

    pub fn to_json_string(&self) -> String {
        if let Some(ref d) = self.data {
            format!(
                r#"{{"code":{},"message":"{}","data":{}}}"#,
                self.code, self.message, d
            )
        } else {
            format!(
                r#"{{"code":{},"message":"{}"}}"#,
                self.code, self.message
            )
        }
    }
}

pub fn parse_json_rpc_request(raw_json: &str) -> Result<JsonRpcRequest, JsonRpcError> {
    JsonRpcRequest::parse(raw_json)
}

pub fn serialize_json_rpc_response(resp: &JsonRpcResponse) -> String {
    resp.to_json_string()
}

/// Permintaan JSON-RPC 2.0 (Request Object).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: RpcId,
    pub method: String,
    pub params: Vec<String>,
}

impl JsonRpcRequest {
    /// Parser JSON-RPC 2.0 deterministik murni tanpa dependensi eksternal.
    pub fn parse(raw_json: &str) -> Result<Self, JsonRpcError> {
        let trimmed = raw_json.trim();
        if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
            return Err(JsonRpcError::new(-32700, "Parse error: Invalid JSON object", None));
        }

        // Ekstraksi field method
        let method = extract_json_string_field(trimmed, "method")
            .ok_or_else(|| JsonRpcError::new(-32600, "Invalid Request: Missing 'method' field", None))?;

        // Ekstraksi jsonrpc version
        let jsonrpc = extract_json_string_field(trimmed, "jsonrpc")
            .unwrap_or_else(|| "2.0".to_string());
        if jsonrpc != "2.0" {
            return Err(JsonRpcError::new(
                -32600,
                "Invalid Request: Protocol version must be '2.0'",
                None,
            ));
        }

        // Ekstraksi id
        let id = extract_json_id_field(trimmed);

        // Ekstraksi params array
        let params = extract_json_params_array(trimmed);

        Ok(JsonRpcRequest {
            jsonrpc,
            id,
            method,
            params,
        })
    }
}

/// Tanggapan JSON-RPC 2.0 (Response Object).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: RpcId,
    pub result: Option<String>,
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: RpcId, result_json: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result_json.into()),
            error: None,
        }
    }

    pub fn error(id: RpcId, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }

    pub fn to_json_string(&self) -> String {
        if let Some(ref err) = self.error {
            format!(
                r#"{{"jsonrpc":"2.0","id":{},"error":{}}}"#,
                self.id,
                err.to_json_string()
            )
        } else {
            let res = self.result.as_deref().unwrap_or("null");
            format!(
                r#"{{"jsonrpc":"2.0","id":{},"result":{}}}"#,
                self.id, res
            )
        }
    }
}

// Utilitas parser teks JSON deterministik
fn extract_json_string_field(json: &str, field_name: &str) -> Option<String> {
    let pattern = format!("\"{}\"", field_name);
    let field_pos = json.find(&pattern)?;
    let after_field = &json[field_pos + pattern.len()..];
    let colon_pos = after_field.find(':')?;
    let after_colon = after_field[colon_pos + 1..].trim_start();

    if !after_colon.starts_with('"') {
        return None;
    }

    let end_quote = after_colon[1..].find('"')?;
    Some(after_colon[1..1 + end_quote].to_string())
}

fn extract_json_id_field(json: &str) -> RpcId {
    let pattern = "\"id\"";
    if let Some(pos) = json.find(pattern) {
        let after_field = &json[pos + pattern.len()..];
        if let Some(colon_pos) = after_field.find(':') {
            let val_str = after_field[colon_pos + 1..].trim_start();
            if let Some(stripped) = val_str.strip_prefix('"') {
                if let Some(end_quote) = stripped.find('"') {
                    return RpcId::String(stripped[..end_quote].to_string());
                }
            } else if val_str.starts_with("null") {
                return RpcId::Null;
            } else {
                let num_str: String = val_str
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(n) = num_str.parse::<u64>() {
                    return RpcId::Number(n);
                }
            }
        }
    }
    RpcId::Null
}

fn extract_json_params_array(json: &str) -> Vec<String> {
    let pattern = "\"params\"";
    if let Some(pos) = json.find(pattern) {
        let after_field = &json[pos + pattern.len()..];
        if let Some(colon_pos) = after_field.find(':') {
            let after_colon = after_field[colon_pos + 1..].trim_start();
            if after_colon.starts_with('[') {
                if let Some(end_bracket) = after_colon.find(']') {
                    let inside = &after_colon[1..end_bracket];
                    return inside
                        .split(',')
                        .map(|item| {
                            let trimmed = item.trim();
                            if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
                                trimmed[1..trimmed.len() - 1].to_string()
                            } else {
                                trimmed.to_string()
                            }
                        })
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
        }
    }
    Vec::new()
}
