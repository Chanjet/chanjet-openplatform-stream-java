use regex::Regex;
use serde_json::json;

pub fn resolve_refs(schema: &mut serde_json::Value, components: &serde_json::Value, depth: usize) {
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

pub fn build_schema_from_openapi(
    path: &str,
    spec: &serde_json::Value,
) -> (serde_json::Value, Option<serde_json::Value>, Vec<String>) {
    let operation = spec.get("operation").unwrap_or(spec);
    let empty_components = json!({});
    let components = spec.get("components").unwrap_or(&empty_components);

    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    extract_path_query_parameters(operation, components, &mut properties, &mut required);
    extract_missing_path_parameters(path, &mut properties, &mut required);
    let body_params = extract_request_body(operation, components, &mut properties, &mut required);

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

    let output_schema = extract_response_schema(operation, components);

    (schema, output_schema, body_params)
}

fn extract_path_query_parameters(
    operation: &serde_json::Value,
    components: &serde_json::Value,
    properties: &mut serde_json::Map<String, serde_json::Value>,
    required: &mut Vec<String>,
) {
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
}

fn extract_missing_path_parameters(
    path: &str,
    properties: &mut serde_json::Map<String, serde_json::Value>,
    required: &mut Vec<String>,
) {
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
}

fn process_object_body(
    body_schema: &serde_json::Value,
    is_body_req: bool,
    properties: &mut serde_json::Map<String, serde_json::Value>,
    required: &mut Vec<String>,
    body_params: &mut Vec<String>,
) {
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
}

fn process_scalar_body(
    mut body_schema: serde_json::Value,
    is_body_req: bool,
    req_desc: &str,
    properties: &mut serde_json::Map<String, serde_json::Value>,
    required: &mut Vec<String>,
    body_params: &mut Vec<String>,
) {
    let schema_desc = body_schema
        .get("description")
        .and_then(|d| d.as_str())
        .unwrap_or("");
    let mut final_desc = String::from("JSON payload for the request body. ");
    if !req_desc.is_empty() {
        final_desc.push_str(req_desc);
        final_desc.push(' ');
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

fn extract_request_body(
    operation: &serde_json::Value,
    components: &serde_json::Value,
    properties: &mut serde_json::Map<String, serde_json::Value>,
    required: &mut Vec<String>,
) -> Vec<String> {
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

            let is_object = body_schema.get("type").and_then(|t| t.as_str()) == Some("object");
            let has_properties = body_schema
                .get("properties")
                .and_then(|p| p.as_object())
                .is_some();

            if is_object && has_properties {
                process_object_body(
                    &body_schema,
                    is_body_req,
                    properties,
                    required,
                    &mut body_params,
                );
            } else {
                process_scalar_body(
                    body_schema,
                    is_body_req,
                    req_desc,
                    properties,
                    required,
                    &mut body_params,
                );
            }
        }
    }

    body_params
}

fn get_ok_response(operation: &serde_json::Value) -> Option<serde_json::Value> {
    if let Some(responses) = operation.get("responses").and_then(|r| r.as_object()) {
        for key in &["200", "201", "202", "204", "default"] {
            if let Some(resp) = responses.get(*key) {
                return Some(resp.clone());
            }
        }
        for (k, v) in responses {
            if k.starts_with('2') || k == "default" {
                return Some(v.clone());
            }
        }
    }
    None
}

fn find_json_schema(
    resp_obj: &serde_json::Value,
) -> Option<(serde_json::Value, serde_json::Value)> {
    if let Some(content) = resp_obj.get("content").and_then(|c| c.as_object()) {
        for (mime, media_type) in content {
            if mime.starts_with("application/json") || mime.contains("json") {
                if let Some(s) = media_type.get("schema") {
                    return Some((s.clone(), media_type.clone()));
                }
            }
        }
        for (_, media_type) in content {
            if let Some(s) = media_type.get("schema") {
                return Some((s.clone(), media_type.clone()));
            }
        }
    }
    None
}

fn ensure_array_schema(
    mut schema: serde_json::Value,
    media_type: &serde_json::Value,
) -> serde_json::Value {
    let has_array_type = media_type
        .get("type")
        .and_then(|t| t.as_str())
        .map(|t| t == "array")
        .unwrap_or(false)
        || media_type
            .get("types")
            .and_then(|ts| ts.as_array())
            .map(|arr| arr.iter().any(|val| val.as_str() == Some("array")))
            .unwrap_or(false);

    if has_array_type {
        let schema_type = schema.get("type").and_then(|t| t.as_str());
        if schema_type != Some("array") {
            schema = serde_json::json!({
                "type": "array",
                "items": schema
            });
        }
    }
    schema
}

fn extract_response_schema(
    operation: &serde_json::Value,
    components: &serde_json::Value,
) -> Option<serde_json::Value> {
    if let Some(mut resp_obj) = get_ok_response(operation) {
        resolve_refs(&mut resp_obj, components, 0);

        if let Some((mut schema, media_type)) = find_json_schema(&resp_obj) {
            resolve_refs(&mut schema, components, 0);
            return Some(ensure_array_schema(schema, &media_type));
        }
    }
    None
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
        assert_eq!(
            out_schema_201
                .unwrap()
                .get("type")
                .unwrap()
                .as_str()
                .unwrap(),
            "object"
        );

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
        assert_eq!(
            out_schema_fallback
                .unwrap()
                .get("type")
                .unwrap()
                .as_str()
                .unwrap(),
            "string"
        );
    }

    #[test]
    fn test_build_schema_non_standard_array_response() {
        let spec = json!({
            "operation": {
                "responses": {
                    "200": {
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "id": { "type": "string" }
                                    }
                                },
                                "type": "array"
                            }
                        }
                    }
                }
            }
        });
        let (_, out_schema, _) = build_schema_from_openapi("/test", &spec);
        assert!(out_schema.is_some());
        let schema = out_schema.unwrap();
        assert_eq!(schema.get("type").unwrap().as_str().unwrap(), "array");
        let items = schema.get("items").unwrap();
        assert_eq!(items.get("type").unwrap().as_str().unwrap(), "object");
        assert!(items.get("properties").unwrap().get("id").is_some());
    }
}
