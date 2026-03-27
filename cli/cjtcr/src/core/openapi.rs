use serde_json::Value;

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
