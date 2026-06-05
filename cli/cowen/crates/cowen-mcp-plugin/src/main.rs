use clap::{Parser, Subcommand};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

pub mod proto {
    tonic::include_proto!("cowen.daemon.v1");
}
use proto::cowen_daemon_service_client::CowenDaemonServiceClient;

#[derive(Parser, Debug)]
#[command(author, version, about = "Cowen MCP Plugin")]
struct Cli {
    #[arg(short, long, env = "COWEN_PROFILE", default_value = "default")]
    profile: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 启动 MCP Server (标准 stdio 交互模式)
    Server,

    /// 获取连接此 MCP 插件的 stdio 配置 JSON，用于配置 Cursor 等 IDE
    Config,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcNotification {
    jsonrpc: String,
    method: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnabledTool {
    method: String,
    path: String,
    description: String,
    input_schema: serde_json::Value,
    output_schema: Option<serde_json::Value>,
    body_params: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct McpState {
    tools: HashMap<String, EnabledTool>,
}

struct AppState {
    profile: String,
    mcp_state: Arc<Mutex<McpState>>,
}

impl AppState {
    fn new(profile: String) -> Self {
        Self {
            profile,
            mcp_state: Arc::new(Mutex::new(McpState::default())),
        }
    }
}

fn generate_tool_name(method: &str, path: &str) -> String {
    let clean_path = path
        .replace("/", "_")
        .replace("{", "")
        .replace("}", "")
        .replace("-", "_");
    let name = format!("{}_{}", method.to_lowercase(), clean_path);
    name.trim_matches('_').to_string()
}

fn resolve_refs(schema: &mut serde_json::Value, components: &serde_json::Value, depth: usize) {
    if depth > 10 {
        return;
    }

    let mut resolved_val = None;
    if let Some(obj) = schema.as_object() {
        if let Some(ref_val) = obj.get("$ref").and_then(|v| v.as_str()) {
            if ref_val.starts_with("#/components/") {
                let parts: Vec<&str> = ref_val
                    .trim_start_matches("#/components/")
                    .split('/')
                    .collect();
                let mut current = components;
                let mut found = true;
                for p in parts {
                    if let Some(next) = current.get(p) {
                        current = next;
                    } else {
                        found = false;
                        break;
                    }
                }
                if found {
                    resolved_val = Some(current.clone());
                }
            }
        }
    }

    if let Some(mut new_val) = resolved_val {
        resolve_refs(&mut new_val, components, depth + 1);
        *schema = new_val;
        return;
    }

    if let Some(obj) = schema.as_object_mut() {
        for (_, v) in obj.iter_mut() {
            resolve_refs(v, components, depth + 1);
        }
    } else if let Some(arr) = schema.as_array_mut() {
        for v in arr.iter_mut() {
            resolve_refs(v, components, depth + 1);
        }
    }
}

fn build_schema_from_openapi(
    path: &str,
    spec: &serde_json::Value,
) -> (serde_json::Value, Option<serde_json::Value>, Vec<String>) {
    let operation = spec.get("operation").unwrap_or(spec);
    let empty_components = json!({});
    let components = spec.get("components").unwrap_or(&empty_components);

    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    if let Some(params) = operation.get("parameters").and_then(|p| p.as_array()) {
        for param in params {
            let mut param_obj = param.clone();
            resolve_refs(&mut param_obj, components, 0);

            if let Some(name) = param_obj.get("name").and_then(|n| n.as_str()) {
                let is_req = param_obj
                    .get("required")
                    .and_then(|r| r.as_bool())
                    .unwrap_or(false);
                let param_in = param_obj.get("in").and_then(|i| i.as_str()).unwrap_or("");

                if param_in == "path" || param_in == "query" {
                    let mut prop_schema = param_obj
                        .get("schema")
                        .cloned()
                        .unwrap_or(json!({ "type": "string" }));
                    resolve_refs(&mut prop_schema, components, 0);

                    if let Some(desc) = param_obj.get("description") {
                        if let Some(obj) = prop_schema.as_object_mut() {
                            obj.insert("description".to_string(), desc.clone());
                        }
                    }
                    properties.insert(name.to_string(), prop_schema);
                    if is_req || param_in == "path" {
                        required.push(name.to_string());
                    }
                }
            }
        }
    }

    let re = Regex::new(r"\{([a-zA-Z0-9_]+)\}").unwrap();
    for cap in re.captures_iter(path) {
        let param = cap[1].to_string();
        if !properties.contains_key(&param) {
            properties.insert(
                param.clone(),
                json!({
                    "type": "string",
                    "description": format!("Path parameter: {}", param)
                }),
            );
            required.push(param);
        }
    }

    let mut body_params = Vec::new();

    if let Some(req_body) = operation.get("requestBody") {
        let mut body_obj = req_body.clone();
        resolve_refs(&mut body_obj, components, 0);

        let is_body_req = body_obj
            .get("required")
            .and_then(|r| r.as_bool())
            .unwrap_or(false);
        let req_desc = body_obj
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("");

        if let Some(schema) = body_obj
            .get("content")
            .and_then(|c| c.get("application/json"))
            .and_then(|j| j.get("schema"))
        {
            let mut body_schema = schema.clone();
            resolve_refs(&mut body_schema, components, 0);

            // If body is an object, flatten its properties into the root schema
            let is_object = body_schema.get("type").and_then(|t| t.as_str()) == Some("object");
            let has_properties = body_schema
                .get("properties")
                .and_then(|p| p.as_object())
                .is_some();

            if is_object && has_properties {
                if let Some(body_props) = body_schema.get("properties").and_then(|p| p.as_object())
                {
                    for (k, v) in body_props {
                        properties.insert(k.clone(), v.clone());
                        body_params.push(k.clone());
                    }
                }
                if let Some(body_req) = body_schema.get("required").and_then(|r| r.as_array()) {
                    for req_key in body_req {
                        if let Some(req_str) = req_key.as_str() {
                            if is_body_req {
                                required.push(req_str.to_string());
                            }
                        }
                    }
                }
            } else {
                // Not an object or no properties, fallback to body_payload
                let schema_desc = body_schema
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                let mut final_desc = String::from("JSON payload for the request body. ");
                if !req_desc.is_empty() {
                    final_desc.push_str(req_desc);
                    final_desc.push_str(" ");
                }
                if !schema_desc.is_empty() && schema_desc != req_desc {
                    final_desc.push_str(schema_desc);
                }

                if let Some(obj) = body_schema.as_object_mut() {
                    obj.insert("description".to_string(), json!(final_desc.trim()));
                }
                properties.insert("body_payload".to_string(), body_schema);
                body_params.push("body_payload".to_string());
                if is_body_req {
                    required.push("body_payload".to_string());
                }
            }
        }
    }

    let mut schema = json!({
        "type": "object",
        "properties": properties,
    });

    if !required.is_empty() {
        required.sort();
        required.dedup();
        schema
            .as_object_mut()
            .unwrap()
            .insert("required".to_string(), json!(required));
    }

    let mut output_schema = None;
    let mut ok_resp = None;
    if let Some(responses) = operation.get("responses").and_then(|r| r.as_object()) {
        for key in &["200", "201", "202", "204", "default"] {
            if let Some(resp) = responses.get(*key) {
                ok_resp = Some(resp.clone());
                break;
            }
        }
        if ok_resp.is_none() {
            for (k, v) in responses {
                if k.starts_with('2') || k == "default" {
                    ok_resp = Some(v.clone());
                    break;
                }
            }
        }
    }

    if let Some(mut resp_obj) = ok_resp {
        resolve_refs(&mut resp_obj, components, 0);
        let mut schema_val = None;
        if let Some(content) = resp_obj.get("content").and_then(|c| c.as_object()) {
            for (mime, media_type) in content {
                if mime.starts_with("application/json") || mime.contains("json") {
                    if let Some(s) = media_type.get("schema") {
                        schema_val = Some(s.clone());
                        break;
                    }
                }
            }
            if schema_val.is_none() {
                for (_, media_type) in content {
                    if let Some(s) = media_type.get("schema") {
                        schema_val = Some(s.clone());
                        break;
                    }
                }
            }
        }

        if let Some(mut schema) = schema_val {
            resolve_refs(&mut schema, components, 0);
            output_schema = Some(schema);
        }
    }

    (schema, output_schema, body_params)
}

async fn get_grpc_client() -> Result<CowenDaemonServiceClient<tonic::transport::Channel>, String> {
    let port_str = std::env::var("COWEN_IPC_PORT")
        .map_err(|_| "Missing COWEN_IPC_PORT env var".to_string())?;
    let endpoint = format!("http://127.0.0.1:{}", port_str);
    CowenDaemonServiceClient::connect(endpoint)
        .await
        .map_err(|e| e.to_string())
}

fn inject_auth<T>(req: T) -> tonic::Request<T> {
    let mut request = tonic::Request::new(req);
    if let Ok(token) = std::env::var("COWEN_BRIDGE_TOKEN") {
        if let Ok(meta_value) = format!("Bearer {}", token).parse() {
            request.metadata_mut().insert("authorization", meta_value);
        }
    }
    request
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Config => {
            let config_json = serde_json::json!({
                "mcpServers": {
                    "cowen-mcp": {
                        "command": "cowen",
                        "args": ["plugins", "run", "cowen-mcp-plugin", "server"]
                    }
                }
            });
            println!("{}", serde_json::to_string_pretty(&config_json)?);
        }
        Commands::Server => {
            let app_state = AppState::new(cli.profile);

            let mut reader = BufReader::new(tokio::io::stdin());
            let mut writer = tokio::io::stdout();
            let mut line = String::new();

            while reader.read_line(&mut line).await? > 0 {
                let req: Result<JsonRpcRequest, _> = serde_json::from_str(&line);
                if let Ok(req) = req {
                    let (resp, should_notify) = handle_request(req, &app_state).await;

                    if let Some(r) = resp {
                        let resp_str = serde_json::to_string(&r)?;
                        writer.write_all(resp_str.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                        writer.flush().await?;
                    }

                    if should_notify {
                        let notification = JsonRpcNotification {
                            jsonrpc: "2.0".to_string(),
                            method: "notifications/tools/list_changed".to_string(),
                        };
                        let notif_str = serde_json::to_string(&notification)?;
                        writer.write_all(notif_str.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                        writer.flush().await?;
                    }
                }
                line.clear();
            }
        }
    }

    Ok(())
}

async fn handle_request(
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

            #[allow(unused_assignments)]
            let mut result_text = String::new();
            let mut is_error = false;
            let mut structured_content: Option<serde_json::Value> = None;

            if name == "cowen_api_list" {
                let search = args
                    .get("search")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string());
                let page = args.get("page").and_then(|v| v.as_i64()).unwrap_or(1) as u32;
                let page_size = args
                    .get("page_size")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(1000) as u32;
                match get_grpc_client().await {
                    Ok(mut client) => {
                        let grpc_req = proto::ApiListRequest {
                            profile: app_state.profile.clone(),
                            search,
                            page,
                            page_size,
                            refresh: false,
                        };
                        match client.api_list(inject_auth(grpc_req)).await {
                            Ok(resp) => {
                                let inner = resp.into_inner();
                                if let Some(err) = inner.error_message {
                                    result_text = format!("Error: {}", err);
                                    is_error = true;
                                } else {
                                    let apis: Vec<serde_json::Value> =
                                        serde_json::from_str(&inner.json).unwrap_or_default();
                                    let mut text = format!("Total APIs found: {}\n", inner.total);
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
                                    result_text = text;
                                    let obj_val = json!({
                                        "total": inner.total,
                                        "apis": items
                                    });
                                    structured_content = Some(obj_val);
                                }
                            }
                            Err(e) => {
                                result_text = format!("gRPC Error: {}", e);
                                is_error = true;
                            }
                        }
                    }
                    Err(e) => {
                        result_text = format!("gRPC Error: {}", e);
                        is_error = true;
                    }
                }
            } else if name == "cowen_enable_api" {
                let target_tool_name = args.get("tool_name").and_then(|s| s.as_str()).unwrap_or("");

                let mut method = String::new();
                let mut path = String::new();
                let mut api_found = false;

                match get_grpc_client().await {
                    Ok(mut client) => {
                        let grpc_req = proto::ApiListRequest {
                            profile: app_state.profile.clone(),
                            search: None,
                            page: 1,
                            page_size: 1000,
                            refresh: false,
                        };
                        match client.api_list(inject_auth(grpc_req)).await {
                            Ok(resp) => {
                                let inner = resp.into_inner();
                                if let Some(err) = inner.error_message {
                                    result_text = format!("Error listing APIs: {}", err);
                                    is_error = true;
                                } else {
                                    let apis: Vec<serde_json::Value> =
                                        serde_json::from_str(&inner.json).unwrap_or_default();
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
                                    if !api_found {
                                        result_text = format!("API for tool_name '{}' not found", target_tool_name);
                                        is_error = true;
                                    }
                                }
                            }
                            Err(e) => {
                                result_text = format!("gRPC Error listing APIs: {}", e);
                                is_error = true;
                            }
                        }
                    }
                    Err(e) => {
                        result_text = format!("gRPC Error: {}", e);
                        is_error = true;
                    }
                }

                if api_found {
                    match get_grpc_client().await {
                        Ok(mut client) => {
                            let grpc_req = proto::ApiSpecRequest {
                                profile: app_state.profile.clone(),
                                method: method.clone(),
                                path: path.clone(),
                            };
                            let spec_json_str = match client.api_spec(inject_auth(grpc_req)).await {
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

                            let spec: serde_json::Value =
                                serde_json::from_str(&spec_json_str).unwrap_or(json!({}));

                            let summary = spec
                                .get("operation")
                                .map(|op| {
                                    let s = op.get("summary").and_then(|v| v.as_str()).unwrap_or("");
                                    let d =
                                        op.get("description").and_then(|v| v.as_str()).unwrap_or("");
                                    if !s.is_empty() && !d.is_empty() {
                                        format!("{} - {}", s, d)
                                    } else if !s.is_empty() {
                                        s.to_string()
                                    } else {
                                        d.to_string()
                                    }
                                })
                                .unwrap_or_default();

                            let (input_schema, output_schema, body_params) =
                                build_schema_from_openapi(&path, &spec);

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

                            list_changed = true;
                            let msg = format!(
                                "Successfully enabled tool '{}'. Tools list changed notification sent.",
                                target_tool_name
                            );
                            result_text = msg.clone();
                            structured_content = Some(json!({
                                "success": true,
                                "tool_name": target_tool_name,
                                "message": msg
                            }));
                        }
                        Err(e) => {
                            result_text = format!("gRPC Error: {}", e);
                            is_error = true;
                        }
                    }
                } else if !is_error {
                    result_text = format!("Failed to enable tool_name '{}'", target_tool_name);
                    is_error = true;
                }
            } else if name == "cowen_disable_api" {
                let target_tool_name = args.get("tool_name").and_then(|s| s.as_str()).unwrap_or("");

                let mut state = app_state.mcp_state.lock().await;
                let removed = state.tools.remove(target_tool_name);
                drop(state);

                if removed.is_some() {
                    list_changed = true;
                    let msg = format!("Successfully disabled tool '{}'.", target_tool_name);
                    let val = json!({
                        "success": true,
                        "tool_name": target_tool_name,
                        "message": msg
                    });
                    result_text = msg.clone();
                    structured_content = Some(val);
                } else {
                    let msg = format!("Tool '{}' was not enabled.", target_tool_name);
                    let val = json!({
                        "success": false,
                        "tool_name": target_tool_name,
                        "message": msg
                    });
                    result_text = msg.clone();
                    is_error = true;
                    structured_content = Some(val);
                }
            } else {
                let state = app_state.mcp_state.lock().await;
                if let Some(tool_def) = state.tools.get(name).cloned() {
                    drop(state);

                    let mut final_path = tool_def.path.clone();
                    let mut query_params = Vec::new();
                    let mut body_str = None;

                    let re = Regex::new(r"\{([a-zA-Z0-9_]+)\}").unwrap();
                    let path_vars: std::collections::HashSet<String> = re
                        .captures_iter(&tool_def.path)
                        .map(|cap| cap[1].to_string())
                        .collect();

                    let mut body_obj = serde_json::Map::new();

                    for (k, v) in &args {
                        if tool_def.body_params.contains(k) {
                            if k == "body_payload" {
                                // If fallback body_payload is used
                                body_str = serde_json::to_string(v).ok();
                            } else {
                                // If flattened
                                body_obj.insert(k.clone(), v.clone());
                            }
                        } else if path_vars.contains(k) {
                            if let Some(val_str) = v.as_str() {
                                final_path = final_path.replace(&format!("{{{}}}", k), val_str);
                            } else {
                                final_path =
                                    final_path.replace(&format!("{{{}}}", k), &v.to_string());
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
                        body_str = serde_json::to_string(&body_obj).ok();
                    }

                    if !query_params.is_empty() {
                        if final_path.contains('?') {
                            final_path = format!("{}&{}", final_path, query_params.join("&"));
                        } else {
                            final_path = format!("{}?{}", final_path, query_params.join("&"));
                        }
                    }

                    match get_grpc_client().await {
                        Ok(mut client) => {
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
                                        result_text = format!("Error: {}", err);
                                        is_error = true;
                                        structured_content = Some(json!({
                                            "error": err
                                        }));
                                    } else {
                                        result_text =
                                            format!("Status: {}\n{}", inner.status, inner.body);
                                        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&inner.body) {
                                            structured_content = Some(json_val);
                                        } else {
                                            structured_content = Some(json!({
                                                "status": inner.status,
                                                "body": inner.body
                                            }));
                                        }
                                    }
                                }
                                Err(e) => {
                                    result_text = format!("gRPC Error: {}", e);
                                    is_error = true;
                                    structured_content = Some(json!({
                                        "error": e.to_string()
                                    }));
                                }
                            }
                        }
                        Err(e) => {
                            result_text = format!("gRPC Error: {}", e);
                            is_error = true;
                            structured_content = Some(json!({
                                "error": e.to_string()
                            }));
                        }
                    }
                } else {
                    result_text = format!("Tool {} not found", name);
                    is_error = true;
                    structured_content = Some(json!({
                        "error": format!("Tool {} not found", name)
                    }));
                }
            }

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_resolve_refs() {
        let components = json!({
            "schemas": {
                "User": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "name": { "type": "string" }
                    }
                }
            }
        });

        let spec = json!({
            "operation": {
                "requestBody": {
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/User"
                            }
                        }
                    }
                }
            },
            "components": components
        });

        let (schema, _, _) = build_schema_from_openapi("/users", &spec);

        let props = schema.get("properties").unwrap();

        assert!(props.get("id").is_some());
        assert!(props.get("name").is_some());
    }

    #[test]
    fn test_resolve_refs_nested() {
        let components = json!({
            "schemas": {
                "Order": {
                    "type": "object",
                    "properties": {
                        "order_id": { "type": "string" },
                        "user": { "$ref": "#/components/schemas/User" }
                    }
                },
                "User": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" }
                    }
                }
            }
        });

        let spec = json!({
            "operation": {
                "requestBody": {
                    "content": {
                        "application/json": {
                            "schema": {
                                "$ref": "#/components/schemas/Order"
                            }
                        }
                    }
                }
            },
            "components": components
        });

        let (schema, _, _) = build_schema_from_openapi("/orders", &spec);

        let props = schema.get("properties").unwrap();

        let user_prop = props.get("user").unwrap();

        assert_eq!(user_prop.get("type").unwrap().as_str().unwrap(), "object");
        assert!(user_prop.get("properties").unwrap().get("name").is_some());
    }

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
        
        assert!(result.get("structuredContent").is_some());
        let structured = result.get("structuredContent").unwrap();
        assert!(structured.get("error").is_some());
    }

    #[test]
    fn test_build_schema_output_schema_translation() {
        let components = json!({
            "schemas": {
                "User": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" }
                    }
                }
            }
        });

        // 1. Test standard 200 response with application/json
        let spec_200 = json!({
            "operation": {
                "responses": {
                    "200": {
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/User"
                                }
                            }
                        }
                    }
                }
            },
            "components": components
        });
        let (_, out_schema_200, _) = build_schema_from_openapi("/test", &spec_200);
        assert!(out_schema_200.is_some());
        let schema_200 = out_schema_200.unwrap();
        assert_eq!(schema_200.get("type").unwrap().as_str().unwrap(), "object");
        assert!(schema_200.get("properties").unwrap().get("id").is_some());

        // 2. Test 201 status code with charset parameter in mime-type
        let spec_201_charset = json!({
            "operation": {
                "responses": {
                    "201": {
                        "content": {
                            "application/json; charset=utf-8": {
                                "schema": {
                                    "$ref": "#/components/schemas/User"
                                }
                            }
                        }
                    }
                }
            },
            "components": components
        });
        let (_, out_schema_201, _) = build_schema_from_openapi("/test", &spec_201_charset);
        assert!(out_schema_201.is_some());
        assert_eq!(out_schema_201.unwrap().get("type").unwrap().as_str().unwrap(), "object");

        // 3. Test fallback to first mime-type when json-like media type is absent
        let spec_fallback_mime = json!({
            "operation": {
                "responses": {
                    "200": {
                        "content": {
                            "text/plain": {
                                "schema": {
                                    "type": "string",
                                    "description": "Raw string response"
                                }
                            }
                        }
                    }
                }
            },
            "components": components
        });
        let (_, out_schema_fallback, _) = build_schema_from_openapi("/test", &spec_fallback_mime);
        assert!(out_schema_fallback.is_some());
        assert_eq!(out_schema_fallback.unwrap().get("type").unwrap().as_str().unwrap(), "string");
    }
}
