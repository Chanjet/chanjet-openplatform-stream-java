use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::collections::HashMap;
use tokio::sync::Mutex;
use std::sync::Arc;
use regex::Regex;

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
    let clean_path = path.replace("/", "_").replace("{", "").replace("}", "").replace("-", "_");
    let name = format!("{}_{}", method.to_lowercase(), clean_path);
    name.trim_matches('_').to_string()
}

fn resolve_refs(schema: &mut serde_json::Value, components: &serde_json::Value, depth: usize) {
    if depth > 10 { return; }
    
    let mut resolved_val = None;
    if let Some(obj) = schema.as_object() {
        if let Some(ref_val) = obj.get("$ref").and_then(|v| v.as_str()) {
            if ref_val.starts_with("#/components/") {
                let parts: Vec<&str> = ref_val.trim_start_matches("#/components/").split('/').collect();
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

fn build_schema_from_openapi(path: &str, spec: &serde_json::Value) -> (serde_json::Value, Vec<String>) {
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
                let is_req = param_obj.get("required").and_then(|r| r.as_bool()).unwrap_or(false);
                let param_in = param_obj.get("in").and_then(|i| i.as_str()).unwrap_or("");
                
                if param_in == "path" || param_in == "query" {
                    let mut prop_schema = param_obj.get("schema").cloned().unwrap_or(json!({ "type": "string" }));
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
            properties.insert(param.clone(), json!({
                "type": "string",
                "description": format!("Path parameter: {}", param)
            }));
            required.push(param);
        }
    }

    let mut body_params = Vec::new();

    if let Some(req_body) = operation.get("requestBody") {
        let mut body_obj = req_body.clone();
        resolve_refs(&mut body_obj, components, 0);
        
        let is_body_req = body_obj.get("required").and_then(|r| r.as_bool()).unwrap_or(false);
        let req_desc = body_obj.get("description").and_then(|d| d.as_str()).unwrap_or("");

        if let Some(schema) = body_obj
            .get("content")
            .and_then(|c| c.get("application/json"))
            .and_then(|j| j.get("schema"))
        {
            let mut body_schema = schema.clone();
            resolve_refs(&mut body_schema, components, 0);
            
            // If body is an object, flatten its properties into the root schema
            let is_object = body_schema.get("type").and_then(|t| t.as_str()) == Some("object");
            let has_properties = body_schema.get("properties").and_then(|p| p.as_object()).is_some();
            
            if is_object && has_properties {
                if let Some(body_props) = body_schema.get("properties").and_then(|p| p.as_object()) {
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
                let schema_desc = body_schema.get("description").and_then(|d| d.as_str()).unwrap_or("");
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
        schema.as_object_mut().unwrap().insert("required".to_string(), json!(required));
    }

    (schema, body_params)
}

async fn get_grpc_client() -> Result<CowenDaemonServiceClient<tonic::transport::Channel>, String> {
    let port_str = std::env::var("COWEN_IPC_PORT").map_err(|_| "Missing COWEN_IPC_PORT env var".to_string())?;
    let endpoint = format!("http://127.0.0.1:{}", port_str);
    CowenDaemonServiceClient::connect(endpoint).await.map_err(|e| e.to_string())
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

async fn handle_request(req: JsonRpcRequest, app_state: &AppState) -> (Option<JsonRpcResponse>, bool) {
    if req.jsonrpc != "2.0" {
        return (None, false);
    }

    let mut list_changed = false;

    let response = match req.method.as_str() {
        "initialize" => {
            Some(JsonRpcResponse {
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
            })
        }
        "notifications/initialized" => None,
        "tools/list" => {
            let mut tools = vec![
                json!({
                    "name": "cowen_api_list",
                    "description": "Lists all available Cowen APIs by searching keywords.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "search": { "type": "string", "description": "Optional search query" }
                        }
                    }
                }),
                json!({
                    "name": "cowen_enable_api",
                    "description": "Enable a specific Cowen API as a standalone MCP Tool so you can call it.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "method": { "type": "string" },
                            "path": { "type": "string" },
                            "description": { "type": "string", "description": "Brief description of what this API does" }
                        },
                        "required": ["method", "path"]
                    }
                }),
                json!({
                    "name": "cowen_disable_api",
                    "description": "Disable a previously enabled Cowen API Tool.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "method": { "type": "string" },
                            "path": { "type": "string" }
                        },
                        "required": ["method", "path"]
                    }
                }),
            ];

            let state = app_state.mcp_state.lock().await;
            for (tool_name, tool_def) in state.tools.iter() {
                tools.push(json!({
                    "name": tool_name,
                    "description": format!("Dynamic API: {} {}\n{}", tool_def.method, tool_def.path, tool_def.description),
                    "inputSchema": tool_def.input_schema
                }));
            }

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

            let mut result_content = String::new();

            if name == "cowen_api_list" {
                let search = args.get("search").and_then(|s| s.as_str()).map(|s| s.to_string());
                match get_grpc_client().await {
                    Ok(mut client) => {
                        let grpc_req = proto::ApiListRequest {
                            profile: app_state.profile.clone(),
                            search,
                            page: 1,
                            page_size: 1000,
                            refresh: false,
                        };
                        match client.api_list(inject_auth(grpc_req)).await {
                            Ok(resp) => {
                                let inner = resp.into_inner();
                                if let Some(err) = inner.error_message {
                                    result_content = format!("Error: {}", err);
                                } else {
                                    result_content = format!("Total APIs found: {}\n\n{}", inner.total, inner.json);
                                }
                            }
                            Err(e) => result_content = format!("gRPC Error: {}", e),
                        }
                    }
                    Err(e) => result_content = format!("gRPC Error: {}", e),
                }
            } else if name == "cowen_enable_api" {
                let method = args.get("method").and_then(|s| s.as_str()).unwrap_or("");
                let path = args.get("path").and_then(|s| s.as_str()).unwrap_or("");
                let description = args.get("description").and_then(|s| s.as_str()).unwrap_or("");
                
                match get_grpc_client().await {
                    Ok(mut client) => {
                        let grpc_req = proto::ApiSpecRequest {
                            profile: app_state.profile.clone(),
                            method: method.to_string(),
                            path: path.to_string(),
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
                        
                        let spec: serde_json::Value = serde_json::from_str(&spec_json_str).unwrap_or(json!({}));
                        
                        let tool_name = generate_tool_name(method, path);
                        let (input_schema, body_params) = build_schema_from_openapi(path, &spec);

                        let mut state = app_state.mcp_state.lock().await;
                        state.tools.insert(tool_name.clone(), EnabledTool {
                            method: method.to_string(),
                            path: path.to_string(),
                            description: description.to_string(),
                            input_schema,
                            body_params,
                        });
                        drop(state);
                        
                        list_changed = true;
                        result_content = format!("Successfully enabled tool '{}'. Tools list changed notification sent.", tool_name);
                    }
                    Err(e) => result_content = format!("gRPC Error: {}", e),
                }
            } else if name == "cowen_disable_api" {
                let method = args.get("method").and_then(|s| s.as_str()).unwrap_or("");
                let path = args.get("path").and_then(|s| s.as_str()).unwrap_or("");
                
                let tool_name = generate_tool_name(method, path);
                let mut state = app_state.mcp_state.lock().await;
                let removed = state.tools.remove(&tool_name);
                drop(state);

                if removed.is_some() {
                    list_changed = true;
                    result_content = format!("Successfully disabled tool '{}'.", tool_name);
                } else {
                    result_content = format!("Tool '{}' was not enabled.", tool_name);
                }
            } else {
                let state = app_state.mcp_state.lock().await;
                if let Some(tool_def) = state.tools.get(name).cloned() {
                    drop(state);
                    
                    let mut final_path = tool_def.path.clone();
                    let mut query_params = Vec::new();
                    let mut body_str = None;

                    let re = Regex::new(r"\{([a-zA-Z0-9_]+)\}").unwrap();
                    let path_vars: std::collections::HashSet<String> = re.captures_iter(&tool_def.path)
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
                                final_path = final_path.replace(&format!("{{{}}}", k), &v.to_string());
                            }
                        } else {
                            let val_str = v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string());
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
                                        result_content = format!("Error: {}", err);
                                    } else {
                                        result_content = format!("Status: {}\n{}", inner.status, inner.body);
                                    }
                                }
                                Err(e) => result_content = format!("gRPC Error: {}", e),
                            }
                        }
                        Err(e) => result_content = format!("gRPC Error: {}", e),
                    }
                } else {
                    result_content = format!("Tool {} not found", name);
                }
            }

            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(json!({
                    "content": [
                        {
                            "type": "text",
                            "text": result_content
                        }
                    ],
                    "isError": result_content.starts_with("Error") || result_content.starts_with("gRPC Error") || result_content.starts_with("Tool ") && result_content.ends_with("not found")
                })),
                error: None,
            })
        }
        _ => {
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: "Method not found".to_string()
                })
            })
        }
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

        let (schema, _) = build_schema_from_openapi("/users", &spec);
        
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

        let (schema, _) = build_schema_from_openapi("/orders", &spec);
        
        let props = schema.get("properties").unwrap();

        let user_prop = props.get("user").unwrap();
            
        assert_eq!(user_prop.get("type").unwrap().as_str().unwrap(), "object");
        assert!(user_prop.get("properties").unwrap().get("name").is_some());
    }
}
