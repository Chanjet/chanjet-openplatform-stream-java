pub struct OpenApiParser;

impl OpenApiParser {
    pub fn parse_operations(spec: &serde_json::Value) -> Vec<serde_json::Value> {
        let mut ops = Vec::new();
        if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
            for (path, methods) in paths {
                if let Some(methods_obj) = methods.as_object() {
                    for (method, details) in methods_obj {
                        let summary = details.get("summary").and_then(|v| v.as_str()).unwrap_or("");
                        let description = details.get("description").and_then(|v| v.as_str()).unwrap_or("");
                        let mut tags_str = String::new();
                        if let Some(tags) = details.get("tags").and_then(|t| t.as_array()) {
                            let tag_list: Vec<&str> = tags.iter().filter_map(|t| t.as_str()).collect();
                            tags_str = tag_list.join(", ");
                        }
                        
                        let combined_desc = format!("{} {}", description, tags_str).trim().to_string();
                        
                        ops.push(serde_json::json!({
                            "id": format!("{} {}", method.to_uppercase(), path),
                            "method": method.to_uppercase(),
                            "path": path,
                            "summary": summary,
                            "description": combined_desc
                        }));
                    }
                }
            }
        }
        ops
    }
}
