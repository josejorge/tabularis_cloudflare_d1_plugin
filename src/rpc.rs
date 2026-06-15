use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
    pub id: Value,
}

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

