pub fn validate_json_against_schema(
    value: &serde_json::Value,
    schema: &serde_json::Value,
) -> Result<(), String> {
    if let serde_json::Value::Bool(b) = schema {
        if *b {
            return Ok(());
        } else {
            return Err("Schema is false".to_string());
        }
    }

    let schema_obj = match schema.as_object() {
        Some(o) => o,
        None => return Ok(()),
    };

    if let Some(type_val) = schema_obj.get("type").and_then(|t| t.as_str()) {
        match type_val {
            "object" => validate_object(value, schema_obj)?,
            "array" => validate_array(value, schema_obj)?,
            "string" => validate_string(value)?,
            "number" => validate_number(value)?,
            "integer" => validate_integer(value)?,
            "boolean" => validate_boolean(value)?,
            "null" => validate_null(value)?,
            _ => {}
        }
    }

    Ok(())
}

fn validate_object(
    value: &serde_json::Value,
    schema_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    if !value.is_object() {
        return Err(format!("Expected object, found {}", get_type_name(value)));
    }
    let val_obj = value.as_object().unwrap();

    if let Some(required_arr) = schema_obj.get("required").and_then(|r| r.as_array()) {
        for req_field in required_arr {
            if let Some(field_name) = req_field.as_str() {
                if !val_obj.contains_key(field_name) {
                    return Err(format!("Missing required property '{}'", field_name));
                }
            }
        }
    }

    if let Some(properties_obj) = schema_obj.get("properties").and_then(|p| p.as_object()) {
        for (prop_name, prop_schema) in properties_obj {
            if let Some(prop_value) = val_obj.get(prop_name) {
                validate_json_against_schema(prop_value, prop_schema)
                    .map_err(|e| format!("Property '{}' failed validation: {}", prop_name, e))?;
            }
        }
    }
    Ok(())
}

fn validate_array(
    value: &serde_json::Value,
    schema_obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    if !value.is_array() {
        return Err(format!("Expected array, found {}", get_type_name(value)));
    }
    let val_arr = value.as_array().unwrap();

    if let Some(items_schema) = schema_obj.get("items") {
        for (idx, item_val) in val_arr.iter().enumerate() {
            validate_json_against_schema(item_val, items_schema)
                .map_err(|e| format!("Item at index {} failed validation: {}", idx, e))?;
        }
    }
    Ok(())
}

fn validate_string(value: &serde_json::Value) -> Result<(), String> {
    if !value.is_string() {
        return Err(format!("Expected string, found {}", get_type_name(value)));
    }
    Ok(())
}

fn validate_number(value: &serde_json::Value) -> Result<(), String> {
    if !value.is_number() {
        return Err(format!("Expected number, found {}", get_type_name(value)));
    }
    Ok(())
}

fn validate_integer(value: &serde_json::Value) -> Result<(), String> {
    if !value.is_i64() && !value.is_u64() {
        if let Some(f) = value.as_f64() {
            if f.fract() != 0.0 {
                return Err(format!("Expected integer, found fractional number {}", f));
            }
        } else {
            return Err(format!("Expected integer, found {}", get_type_name(value)));
        }
    }
    Ok(())
}

fn validate_boolean(value: &serde_json::Value) -> Result<(), String> {
    if !value.is_boolean() {
        return Err(format!("Expected boolean, found {}", get_type_name(value)));
    }
    Ok(())
}

fn validate_null(value: &serde_json::Value) -> Result<(), String> {
    if !value.is_null() {
        return Err(format!("Expected null, found {}", get_type_name(value)));
    }
    Ok(())
}

pub fn get_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_schema_validation() {
        let array_val = json!([1, 2, 3]);
        let array_schema = json!({
            "type": "array",
            "items": { "type": "integer" }
        });
        assert!(validate_json_against_schema(&array_val, &array_schema).is_ok());

        let obj_val = json!({
            "id": 123,
            "name": "test"
        });
        let obj_schema = json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": { "type": "integer" },
                "name": { "type": "string" }
            }
        });
        assert!(validate_json_against_schema(&obj_val, &obj_schema).is_ok());

        let obj_missing = json!({
            "name": "test"
        });
        assert!(validate_json_against_schema(&obj_missing, &obj_schema).is_err());
    }

    #[test]
    fn test_mcp_output_and_schema_matching_rule() {
        let schema1 = json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" }
            }
        });
        let val1 = json!({ "id": "123" });
        assert!(validate_json_against_schema(&val1, &schema1).is_ok());
        assert!(val1.is_object());

        let schema2 = json!({
            "type": "array",
            "items": { "type": "string" }
        });
        let val2 = json!(["123", "456"]);
        assert!(validate_json_against_schema(&val2, &schema2).is_ok());
        assert!(!val2.is_object());

        let val3 = json!({ "id": 123 });
        assert!(validate_json_against_schema(&val3, &schema1).is_err());
    }
}
