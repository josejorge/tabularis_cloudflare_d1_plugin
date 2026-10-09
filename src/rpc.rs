// File: rpc.rs
// Description: JSON-RPC request/response types and helpers for the Tabularis plugin protocol.
// Author: Jose-Jorge HERNANDEZ
// Company: Parlee Conseiller, Inc.
// Date: 2026-06-15
// Last edit date: 2026-10-09
// Version: 1.1.0

use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    // Absent for JSON-RPC notifications (e.g. the host's `cancel`), which
    // must never receive a reply.
    #[serde(default)]
    pub id: Option<Value>,
}

/// JSON-RPC "method not found" code; the host keys its optional-method
/// fallbacks (connection metadata, table query templates...) on this code.
pub const METHOD_NOT_FOUND: i32 = -32601;

pub fn ok(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "result": result,
        "id": id
    })
}

pub fn err(id: Value, code: i32, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "error": { "code": code, "message": message.into() },
        "id": id
    })
}

pub fn internal_err(id: Value, message: impl Into<String>) -> Value {
    err(id, -32603, message)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_without_id_is_a_notification() {
        let req: RpcRequest =
            serde_json::from_str(r#"{"jsonrpc":"2.0","method":"cancel","params":{"id":7}}"#)
                .unwrap();
        assert!(req.id.is_none());
    }

    #[test]
    fn request_with_id_keeps_it() {
        let req: RpcRequest =
            serde_json::from_str(r#"{"jsonrpc":"2.0","method":"ping","params":{},"id":3}"#)
                .unwrap();
        assert_eq!(req.id, Some(json!(3)));
    }
}
