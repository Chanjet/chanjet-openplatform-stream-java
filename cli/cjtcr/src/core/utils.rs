use serde::Serialize;
use anyhow::Result;

pub fn render<T: Serialize>(data: &T, format: &str) -> Result<()> {
    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(data)?);
        }
        "yaml" => {
            println!("{}", serde_yaml::to_string(data)?);
        }
        _ => {
            // Default to JSON for background processes if text is requested but not supported
            println!("{}", serde_json::to_string_pretty(data)?);
        }
    }
    Ok(())
}

pub fn mask_string(val: &str) -> String {
    if val.is_empty() {
        return "********".to_string();
    }
    if val.len() <= 12 {
        return "********".to_string();
    }
    format!("{}...{}", &val[..8], &val[val.len() - 4..])
}
