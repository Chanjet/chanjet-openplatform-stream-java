use crate::client::{get_grpc_client, inject_auth, proto};
use crate::protocol::{AppState, EnabledTool, JsonRpcError, JsonRpcRequest, JsonRpcResponse, generate_tool_name};
use crate::schema::{build_schema_from_openapi, validate_json_against_schema, get_type_name};
use regex::Regex;
use serde_json::json;

pub async fn handle_request(
    req: JsonRpcRequest,
    app_state: &AppState,
) -> (Option<JsonRpcResponse>, bool) {
    if req.jsonrpc != "2.0" {
        return (None, false);
    }

    let mut list_changed = false;

    let response = match req.method.as_str() {
        "initialize" => Some(JsonRpcResponse {
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
        }),
        "notifications/initialized" => None,

        "tools/list" => {
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

            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(json!({ "tools": tools })),
                error: None,
            })
        }
        "tools/call" => {
            let params = req.params.unwrap_or_default();
            let name = params["name"].as_str().unwrap_or("");
            let args = params["arguments"].as_object().cloned().unwrap_or_default();

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

            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(result_obj),
                error: None,
            })
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

async fn fetch_apis(
    app_state: &AppState,
    search: Option<String>,
    page: u32,
    page_size: u32,
) -> Result<(u32, Vec<serde_json::Value>), String> {
    let mut client = get_grpc_client().await?;
    let grpc_req = proto::ApiListRequest {
        profile: app_state.profile.clone(),
        search,
        page,
        page_size,
        refresh: false,
    };

    let resp = client.api_list(inject_auth(grpc_req)).await
        .map_err(|e| format!("gRPC Error listing APIs: {}", e))?;
    
    let inner = resp.into_inner();
    if let Some(err) = inner.error_message {
        return Err(format!("Error: {}", err));
    }

    let apis: Vec<serde_json::Value> = serde_json::from_str(&inner.json).unwrap_or_default();
    Ok((inner.total, apis))
}

fn prepare_request_params(
    tool_def: &EnabledTool,
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<(String, Option<String>), String> {
    let mut final_path = tool_def.path.clone();
    let mut query_params = Vec::new();
    let mut body_str = None;

    let re = Regex::new(r"\{([a-zA-Z0-9_]+)\}").unwrap();
    let path_vars: std::collections::HashSet<String> = re
        .captures_iter(&tool_def.path)
        .map(|cap| cap[1].to_string())
        .collect();

    let mut body_obj = serde_json::Map::new();

    for (k, v) in args {
        if tool_def.body_params.contains(k) {
            if k == "body_payload" {
                body_str = serde_json::to_string(v)
                    .map_err(|e| format!("Failed to serialize body_payload: {}", e))?
                    .into();
            } else {
                body_obj.insert(k.clone(), v.clone());
            }
        } else if path_vars.contains(k) {
            if let Some(val_str) = v.as_str() {
                final_path = final_path.replace(&format!("{{{}}}", k), val_str);
            } else {
                final_path = final_path.replace(&format!("{{{}}}", k), &v.to_string());
            }
        } else {
            let val_str = v
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| v.to_string());
            query_params.push(format!("{}={}", k, urlencoding::encode(&val_str)));
        }
    }

    if !body_obj.is_empty() {
        body_str = serde_json::to_string(&body_obj)
            .map_err(|e| format!("Failed to serialize request body properties: {}", e))?
            .into();
    }

    if !query_params.is_empty() {
        if final_path.contains('?') {
            final_path = format!("{}&{}", final_path, query_params.join("&"));
        } else {
            final_path = format!("{}?{}", final_path, query_params.join("&"));
        }
    }

    Ok((final_path, body_str))
}

async fn handle_api_list(
    args: &serde_json::Map<String, serde_json::Value>,
    app_state: &AppState,
) -> (String, bool, Option<serde_json::Value>) {
    let search = args
        .get("search")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());
    let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1) as u32;
    let page_size = args
        .get("page_size")
        .and_then(|v| v.as_i64())
        .unwrap_or(1000) as u32;

    match fetch_apis(app_state, search, page, page_size).await {
        Ok((total, apis)) => {
            let mut text = format!("Total APIs found: {}\n", total);
            let mut items = Vec::new();
            for api in apis {
                let method = api.get("method").and_then(|v| v.as_str()).unwrap_or("");
                let path = api.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let summary = api.get("summary").and_then(|v| v.as_str()).unwrap_or("");
                let description = api.get("description").and_then(|v| v.as_str()).unwrap_or("");
                let score = api.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);

                text.push_str(&format!("- {} {} ({})\n", method, path, summary));

                let tool_name = generate_tool_name(method, path);
                items.push(json!({
                    "tool_name": tool_name,
                    "method": method,
                    "path": path,
                    "summary": summary,
                    "description": description,
                    "score": score
                }));
            }
            let obj_val = json!({
                "total": total,
                "apis": items
            });
            (text, false, Some(obj_val))
        }
        Err(e) => (e, true, None),
    }
}

