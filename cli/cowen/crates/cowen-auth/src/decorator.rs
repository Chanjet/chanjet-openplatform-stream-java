use serde_json::Value;

/// Shared utility for spec-driven request decoration.
/// This consolidates the logic for injecting appKey, appSecret, and openToken
/// based on the OpenAPI specification's requirements.
pub struct RequestDecorator;

impl RequestDecorator {
    /// Returns a list of headers (name, value) that should be injected based on the spec.
    pub fn get_auth_headers(
        spec: &Value,
        path: &str,
        method: &str,
        app_key: &str,
        app_secret: &str,
        token_value: &str,
    ) -> Vec<(String, String)> {
        let mut headers = Vec::new();

        // 1. Resolve operation from spec
        if let Some(operation) = crate::client::get_operation(spec, path, method) {
            // 2. Scan parameters for headers
            if let Some(params) = operation.get("parameters").and_then(|p| p.as_array()) {
                for param in params.iter() {
                    let name = param.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let p_in = param.get("in").and_then(|i| i.as_str()).unwrap_or("");
                    
                    if p_in == "header" {
                        match name {
                            "appKey" => {
                                headers.push(("appKey".to_string(), app_key.to_string()));
                            }
                            "appSecret" => {
                                headers.push(("appSecret".to_string(), app_secret.to_string()));
                            }
                            "openToken" => {
                                headers.push(("openToken".to_string(), token_value.to_string()));
                            }
                            _ => {}
                        }
                    }
                }
            }
        } else {
            // Fallback: spec unavailable or path not defined — inject default auth headers
            headers.push(("appKey".to_string(), app_key.to_string()));
            headers.push(("openToken".to_string(), token_value.to_string()));
        }

        headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_get_auth_headers_all() {
        let spec = json!({
            "paths": {
                "/v1/test": {
                    "get": {
                        "parameters": [
                            { "name": "appKey", "in": "header" },
                            { "name": "appSecret", "in": "header" },
                            { "name": "openToken", "in": "header" }
                        ]
                    }
                }
            }
        });

        let headers = RequestDecorator::get_auth_headers(
            &spec,
            "/v1/test",
            "get",
            "key123",
            "sec123",
            "tok123"
        );

        assert_eq!(headers.len(), 3);
        assert!(headers.contains(&("appKey".to_string(), "key123".to_string())));
        assert!(headers.contains(&("appSecret".to_string(), "sec123".to_string())));
        assert!(headers.contains(&("openToken".to_string(), "tok123".to_string())));
    }

    #[test]
    fn test_get_auth_headers_none() {
        let spec = json!({
            "paths": {
                "/v1/test": {
                    "get": {
                        "parameters": []
                    }
                }
            }
        });

        let headers = RequestDecorator::get_auth_headers(
            &spec,
            "/v1/test",
            "get",
            "key123",
            "sec123",
            "tok123"
        );

        assert_eq!(headers.len(), 0);
    }
}
