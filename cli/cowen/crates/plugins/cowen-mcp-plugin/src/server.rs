use crate::protocol::{AppState, JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::handlers::{handle_initialize, handle_tools_list, handle_tools_call};

pub async fn handle_request(
    req: JsonRpcRequest,
    app_state: &AppState,
) -> (Option<JsonRpcResponse>, bool) {
    if req.jsonrpc != "2.0" {
        return (None, false);
    }

    let mut list_changed = false;

    let response = match req.method.as_str() {
        "initialize" => Some(handle_initialize(req, app_state).await),
        "notifications/initialized" => None,
        "tools/list" => Some(handle_tools_list(req, app_state).await),
        "tools/call" => {
            let (resp, changed) = handle_tools_call(req, app_state).await;
            list_changed = changed;
            Some(resp)
        }
        _ => Some(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
            }),
        }),
    };

    (response, list_changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::EnabledTool;
    use serde_json::json;

    async fn setup_app_state_with_tool(protocol_version: &str, tool_name: &str, output_schema: Option<serde_json::Value>) -> AppState {
        let app_state = AppState::new("test_tenant".to_string());
        let mut state = app_state.mcp_state.lock().await;
        state.protocol_version = Some(protocol_version.to_string());
        state.tools.insert(
            tool_name.to_string(),
            EnabledTool {
                method: "GET".to_string(),
                path: if tool_name == "get__v1_array" { "/v1/array".to_string() } else { "/v1/test".to_string() },
                description: "Test description".to_string(),
                input_schema: json!({ "type": "object", "properties": {} }),
                output_schema,
                body_params: vec![],
            },
        );
        drop(state);
        app_state
    }

    #[tokio::test]
    async fn test_dynamic_tool_structured_content_fallback() {
        let app_state = setup_app_state_with_tool(
            "2025-06-18",
            "get__v1_test",
            Some(json!({
                "type": "object",
                "properties": {
                    "data": { "type": "string" }
                }
            }))
        ).await;

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "get__v1_test",
                "arguments": {}
            })),
        };

        let (resp, _changed) = handle_request(req, &app_state).await;
        let response = resp.unwrap();

        assert_eq!(response.id.unwrap().as_i64().unwrap(), 1);
        let result = response.result.unwrap();
        
        assert_eq!(result.get("structuredContent").unwrap(), &json!({}));
    }

    #[tokio::test]
    async fn test_tools_list_wraps_non_object_schema() {
        let app_state = setup_app_state_with_tool(
            "2025-06-18",
            "get__v1_array",
            Some(json!({
                "type": "array",
                "items": { "type": "string" }
            }))
        ).await;

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/list".to_string(),
            params: None,
        };

        let (resp, _) = handle_request(req, &app_state).await;
        let response = resp.unwrap();
        let result = response.result.unwrap();
        let tools = result.get("tools").unwrap().as_array().unwrap();
        
        let tool = tools.iter().find(|t| t.get("name").unwrap().as_str().unwrap() == "get__v1_array").unwrap();
        let output_schema = tool.get("outputSchema").unwrap();
        
        assert_eq!(output_schema.get("type").unwrap().as_str().unwrap(), "object");
        let properties = output_schema.get("properties").unwrap();
        let value_schema = properties.get("value").unwrap();
        assert_eq!(value_schema.get("type").unwrap().as_str().unwrap(), "array");
        assert_eq!(value_schema.get("items").unwrap().get("type").unwrap().as_str().unwrap(), "string");
        assert_eq!(output_schema.get("required").unwrap(), &json!(["value"]));
    }

    #[tokio::test]
    async fn test_initialize_contains_orchestration_instructions() {
        let app_state = AppState::new("test_tenant".to_string());
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test-client", "version": "1.0.0" }
            })),
        };

        let (resp, _) = handle_request(req, &app_state).await;
        let response = resp.unwrap();
        let result = response.result.unwrap();
        let server_info = result.get("serverInfo").unwrap();
        let description = server_info.get("description").unwrap().as_str().unwrap();

        assert!(description.contains("cowen_api_list"));
        assert!(description.contains("cowen_enable_api"));
        assert!(description.contains("cowen_disable_api"));
        assert!(description.contains("orchestrat"));
    }

    #[tokio::test]
    async fn test_initialize_tracks_client_version() {
        let app_state = AppState::new("test_tenant".to_string());
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: Some(json!({
                "protocolVersion": "2024-10-01",
                "capabilities": {},
                "clientInfo": { "name": "legacy-client", "version": "3.0.0" }
            })),
        };

        let _ = handle_request(req, &app_state).await;

        let state = app_state.mcp_state.lock().await;
        assert_eq!(state.protocol_version.as_deref(), Some("2024-10-01"));
    }

    #[tokio::test]
    async fn test_tools_list_omits_output_schema_for_legacy_clients() {
        let app_state = setup_app_state_with_tool(
            "2024-10-01",
            "get__v1_test",
            Some(json!({ "type": "object" }))
        ).await;

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/list".to_string(),
            params: None,
        };

        let (resp, _) = handle_request(req, &app_state).await;
        let response = resp.unwrap();
        let tools = response.result.unwrap().get("tools").unwrap().as_array().unwrap().clone();
        
        let tool = tools.iter().find(|t| t.get("name").unwrap().as_str().unwrap() == "get__v1_test").unwrap();
        assert!(tool.get("outputSchema").is_none());
        
        let list_tool = tools.iter().find(|t| t.get("name").unwrap().as_str().unwrap() == "cowen_api_list").unwrap();
        assert!(list_tool.get("outputSchema").is_none());
    }

    #[tokio::test]
    async fn test_tools_list_includes_output_schema_for_modern_clients() {
        let app_state = setup_app_state_with_tool(
            "2025-06-18",
            "get__v1_test",
            Some(json!({ "type": "object" }))
        ).await;

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/list".to_string(),
            params: None,
        };

        let (resp, _) = handle_request(req, &app_state).await;
        let response = resp.unwrap();
        let tools = response.result.unwrap().get("tools").unwrap().as_array().unwrap().clone();
        
        let tool = tools.iter().find(|t| t.get("name").unwrap().as_str().unwrap() == "get__v1_test").unwrap();
        assert!(tool.get("outputSchema").is_some());
        
        let list_tool = tools.iter().find(|t| t.get("name").unwrap().as_str().unwrap() == "cowen_api_list").unwrap();
        assert!(list_tool.get("outputSchema").is_some());
    }

    #[tokio::test]
    async fn test_tools_list_omits_output_schema_for_legacy_version_2024_11_05() {
        let app_state = setup_app_state_with_tool(
            "2024-11-05",
            "get__v1_test",
            Some(json!({ "type": "object" }))
        ).await;

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/list".to_string(),
            params: None,
        };

        let (resp, _) = handle_request(req, &app_state).await;
        let tools = resp.unwrap().result.unwrap().get("tools").unwrap().as_array().unwrap().clone();
        
        let tool = tools.iter().find(|t| t.get("name").unwrap().as_str().unwrap() == "get__v1_test").unwrap();
        assert!(tool.get("outputSchema").is_none());
    }

    #[tokio::test]
    async fn test_tools_list_includes_output_schema_for_modern_version_2026_01_01() {
        let app_state = setup_app_state_with_tool(
            "2026-01-01",
            "get__v1_test",
            Some(json!({ "type": "object" }))
        ).await;

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/list".to_string(),
            params: None,
        };

        let (resp, _) = handle_request(req, &app_state).await;
        let tools = resp.unwrap().result.unwrap().get("tools").unwrap().as_array().unwrap().clone();
        
        let tool = tools.iter().find(|t| t.get("name").unwrap().as_str().unwrap() == "get__v1_test").unwrap();
        assert!(tool.get("outputSchema").is_some());
    }

    #[tokio::test]
    async fn test_tools_call_omits_structured_content_for_legacy_version_2024_11_05() {
        let app_state = setup_app_state_with_tool(
            "2024-11-05",
            "get__v1_test",
            Some(json!({ "type": "object", "properties": { "data": { "type": "string" } } }))
        ).await;

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/call".to_string(),
            params: Some(json!({ "name": "get__v1_test", "arguments": {} })),
        };

        let (resp, _) = handle_request(req, &app_state).await;
        let result = resp.unwrap().result.unwrap();
        
        assert!(result.get("structuredContent").is_none());
    }
}
