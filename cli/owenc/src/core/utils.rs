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

pub fn mask_sensitive_json(input: &str) -> String {
    use regex::Regex;
    let mut output = input.to_string();
    
    // Pattern for JSON keys
    let patterns = [
        r#"(?i)("accessToken"\s*:\s*")([^"]+)(")"#,
        r#"(?i)("access_token"\s*:\s*")([^"]+)(")"#,
        r#"(?i)("appSecret"\s*:\s*")([^"]+)(")"#,
        r#"(?i)("app_secret"\s*:\s*")([^"]+)(")"#,
        r#"(?i)("certificate"\s*:\s*")([^"]+)(")"#,
        r#"(?i)("appTicket"\s*:\s*")([^"]+)(")"#,
        r#"(?i)("app_ticket"\s*:\s*")([^"]+)(")"#,
        r#"(?i)("encryptKey"\s*:\s*")([^"]+)(")"#,
        r#"(?i)("encrypt_key"\s*:\s*")([^"]+)(")"#,
    ];

    for p in patterns {
        if let Ok(re) = Regex::new(p) {
            output = re.replace_all(&output, |caps: &regex::Captures| {
                let secret = &caps[2];
                format!("{}{}{}", &caps[1], mask_string(secret), &caps[3])
            }).to_string();
        }
    }
    output
}
