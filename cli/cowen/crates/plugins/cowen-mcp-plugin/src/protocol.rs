use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnabledTool {
    pub method: String,
    pub path: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: Option<serde_json::Value>,
    pub body_params: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpState {
    pub tools: HashMap<String, EnabledTool>,
    pub protocol_version: Option<String>,
}

impl McpState {
    pub fn supports_feature(&self, feature: &crate::capabilities::McpFeature) -> bool {
        crate::capabilities::get_global_registry().supports(feature, self.protocol_version.as_deref())
    }
}

pub struct AppState {
    pub profile: String,
    pub mcp_state: Arc<Mutex<McpState>>,
}

impl AppState {
    pub fn new(profile: String) -> Self {
        Self {
            profile,
            mcp_state: Arc::new(Mutex::new(McpState::default())),
        }
    }
}

pub fn generate_tool_name(method: &str, path: &str) -> String {
    let clean_path = path
        .replace("/", "_")
        .replace("{", "")
        .replace("}", "")
        .replace("-", "_");
    let name = format!("{}_{}", method.to_lowercase(), clean_path);
    name.trim_matches('_').to_string()
}
