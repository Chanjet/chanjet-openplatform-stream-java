use crate::client::{get_grpc_client, inject_auth, proto};
use crate::protocol::{AppState, EnabledTool};
use crate::schema::{validate_json_against_schema, get_type_name};
use regex::Regex;

pub fn prepare_request_params(
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

pub async fn handle_dynamic_tool_call(
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
                let mut result_text = format!("Status: {}\n{}", inner.status, inner.body);
                let mut is_err = false;
                let mut structured_content = None;
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&inner.body) {
                    let mut matched = true;
                    if let Some(out_schema) = &tool_def.output_schema {
                        if let Err(e) = validate_json_against_schema(&json_val, out_schema) {
                            matched = false;
                            eprintln!("DEBUG: MCP Tool output schema validation failed: {}", e);
                            result_text = format!("Schema Validation Error: {}\nResponse Body: {}", e, inner.body);
                            is_err = true;
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
                (result_text, is_err, structured_content)
            }
        }
        Err(e) => (format!("gRPC Error: {}", e), true, None),
    }
}
