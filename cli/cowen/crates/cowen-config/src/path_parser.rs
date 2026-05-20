use serde_json::Value;
use cowen_common::{CowenResult, CowenError};

pub fn set_by_path(root: &mut Value, path: &str, value: &str) -> CowenResult<()> {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return Err(CowenError::Config(format!("Invalid path: {}", path)));
    }

    let mut current = root;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // Last part: set the value
            // Try to parse value as number, bool, or keep as string
            let json_val = if let Ok(n) = value.parse::<i64>() {
                Value::Number(n.into())
            } else if let Ok(b) = value.parse::<bool>() {
                Value::Bool(b)
            } else if value.starts_with('[') && value.ends_with(']') {
                // Simple support for empty array or basic strings
                serde_json::from_str(value).unwrap_or(Value::String(value.to_string()))
            } else {
                Value::String(value.to_string())
            };
            
            if let Some(obj) = current.as_object_mut() {
                obj.insert(part.to_string(), json_val);
            } else {
                return Err(CowenError::Config(format!("Path element '{}' is not an object", parts[..i].join("."))));
            }
        } else {
            // Intermediate part: navigate or create object
            if !current.is_object() {
                 *current = Value::Object(serde_json::Map::new());
            }
            current = current.as_object_mut().unwrap()
                .entry(part.to_string())
                .or_insert(Value::Object(serde_json::Map::new()));
        }
    }
    Ok(())
}

pub fn get_by_path(root: &Value, path: &str) -> Option<Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = root;
    for part in parts {
        current = current.get(part)?;
    }
    Some(current.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_set_nested() {
        let mut root = json!({});
        set_by_path(&mut root, "log.level", "debug").unwrap();
        assert_eq!(root["log"]["level"], "debug");

        set_by_path(&mut root, "log.max_files", "10").unwrap();
        assert_eq!(root["log"]["max_files"], 10);

        set_by_path(&mut root, "proxy_enabled", "false").unwrap();
        assert_eq!(root["proxy_enabled"], false);
    }

    #[test]
    fn test_get_nested() {
        let root = json!({
            "log": {
                "level": "info",
                "max_files": 7
            },
            "proxy_port": 16000
        });

        assert_eq!(get_by_path(&root, "log.level").unwrap(), "info");
        assert_eq!(get_by_path(&root, "log.max_files").unwrap(), 7);
        assert_eq!(get_by_path(&root, "proxy_port").unwrap(), 16000);
        assert!(get_by_path(&root, "non.existent").is_none());
    }
}
