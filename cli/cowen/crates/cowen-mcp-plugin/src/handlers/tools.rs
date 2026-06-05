use crate::protocol::{AppState, JsonRpcRequest, JsonRpcResponse};
use super::api::{handle_api_list, handle_enable_api, handle_disable_api};
use super::dynamic::handle_dynamic_tool_call;
use serde_json::json;

pub async fn handle_tools_list(req: JsonRpcRequest, app_state: &AppState) -> JsonRpcResponse {
    let mut tools = vec![
        json!({
            "name": "cowen_api_list",
            "description": "Lists all available Cowen APIs by searching keywords.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "search": { "type": "string", "description": "Optional search query" },
                    "page": { "type": "integer", "description": "Optional page number, default 1" },
                    "page_size": { "type": "integer", "description": "Optional page size, default 1000" }
                }
            },
            "outputSchema": {
                "type": "object",
                "properties": {
                    "total": { "type": "integer" },
                    "apis": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "tool_name": { "type": "string" },
                                "method": { "type": "string" },
                                "path": { "type": "string" },
                                "summary": { "type": "string" },
                                "description": { "type": "string" },
                                "score": { "type": "number" }
                            }
                        }
                    }
                }
            }
        }),
        json!({
            "name": "cowen_enable_api",
            "description": "Enable a specific Cowen API as a standalone MCP Tool so you can call it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_name": { "type": "string", "description": "The tool_name of the API returned by cowen_api_list" }
                },
                "required": ["tool_name"]
            },
            "outputSchema": {
                "type": "object",
                "properties": {
                    "success": { "type": "boolean" },
                    "tool_name": { "type": "string" },
                    "message": { "type": "string" }
                }
            }
        }),
        json!({
            "name": "cowen_disable_api",
            "description": "Disable a previously enabled Cowen API Tool.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_name": { "type": "string", "description": "The tool_name of the enabled API" }
                },
                "required": ["tool_name"]
            },
            "outputSchema": {
                "type": "object",
                "properties": {
                    "success": { "type": "boolean" },
                    "tool_name": { "type": "string" },
                    "message": { "type": "string" }
                }
            }
        }),
    ];

    let state = app_state.mcp_state.lock().await;
    for (tool_name, tool_def) in state.tools.iter() {
        let mut tool_json = json!({
            "name": tool_name,
            "description": format!("Dynamic API: {} {}\n{}", tool_def.method, tool_def.path, tool_def.description),
            "inputSchema": tool_def.input_schema
        });
        if let Some(out_schema) = &tool_def.output_schema {
            tool_json
                .as_object_mut()
                .unwrap()
                .insert("outputSchema".to_string(), out_schema.clone());
        }
        tools.push(tool_json);
    }
    drop(state);

    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: req.id,
        result: Some(json!({ "tools": tools })),
        error: None,
    }
}

pub async fn handle_tools_call(req: JsonRpcRequest, app_state: &AppState) -> (JsonRpcResponse, bool) {
    let params = req.params.unwrap_or_default();
    let name = params["name"].as_str().unwrap_or("");
    let args = params["arguments"].as_object().cloned().unwrap_or_default();

    let mut list_changed = false;

    let (result_text, is_error, structured_content) = match name {
        "cowen_api_list" => handle_api_list(&args, app_state).await,
        "cowen_enable_api" => {
            let res = handle_enable_api(&args, app_state).await;
            if !res.1 {
                list_changed = true;
            }
            res
        }
        "cowen_disable_api" => {
            let res = handle_disable_api(&args, app_state).await;
            if !res.1 {
                list_changed = true;
            }
            res
        }
        _ => handle_dynamic_tool_call(name, &args, app_state).await,
    };

    let mut result_obj = json!({
        "content": [
            {
                "type": "text",
                "text": result_text
            }
        ],
        "isError": is_error
    });

    if let Some(structured) = structured_content {
        result_obj
            .as_object_mut()
            .unwrap()
            .insert("structuredContent".to_string(), structured);
    }

    let response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: req.id,
        result: Some(result_obj),
        error: None,
    };

    (response, list_changed)
}
