use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use serde_json::json;

pub fn handle_initialize(req: JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: req.id,
        result: Some(json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {
                "tools": { "listChanged": true }
            },
            "serverInfo": {
                "name": "cowen-mcp-plugin",
                "version": "1.0.0"
            }
        })),
        error: None,
    }
}
