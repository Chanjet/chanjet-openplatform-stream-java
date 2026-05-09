use anyhow::{Result, anyhow};

/// Validates a request against the provided OpenAPI spec.
pub fn validate_request(
    spec: &serde_json::Value,
    method: &str,
    path_with_query: &str,
    data: &Option<String>,
) -> Result<()> {
    let path_no_query = path_with_query.split('?').next().unwrap_or(path_with_query);
    
    // 1. Find the operation in the spec
    let matched_path = cowen_auth::client::find_matching_spec_path(path_no_query, spec)
        .ok_or_else(|| anyhow!("Path '{}' not found in OpenAPI spec", path_no_query))?;
    
    let op = spec["paths"][&matched_path].get(method.to_lowercase())
        .ok_or_else(|| anyhow!("Method '{}' not supported for path '{}'", method, matched_path))?;

    // 2. Validate Parameters (Query, Path, Header)
    if let Some(params) = op.get("parameters").and_then(|p| p.as_array()) {
        let query_pairs: std::collections::HashMap<String, String> = if let Some(q_idx) = path_with_query.find('?') {
            path_with_query[q_idx+1..].split('&')
                .filter_map(|s| {
                    let mut parts = s.splitn(2, '=');
                    Some((parts.next()?.to_string(), parts.next().unwrap_or("").to_string()))
                })
                .collect()
        } else {
            std::collections::HashMap::new()
        };

        for param in params {
            let name = param["name"].as_str().unwrap_or("");
            let location = param["in"].as_str().unwrap_or("");
            let required = param["required"].as_bool().unwrap_or(false);

            if required {
                match location {
                    "query" => {
                        if !query_pairs.contains_key(name) {
                            return Err(anyhow!("Missing required query parameter: '{}'", name));
                        }
                    }
                    "path" => {
                        // Path variables are usually already in the path if it matched find_matching_spec_path
                        // but we could add more specific regex validation here if needed.
                    }
                    _ => {} // Headers are handled by the Auth/RequestDecorator usually
                }
            }
        }
    }

    // 3. Validate Request Body
    if let Some(body_def) = op.get("requestBody") {
        let body_required = body_def["required"].as_bool().unwrap_or(false);
        if body_required && data.is_none() {
            return Err(anyhow!("Request body is required for {} {}", method.to_uppercase(), matched_path));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_request() {
        let spec = json!({
            "paths": {
                "/test": {
                    "post": {
                        "parameters": [
                            { "name": "q", "in": "query", "required": true }
                        ],
                        "requestBody": {
                            "required": true
                        }
                    }
                }
            }
        });

        // 1. Missing query parameter
        let res = validate_request(&spec, "POST", "/test", &Some("data".into()));
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "Missing required query parameter: 'q'");

        // 2. Missing request body
        let res = validate_request(&spec, "POST", "/test?q=1", &None);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "Request body is required for POST /test");

        // 3. Success
        let res = validate_request(&spec, "POST", "/test?q=1", &Some("data".into()));
        assert!(res.is_ok());
    }
}
