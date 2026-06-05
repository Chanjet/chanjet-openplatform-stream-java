use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

impl JsonRpcResponse {
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<serde_json::Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message }),
        }
    }
}

/// Generic SPI trait representing standard behaviors of a Sidecar search / embedding plugin.
pub trait SidecarPluginSpi {
    type Document;
    type QueryResult;

    fn update_index(&self, tenant_id: &str, documents: Vec<Self::Document>) -> Result<(), String>;
    fn query(&self, tenant_id: &str, query: &str, top: usize) -> Result<Vec<Self::QueryResult>, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonrpc_request_parsing() {
        let json_data = r#"{"jsonrpc":"2.0","id":1,"method":"query","params":{"tenant_id":"test_tenant","query":"hello","top":5}}"#;
        let parsed: JsonRpcRequest = serde_json::from_str(json_data).unwrap();
        
        // Assert correctly (Green phase of TDD)
        assert_eq!(parsed.jsonrpc, "2.0");
        assert_eq!(parsed.method, "query");
    }

    #[test]
    fn test_jsonrpc_response_serialization() {
        let resp = JsonRpcResponse::success(Some(serde_json::json!(1)), serde_json::json!({"status": "ok"}));
        let serialized = serde_json::to_string(&resp).unwrap();
        assert!(serialized.contains(r#""jsonrpc":"2.0""#));
        assert!(serialized.contains(r#""result":{"status":"ok"}"#));
    }
}
