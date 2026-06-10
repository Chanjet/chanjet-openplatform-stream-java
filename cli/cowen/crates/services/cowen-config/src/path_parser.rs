use cowen_common::{CowenError, CowenResult};
use serde_json::Value;

fn apply_leaf_segment(current: &mut Value, part: &str, value: &str) -> CowenResult<()> {
    let json_val = parse_value(value);
    if part == "+" {
        let arr = current.as_array_mut().ok_or_else(|| {
            CowenError::Config("Target is not an array for append".into())
        })?;
        arr.push(json_val);
    } else if part.contains(':') {
        return Err(CowenError::Config(
            "Locator cannot be the last segment of a path for set".into(),
        ));
    } else if let Ok(idx) = part.parse::<usize>() {
        let arr = current
            .as_array_mut()
            .ok_or_else(|| CowenError::Config("Target is not an array for index".into()))?;
        if idx >= arr.len() {
            return Err(CowenError::Config(format!(
                "Index {} out of bounds (length: {})",
                idx, arr.len()
            )));
        }
        arr[idx] = json_val;
    } else {
        let obj = current
            .as_object_mut()
            .ok_or_else(|| CowenError::Config("Target is not an object".into()))?;
        obj.insert(part.to_string(), json_val);
    }
    Ok(())
}

pub fn set_by_path(root: &mut Value, path: &str, value: &str) -> CowenResult<()> {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return Err(CowenError::Config(format!("Invalid path: {}", path)));
    }

    let mut current = root;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            apply_leaf_segment(current, part, value)?;
        } else {
            current = resolve_or_create_segment(current, part, parts.get(i + 1).cloned())?;
        }
    }
    Ok(())
}

pub fn get_by_path(root: &Value, path: &str) -> Option<Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = root;
    for part in parts {
        current = resolve_segment_immutable(current, part)?;
    }
    Some(current.clone())
}

fn remove_leaf_segment(current: &mut Value, part: &str) -> CowenResult<()> {
    if part.contains(':') {
        let (idx, arr) = get_locator_index_mut(current, part)?;
        arr.remove(idx);
    } else if let Ok(idx) = part.parse::<usize>() {
        let arr = current
            .as_array_mut()
            .ok_or_else(|| CowenError::Config("Not an array".into()))?;
        if idx >= arr.len() {
            return Err(CowenError::Config("Index out of bounds".into()));
        }
        arr.remove(idx);
    } else {
        let obj = current
            .as_object_mut()
            .ok_or_else(|| CowenError::Config("Not an object".into()))?;
        obj.remove(part);
    }
    Ok(())
}

pub fn unset_by_path(root: &mut Value, path: &str) -> CowenResult<()> {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return Ok(());
    }

    let mut current = root;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            remove_leaf_segment(current, part)?;
        } else {
            current = resolve_segment_mutable(current, part)?;
        }
    }
    Ok(())
}

fn parse_value(value: &str) -> Value {
    if let Ok(n) = value.parse::<i64>() {
        Value::Number(n.into())
    } else if let Ok(b) = value.parse::<bool>() {
        Value::Bool(b)
    } else if (value.starts_with('[') && value.ends_with(']'))
        || (value.starts_with('{') && value.ends_with('}'))
    {
        serde_json::from_str(value).unwrap_or(Value::String(value.to_string()))
    } else {
        Value::String(value.to_string())
    }
}

fn parse_locator(segment: &str) -> CowenResult<(&str, &str)> {
    let parts: Vec<&str> = segment.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(CowenError::Config(format!(
            "Invalid locator format: {}",
            segment
        )));
    }
    Ok((parts[0], parts[1]))
}

fn find_locator_index(arr: &[Value], key: &str, val: &str) -> Option<usize> {
    arr.iter()
        .position(|item| item.get(key).and_then(|v| v.as_str()) == Some(val))
}

fn resolve_segment_immutable<'a>(current: &'a Value, segment: &str) -> Option<&'a Value> {
    if segment.contains(':') {
        let (key, val) = parse_locator(segment).ok()?;
        let arr = current.as_array()?;
        let idx = find_locator_index(arr, key, val)?;
        Some(&arr[idx])
    } else if let Ok(idx) = segment.parse::<usize>() {
        current.as_array()?.get(idx)
    } else {
        current.as_object()?.get(segment)
    }
}


fn get_array_index_mut(current: &mut Value, idx: usize) -> CowenResult<&mut Value> {
    let arr = current
        .as_array_mut()
        .ok_or_else(|| CowenError::Config("Not an array".into()))?;
    arr.get_mut(idx)
        .ok_or_else(|| CowenError::Config(format!("Index {} out of bounds", idx)))
}

