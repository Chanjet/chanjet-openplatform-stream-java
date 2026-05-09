use anyhow::{Result, anyhow};

pub fn find_matching_spec_path(req_path: &str, spec: &serde_json::Value) -> Option<String> {
    if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
        if paths.contains_key(req_path) {
            return Some(req_path.to_string());
        }
        let req_segments: Vec<&str> = req_path.split('/').filter(|s| !s.is_empty()).collect();
        for spec_path in paths.keys() {
            let spec_segments: Vec<&str> = spec_path.split('/').filter(|s| !s.is_empty()).collect();
            if req_segments.len() == spec_segments.len() {
                let mut match_ok = true;
                for (req_seg, spec_seg) in req_segments.iter().zip(spec_segments.iter()) {
                    if spec_seg.starts_with('{') && spec_seg.ends_with('}') {
                        continue;
                    }
                    if req_seg != spec_seg {
                        match_ok = false;
                        break;
                    }
                }
                if match_ok {
                    return Some(spec_path.clone());
                }
            }
        }
    }
    None
}

pub fn get_operation(spec: &serde_json::Value, path: &str, method: &str) -> Option<serde_json::Value> {
    if let Some(matched_path) = find_matching_spec_path(path, spec) {
        spec.get("paths")?
            .get(&matched_path)?
            .get(method.to_lowercase())
            .cloned()
    } else {
        None
    }
}

pub fn is_path_in_whitelist(req_path: &str, spec: &serde_json::Value) -> bool {
    find_matching_spec_path(req_path, spec).is_some()
}

/// Validates a request against the provided OpenAPI spec.
pub fn validate_request(
    spec: &serde_json::Value,
    method: &str,
    path_with_query: &str,
    data: &Option<String>,
) -> Result<()> {
    let path_no_query = path_with_query.split('?').next().unwrap_or(path_with_query);
    
    // 1. Find the operation in the spec
    let matched_path = find_matching_spec_path(path_no_query, spec)
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
                    }
                    _ => {}
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
