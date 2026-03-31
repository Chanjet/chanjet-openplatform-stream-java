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
        if let Some(operation) = crate::auth::client::get_operation(spec, path, method) {
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
                                // Some APIs use openToken directly as a header
                                headers.push(("openToken".to_string(), token_value.to_string()));
                            }
                            "Authorization" => {
                                // If the spec explicitly asks for Authorization, we provide it
                                // (Note: Usually this is "Bearer {token}")
                                headers.push(("Authorization".to_string(), format!("Bearer {}", token_value)));
                            }
                            _ => {}
                        }
                    }
                }
            }
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
    
    #[test]
    fn test_get_auth_headers_authorization() {
        let spec = json!({
            "paths": {
                "/v1/auth": {
                    "post": {
                        "parameters": [
                            { "name": "Authorization", "in": "header" }
                        ]
                    }
                }
            }
        });

        let headers = RequestDecorator::get_auth_headers(
            &spec,
            "/v1/auth",
            "post",
            "key",
            "sec",
            "token123"
        );

        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], ("Authorization".to_string(), "Bearer token123".to_string()));
    }
}