async fn handle_enable_api(
    args: &serde_json::Map<String, serde_json::Value>,
    app_state: &AppState,
) -> (String, bool, Option<serde_json::Value>) {
    let target_tool_name = args.get("tool_name").and_then(|s| s.as_str()).unwrap_or("");

    let mut method = String::new();
    let mut path = String::new();
    let mut api_found = false;

    match fetch_apis(app_state, None, 1, 1000).await {
        Ok((_, apis)) => {
            for api in apis {
                let m = api.get("method").and_then(|v| v.as_str()).unwrap_or("");
                let p = api.get("path").and_then(|v| v.as_str()).unwrap_or("");
                if generate_tool_name(m, p) == target_tool_name {
                    method = m.to_string();
                    path = p.to_string();
                    api_found = true;
                    break;
                }
            }
        }
        Err(e) => return (e, true, None),
    }

    if !api_found {
        return (format!("API for tool_name '{}' not found", target_tool_name), true, None);
    }

    let mut client = match get_grpc_client().await {
        Ok(c) => c,
        Err(e) => return (format!("gRPC Error: {}", e), true, None),
    };

    let grpc_req_spec = proto::ApiSpecRequest {
        profile: app_state.profile.clone(),
        method: method.clone(),
        path: path.clone(),
    };

    let spec_json_str = match client.api_spec(inject_auth(grpc_req_spec)).await {
        Ok(resp) => {
            let inner = resp.into_inner();
            if let Some(_err) = inner.error_message {
                "{}".to_string()
            } else {
                inner.json
            }
        }
        Err(_) => "{}".to_string(),
    };

    let spec: serde_json::Value = serde_json::from_str(&spec_json_str).unwrap_or(json!({}));

    let summary = spec
        .get("operation")
        .map(|op| {
            let s = op.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            let d = op.get("description").and_then(|v| v.as_str()).unwrap_or("");
            if !s.is_empty() && !d.is_empty() {
                format!("{} - {}", s, d)
            } else if !s.is_empty() {
                s.to_string()
            } else {
                d.to_string()
            }
        })
        .unwrap_or_default();

    let (input_schema, output_schema, body_params) = build_schema_from_openapi(&path, &spec);

    let mut state = app_state.mcp_state.lock().await;
    state.tools.insert(
        target_tool_name.to_string(),
        EnabledTool {
            method,
            path,
            description: summary,
            input_schema,
            output_schema,
            body_params,
        },
    );
    drop(state);

    let msg = format!(
        "Successfully enabled tool '{}'. Tools list changed notification sent.",
        target_tool_name
    );
    
    (
        msg.clone(),
        false,
        Some(json!({
            "success": true,
            "tool_name": target_tool_name,
            "message": msg
        })),
    )
}

async fn handle_disable_api(
    args: &serde_json::Map<String, serde_json::Value>,
    app_state: &AppState,
) -> (String, bool, Option<serde_json::Value>) {
    let target_tool_name = args.get("tool_name").and_then(|s| s.as_str()).unwrap_or("");

    let mut state = app_state.mcp_state.lock().await;
    let removed = state.tools.remove(target_tool_name);
    drop(state);

    if removed.is_some() {
        let msg = format!("Successfully disabled tool '{}'.", target_tool_name);
        (
            msg.clone(),
            false,
            Some(json!({
                "success": true,
                "tool_name": target_tool_name,
                "message": msg
            })),
        )
    } else {
        let msg = format!("Tool '{}' was not enabled.", target_tool_name);
        (
            msg.clone(),
            true,
            Some(json!({
                "success": false,
                "tool_name": target_tool_name,
                "message": msg
            })),
        )
    }
}

async fn handle_dynamic_tool_call(
    name: &str,
    args: &serde_json::Map<String, serde_json::Value>,
    app_state: &AppState,
) -> (String, bool, Option<serde_json::Value>) {
    let state = app_state.mcp_state.lock().await;
    let tool_def = match state.tools.get(name).cloned() {
        Some(td) => td,
        None => return (format!("Tool {} not found", name), true, None),
    };
    drop(state);

    let (final_path, body_str) = match prepare_request_params(&tool_def, args) {
        Ok(params) => params,
        Err(e) => return (e, true, None),
    };

    let mut client = match get_grpc_client().await {
        Ok(c) => c,
        Err(e) => return (format!("gRPC Error: {}", e), true, None),
    };

    let grpc_req = proto::CallApiRequest {
        profile: app_state.profile.clone(),
        method: tool_def.method,
        path: final_path,
        data: body_str,
        force: false,
    };

    match client.call_api(inject_auth(grpc_req)).await {
        Ok(resp) => {
            let inner = resp.into_inner();
            if let Some(err) = inner.error_message {
                (format!("Error: {}", err), true, None)
            } else {
                let result_text = format!("Status: {}\n{}", inner.status, inner.body);
                let mut structured_content = None;
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&inner.body) {
                    let mut matched = true;
                    if let Some(out_schema) = &tool_def.output_schema {
                        if let Err(e) = validate_json_against_schema(&json_val, out_schema) {
                            matched = false;
                            eprintln!("DEBUG: MCP Tool output schema validation failed: {}", e);
                        }
                    } else if !json_val.is_object() {
                        matched = false;
                    }
                    if matched {
                        if json_val.is_object() {
                            structured_content = Some(json_val);
                        } else {
                            eprintln!("DEBUG: MCP Tool output schema validation succeeded but payload is not a JSON Object (record), skipping structuredContent. Payload type: {}", get_type_name(&json_val));
                        }
                    }
                }
                (result_text, false, structured_content)
            }
        }
        Err(e) => (format!("gRPC Error: {}", e), true, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_dynamic_tool_structured_content_fallback() {
        let app_state = AppState::new("test_tenant".to_string());
        
        let mut state = app_state.mcp_state.lock().await;
        state.tools.insert(
            "get__v1_test".to_string(),
            EnabledTool {
                method: "GET".to_string(),
                path: "/v1/test".to_string(),
                description: "Test description".to_string(),
                input_schema: json!({ "type": "object", "properties": {} }),
                output_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "data": { "type": "string" }
                    }
                })),
                body_params: vec![],
            },
        );
        drop(state);

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
        
        assert!(result.get("structuredContent").is_none());
    }
}
