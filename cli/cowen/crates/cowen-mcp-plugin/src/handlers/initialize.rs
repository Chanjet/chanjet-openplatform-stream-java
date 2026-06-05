use crate::protocol::{AppState, JsonRpcRequest, JsonRpcResponse};
use serde_json::json;

pub const LATEST_PROTOCOL_VERSION: &str = "2024-11-05";

pub async fn handle_initialize(req: JsonRpcRequest, app_state: &AppState) -> JsonRpcResponse {
    let mut negotiated_version = LATEST_PROTOCOL_VERSION.to_string();

    if let Some(params) = &req.params {
        let mut state = app_state.mcp_state.lock().await;
        
        if let Some(pv) = params.get("protocolVersion").and_then(|v| v.as_str()) {
            negotiated_version = pv.to_string();
            state.protocol_version = Some(pv.to_string());
        }
    }

    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: req.id,
        result: Some(json!({
            "protocolVersion": negotiated_version,
            "capabilities": {
                "tools": { "listChanged": true }
            },
            "serverInfo": {
                "name": "cowen-mcp-plugin",
                "version": "1.0.0",
                "description": "This MCP server provides three meta-tools to manage and orchestrate dynamic Cowen APIs:\n1. Use `cowen_api_list` to search and discover available APIs and find the target `tool_name`.\n2. Call `cowen_enable_api` with the `tool_name` to dynamically load that API as a new standard MCP Tool.\n3. Once registered, you (the LLM) can invoke the newly registered API tool directly to perform operations.\n4. Call `cowen_disable_api` with the `tool_name` to deregister the tool when it is no longer needed."
            }
        })),
        error: None,
    }
}
