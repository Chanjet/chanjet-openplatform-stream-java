use super::api::{handle_api_list, handle_disable_api, handle_enable_api};
use super::dynamic::handle_dynamic_tool_call;
use crate::protocol::{AppState, JsonRpcRequest, JsonRpcResponse};
use serde_json::json;

use crate::capabilities::McpFeature;

pub async fn handle_tools_list(req: JsonRpcRequest, app_state: &AppState) -> JsonRpcResponse {
    let state_lock = app_state.mcp_state.lock().await;
    let supports_output_schema = state_lock.supports_feature(&McpFeature::OutputSchema);
    drop(state_lock);

    let mut tools = vec![
        json!({
            "name": "cowen_api_list",
            "description": "Lists all available Cowen APIs by searching keywords. (Step 1 of the API orchestration workflow: find the tool_name you want to enable, then call cowen_enable_api)",
                        "inputSchema": {
                "type": "object",
                "properties": {
                    "search": { "type": "string", "description": "Optional search query" },
                    "page": { "type": "integer", "description": "Optional page number, default 1" },
                    "page_size": { "type": "integer", "description": "Optional page size, default 20" }
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
            "description": "Enable a specific Cowen API as a standalone MCP Tool so you can call it. (Step 2 of the API orchestration workflow: register a tool_name found via cowen_api_list to expose it as a standard MCP tool. Once registered, you can directly invoke it)",
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
            "description": "Disable a previously enabled Cowen API Tool. (Step 3 of the API orchestration workflow: deregister dynamic tools when no longer needed)",
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
            let is_object_schema = out_schema
                .get("type")
                .and_then(|t| t.as_str())
                .map(|t| t == "object")
                .unwrap_or(true);
            if is_object_schema {
                tool_json
                    .as_object_mut()
                    .unwrap()
                    .insert("outputSchema".to_string(), out_schema.clone());
            } else {
                let wrapped = json!({
                    "type": "object",
                    "properties": {
                        "value": out_schema.clone()
                    },
                    "required": ["value"]
                });
                tool_json
                    .as_object_mut()
                    .unwrap()
                    .insert("outputSchema".to_string(), wrapped);
            }
        }
        tools.push(tool_json);
    }
    drop(state);

    if !supports_output_schema {
        for tool in tools.iter_mut() {
            if let Some(obj) = tool.as_object_mut() {
                obj.remove("outputSchema");
            }
        }
    }

    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: req.id,
        result: Some(json!({ "tools": tools })),
        error: None,
    }
}

pub async fn handle_tools_call(
    req: JsonRpcRequest,
    app_state: &AppState,
) -> (JsonRpcResponse, bool) {
    let params = req.params.unwrap_or_default();
    let name = params["name"].as_str().unwrap_or("");
    let args = params["arguments"].as_object().cloned().unwrap_or_default();

    let mut list_changed = false;

    let (result_text, is_error, mut structured_content, schema_error) = match name {
        "cowen_api_list" => {
            let res = handle_api_list(&args, app_state).await;
            (res.0, res.1, res.2, None)
        }
        "cowen_enable_api" => {
            let res = handle_enable_api(&args, app_state).await;
            if !res.1 {
                list_changed = true;
            }
            (res.0, res.1, res.2, None)
        }
        "cowen_disable_api" => {
            let res = handle_disable_api(&args, app_state).await;
            if !res.1 {
                list_changed = true;
            }
            (res.0, res.1, res.2, None)
        }
        _ => handle_dynamic_tool_call(name, &args, app_state).await,
    };

    let supports_output_schema = {
        let state = app_state.mcp_state.lock().await;
        state.supports_feature(&crate::capabilities::McpFeature::OutputSchema)
    };

    let mut final_text = result_text;

    if supports_output_schema {
        let has_output_schema = match name {
            "cowen_api_list" | "cowen_enable_api" | "cowen_disable_api" => true,
            _ => {
                let state = app_state.mcp_state.lock().await;
                state
                    .tools
                    .get(name)
                    .and_then(|t| t.output_schema.as_ref())
                    .is_some()
            }
        };

        if has_output_schema && structured_content.is_none() {
            structured_content = Some(json!({}));
        }
    } else {
        if let Some(structured) = &structured_content {
            final_text = serde_json::to_string_pretty(structured).unwrap_or(final_text);
        }
        structured_content = None;
    }

    let mut content_items = vec![json!({
        "type": "text",
        "text": final_text
    })];

    if let Some(err_msg) = schema_error {
        content_items.push(json!({
            "type": "text",
            "text": err_msg
        }));
    }

    let mut result_obj = json!({
        "content": content_items,
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
