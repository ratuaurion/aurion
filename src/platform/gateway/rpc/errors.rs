//! Taksonomi Galat Standar JSON-RPC 2.0 Aurion.
//! Mematuhi Dokumen 02 (02-RPC-API-RULES.md Bagian 5).

use crate::gateway::rpc::types::JsonRpcError;

pub const ERR_PARSE_ERROR: i32 = -32700;
pub const ERR_INVALID_REQUEST: i32 = -32600;
pub const ERR_METHOD_NOT_FOUND: i32 = -32601;
pub const ERR_INVALID_PARAMS: i32 = -32602;
pub const ERR_INTERNAL_ERROR: i32 = -32603;

// Aurion Specific Extension Errors (-32001 s/d -32005)
pub const ERR_TX_REJECTED: i32 = -32001;
pub const ERR_RESOURCE_NOT_FOUND: i32 = -32002;
pub const ERR_FINALITY_NOT_REACHED: i32 = -32003;
pub const ERR_RATE_LIMIT_EXCEEDED: i32 = -32004;
pub const ERR_NODE_SYNCING: i32 = -32005;

pub fn method_not_found(method: &str) -> JsonRpcError {
    JsonRpcError::new(
        ERR_METHOD_NOT_FOUND,
        format!("Method '{method}' not found on Aurion node"),
        None,
    )
}

pub fn invalid_params(reason: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(ERR_INVALID_PARAMS, reason, None)
}

pub fn resource_not_found(resource: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(ERR_RESOURCE_NOT_FOUND, resource, None)
}

pub fn tx_rejected(reason: &str, details_json: Option<String>) -> JsonRpcError {
    JsonRpcError::new(
        ERR_TX_REJECTED,
        format!("Transaction Rejected: {reason}"),
        details_json,
    )
}

pub fn finality_not_reached(height: u64) -> JsonRpcError {
    JsonRpcError::new(
        ERR_FINALITY_NOT_REACHED,
        format!("Block at height {height} has not achieved BFT quorum finality (>2/3)"),
        None,
    )
}
