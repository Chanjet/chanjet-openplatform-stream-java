use serde_json::Value;

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
    let matched_path = crate::auth::client::find_matching_spec_path(path_no_query, spec)
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

/// Flattens an OpenAPI 3.x specification by resolving all local $ref pointers.
/// Supported pointers must start with "#/components/".
pub fn flatten(spec: &mut Value) {
    if let Some(components) = spec.get("components").cloned() {
        resolve_recursive(spec, &components, 0);
        // Remove components after they are embedded to keep the output clean and flat
        if let Some(obj) = spec.as_object_mut() {
            obj.remove("components");
        }
    }
}

fn resolve_recursive(node: &mut Value, components: &Value, depth: u32) {
    // Prevent infinite recursion on circular references
    if depth > 20 { return; }

    if let Some(obj) = node.as_object_mut() {
        // Check if this node is a reference
        if let Some(ref_val) = obj.remove("$ref") {
            if let Some(ref_str) = ref_val.as_str() {
                if let Some(resolved) = resolve_ptr(ref_str, components) {
                    *node = resolved;
                    // Keep resolving in case the resolved content itself contains $refs
                    resolve_recursive(node, components, depth + 1);
                    return;
                }
            }
        }

        // Recursively process all properties
        for value in obj.values_mut() {
            resolve_recursive(value, components, depth);
        }
    } else if let Some(arr) = node.as_array_mut() {
        // Recursively process all array items
        for value in arr.iter_mut() {
            resolve_recursive(value, components, depth);
        }
    }
}

fn resolve_ptr(ptr: &str, components: &Value) -> Option<Value> {
    const PREFIX: &str = "#/components/";
    if !ptr.starts_with(PREFIX) {
        return None;
    }

    // Convert #/components/schemas/Name to /schemas/Name for use with Value::pointer()
    let json_ptr = &ptr[PREFIX.len() - 1..]; // Result: /schemas/Name or /responses/Name
    components.pointer(json_ptr).cloned()
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

    #[test]
    fn test_flatten_simple() {
        let mut spec = json!({
            "paths": {
                "/test": {
                    "get": {
                        "responses": {
                            "200": { "schema": { "$ref": "#/components/schemas/User" } }
                        }
                    }
                }
            },
            "components": {
                "schemas": {
                    "User": { "type": "object", "properties": { "name": { "type": "string" } } }
                }
            }
        });

        flatten(&mut spec);

        let user_schema = spec.pointer("/paths/~1test/get/responses/200/schema").unwrap();
        assert_eq!(user_schema["type"], "object");
        assert_eq!(user_schema["properties"]["name"]["type"], "string");
        assert!(spec.get("components").is_none());
    }
}
