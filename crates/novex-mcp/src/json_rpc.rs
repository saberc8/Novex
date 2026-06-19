use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::types::McpToolInvocationRequest;
use crate::MCP_PROTOCOL_VERSION;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpJsonRpcRequest {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    pub params: Value,
}

impl McpJsonRpcRequest {
    pub fn initialize(id: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id: id.into(),
            method: "initialize".to_owned(),
            params: json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "novex",
                    "version": env!("CARGO_PKG_VERSION"),
                },
            }),
        }
    }

    pub fn tools_call(id: impl Into<String>, request: &McpToolInvocationRequest) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id: id.into(),
            method: "tools/call".to_owned(),
            params: json!({
                "name": request.tool_name,
                "arguments": request.arguments,
            }),
        }
    }

    pub fn into_value(self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| Value::Null)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpJsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
}

impl McpJsonRpcNotification {
    pub fn initialized() -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            method: "notifications/initialized".to_owned(),
            params: json!({}),
        }
    }

    pub fn into_value(self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| Value::Null)
    }
}
