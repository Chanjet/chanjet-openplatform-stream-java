use crate::client::{get_grpc_client, inject_auth, proto};
use crate::protocol::{AppState, EnabledTool};
use crate::schema::{get_type_name, validate_json_against_schema};
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
) -> (String, bool, Option<serde_json::Value>, Option<String>) {
    let state = app_state.mcp_state.lock().await;
    let tool_def = match state.tools.get(name).cloned() {
        Some(td) => td,
        None => return (format!("Tool {} not found", name), true, None, None),
    };
    drop(state);

    let (final_path, body_str) = match prepare_request_params(&tool_def, args) {
        Ok(params) => params,
        Err(e) => return (e, true, None, None),
    };

    let mut client = match get_grpc_client().await {
        Ok(c) => c,
        Err(e) => return (format!("gRPC Error: {}", e), true, None, None),
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
                (format!("Error: {}", err), true, None, None)
            } else {
                let (result_text, is_err, structured_content, schema_error) =
                    process_api_response(inner.status, &inner.body, &tool_def.output_schema);
                (result_text, is_err, structured_content, schema_error)
            }
        }
        Err(e) => (crate::client::handle_grpc_status(e), true, None, None),
    }
}

pub fn process_api_response(
    status: u32,
    body: &str,
    output_schema: &Option<serde_json::Value>,
) -> (String, bool, Option<serde_json::Value>, Option<String>) {
    let result_text = format!("Status: {}\n{}", status, body);
    let mut is_err = false;
    let mut structured_content = None;
    let mut schema_error = None;

    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(out_schema) = output_schema {
            let is_object_schema = out_schema
                .get("type")
                .and_then(|t| t.as_str())
                .map(|t| t == "object")
                .unwrap_or(true);

            match validate_json_against_schema(&json_val, out_schema) {
                Ok(_) => {
                    if is_object_schema {
                        if json_val.is_object() {
                            structured_content = Some(json_val);
                        } else {
                            eprintln!("DEBUG: MCP Tool output schema validation succeeded but payload is not a JSON Object (record), skipping structuredContent. Payload type: {}", get_type_name(&json_val));
                        }
                    } else {
                        structured_content = Some(serde_json::json!({
                            "value": json_val
                        }));
                    }
                }
                Err(e) => {
                    eprintln!("DEBUG: MCP Tool output schema validation failed: {}", e);
                    schema_error = Some(format!("Schema Validation Error: {}", e));
                    is_err = true;
                }
            }
        } else {
            if json_val.is_object() {
                structured_content = Some(json_val);
            }
        }
    }

    (result_text, is_err, structured_content, schema_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_process_api_response_schema_validation_failure() {
        let status = 200;
        let output_schema = Some(json!({
            "type": "array",
            "items": { "type": "string" }
        }));
        let body = r#"{"data": "not an array"}"#;

        let (result_text, is_err, structured, schema_err) =
            process_api_response(status, body, &output_schema);

        assert_eq!(result_text, "Status: 200\n{\"data\": \"not an array\"}");
        assert_eq!(is_err, true);
        assert!(structured.is_none());
        assert!(schema_err.is_some());
        assert_eq!(
            schema_err.unwrap(),
            "Schema Validation Error: Expected array, found object"
        );
    }
}
