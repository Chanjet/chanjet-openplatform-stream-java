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
                "version": "1.0.0",
                "description": "This MCP server provides three meta-tools to manage and orchestrate dynamic Cowen APIs:\n1. Use `cowen_api_list` to search and discover available APIs and find the target `tool_name`.\n2. Call `cowen_enable_api` with the `tool_name` to dynamically load that API as a new standard MCP Tool.\n3. Once registered, you (the LLM) can invoke the newly registered API tool directly to perform operations.\n4. Call `cowen_disable_api` with the `tool_name` to deregister the tool when it is no longer needed."
            }
        })),
        error: None,
    }
}
