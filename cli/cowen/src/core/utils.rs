use serde::Serialize;
use anyhow::Result;

pub fn get_bin_name() -> String {
    std::env::var("CARGO_BIN_NAME_OVERRIDE")
        .unwrap_or_else(|_| option_env!("CARGO_BIN_NAME_OVERRIDE").unwrap_or(env!("CARGO_PKG_NAME")).to_string())
}

pub fn render<T: Serialize>(data: &T, format: &str) -> Result<()> {
    let output = match format {
        "json" => {
            serde_json::to_string_pretty(data)?
        }
        "yaml" => {
            serde_yaml::to_string(data)?
        }
        _ => {
            serde_json::to_string_pretty(data)?
        }
    };
    
    // Apply masking to the final output string
    println!("{}", mask_sensitive_json(&output));
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
pub fn mask_tail(val: &str, show_len: usize) -> String {
    if val.len() <= show_len {
        return val.to_string();
    }
    let masked_len = val.len() - show_len;
    let mut result = "*".repeat(masked_len);
    result.push_str(&val[masked_len..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_tail() {
        assert_eq!(mask_tail("ABCDEFGH", 4), "****EFGH");
        assert_eq!(mask_tail("12345678", 4), "****5678");
        assert_eq!(mask_tail("123", 4), "123");
        assert_eq!(mask_tail("", 4), "");
    }
}
