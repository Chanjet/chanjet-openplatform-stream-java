use crate::client::{get_grpc_client, inject_auth, proto};
use crate::protocol::{AppState, EnabledTool, generate_tool_name};
use crate::schema::build_schema_from_openapi;
use serde_json::json;

pub async fn fetch_apis(
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
        .map_err(|e| crate::client::handle_grpc_status(e))?;
    
    let inner = resp.into_inner();
    if let Some(err) = inner.error_message {
        return Err(format!("Error: {}", err));
    }

    let apis: Vec<serde_json::Value> = serde_json::from_str(&inner.json).unwrap_or_default();
    Ok((inner.total, apis))
}

pub async fn handle_api_list(
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
        .unwrap_or(20) as u32;

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
                let tool_name = generate_tool_name(method, path);

                text.push_str(&format!("- [{}] {} {} ({})\n", tool_name, method, path, summary));
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

pub async fn handle_enable_api(
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
        crate::protocol::EnabledTool {
            method: method.clone(),
            path: path.clone(),
            description: summary.clone(),
            input_schema: input_schema.clone(),
            output_schema: output_schema.clone(),
            body_params,
        },
    );
    drop(state);

    let mut tool_json = json!({
        "name": target_tool_name,
        "description": format!("Dynamic API: {} {}\n{}", method, path, summary),
        "inputSchema": input_schema
    });

    if let Some(out_schema) = &output_schema {
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

    let msg = format!(
        "Successfully enabled tool '{}'.\n\nTool Definition:\n{}",
        target_tool_name,
        serde_json::to_string_pretty(&tool_json).unwrap_or_default()
    );
    
    (
        msg.clone(),
        false,
        Some(tool_json),
    )
}

pub async fn handle_disable_api(
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