fn get_locator_index_mut<'a>(current: &'a mut Value, segment: &str) -> CowenResult<(usize, &'a mut Vec<Value>)> {
    let (key, val) = parse_locator(segment)?;
    let arr = current
        .as_array_mut()
        .ok_or_else(|| CowenError::Config("Not an array".into()))?;
    let idx = find_locator_index(arr, key, val)
        .ok_or_else(|| CowenError::Config(format!("Locator {} not found", segment)))?;
    Ok((idx, arr))
}

fn resolve_segment_mutable<'a>(
    current: &'a mut Value,
    segment: &str,
) -> CowenResult<&'a mut Value> {
    if segment.contains(':') {
        let (idx, arr) = get_locator_index_mut(current, segment)?;
        Ok(&mut arr[idx])
    } else if let Ok(idx) = segment.parse::<usize>() {
        get_array_index_mut(current, idx)
    } else {
        current
            .as_object_mut()
            .ok_or_else(|| CowenError::Config("Not an object".into()))?
            .get_mut(segment)
            .ok_or_else(|| CowenError::Config(format!("Field {} not found", segment)))
    }
}

fn resolve_or_create_segment<'a>(
    current: &'a mut Value,
    segment: &str,
    next_segment: Option<&str>,
) -> CowenResult<&'a mut Value> {
    if segment == "+" {
        let arr = current
            .as_array_mut()
            .ok_or_else(|| CowenError::Config("Not an array for append".into()))?;
        arr.push(serde_json::json!({}));
        Ok(arr.last_mut().unwrap())
    } else if segment.contains(':') {
        resolve_segment_mutable(current, segment)
    } else if let Ok(idx) = segment.parse::<usize>() {
        get_array_index_mut(current, idx)
    } else {
        if !current.is_object() {
            *current = Value::Object(serde_json::Map::new());
        }
        let obj = current.as_object_mut().unwrap();
        if !obj.contains_key(segment) {
            let next_is_array = next_segment
                .map(|s| s == "+" || s.parse::<usize>().is_ok() || s.contains(':'))
                .unwrap_or(false);
            if next_is_array {
                obj.insert(segment.to_string(), Value::Array(vec![]));
            } else {
                obj.insert(segment.to_string(), Value::Object(serde_json::Map::new()));
            }
        }
        Ok(obj.get_mut(segment).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_set_with_index() {
        let mut root = json!({"items": ["a", "b"]});
        set_by_path(&mut root, "items.0", "new_a").unwrap();
        assert_eq!(root["items"][0], "new_a");

        let res = set_by_path(&mut root, "items.5", "fail");
        assert!(res.is_err());
    }

    #[test]
    fn test_set_with_append() {
        let mut root = json!({"items": ["a"]});
        set_by_path(&mut root, "items.+", "b").unwrap();
        assert_eq!(root["items"].as_array().unwrap().len(), 2);
        assert_eq!(root["items"][1], "b");
    }

    #[test]
    fn test_set_with_locator() {
        let mut root = json!({
            "plugins": [
                {"name": "p1", "enabled": true},
                {"name": "p2", "enabled": false}
            ]
        });

        set_by_path(&mut root, "plugins.name:p2.enabled", "true").unwrap();
        assert_eq!(root["plugins"][1]["enabled"], true);

        // Immediate binding check: rename p1 to p3
        set_by_path(&mut root, "plugins.name:p1.name", "p3").unwrap();
        assert_eq!(root["plugins"][0]["name"], "p3");

        // Old locator should fail
        assert!(get_by_path(&root, "plugins.name:p1").is_none());
        assert!(get_by_path(&root, "plugins.name:p3").is_some());
    }

    #[test]
    fn test_unset_with_collapsing() {
        let mut root = json!({"items": ["a", "b", "c"]});
        unset_by_path(&mut root, "items.1").unwrap();
        assert_eq!(root["items"].as_array().unwrap().len(), 2);
        assert_eq!(root["items"][0], "a");
        assert_eq!(root["items"][1], "c"); // b is removed, c moved left
    }

    #[test]
    fn test_unset_with_locator() {
        let mut root = json!({
            "plugins": [
                {"name": "p1"},
                {"name": "p2"}
            ]
        });
        unset_by_path(&mut root, "plugins.name:p1").unwrap();
        assert_eq!(root["plugins"].as_array().unwrap().len(), 1);
        assert_eq!(root["plugins"][0]["name"], "p2");
    }
}
