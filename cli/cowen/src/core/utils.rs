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
    
    // Pattern for JSON keys (obfuscated to prevent binary string extraction)
    let patterns = [
        obfs!(r#"(?i)("accessToken"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("access_token"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("appSecret"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("app_secret"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("certificate"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("appTicket"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("app_ticket"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("encryptKey"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("encrypt_key"\s*:\s*")([^"]+)(")"#),
    ];

    for p in &patterns {
        if let Ok(re) = Regex::new(p) {
            output = re.replace_all(&output, |caps: &regex::Captures| {
                let secret = &caps[2];
                format!("{}{}{}", &caps[1], mask_string(secret), &caps[3])
            }).to_string();
        }
    }
    output
}

pub fn mask_url_query(url: &str) -> String {
    use regex::Regex;
    let mut output = url.to_string();
    
    // Obfuscated keys to prevent static string scanning in binary
    let patterns = [
        obfs!(r#"(?i)([?&](accessToken|access_token|token|openToken|appSecret|appTicket|encryptKey)=)([^&]+)"#),
    ];

    for p in &patterns {
        if let Ok(re) = Regex::new(p) {
            output = re.replace_all(&output, |caps: &regex::Captures| {
                let secret = &caps[3];
                format!("{}{}", &caps[1], mask_string(secret))
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

    #[test]
    fn test_mask_string() {
        // Empty
        assert_eq!(mask_string(""), "********");
        // Less than or equal to 12 chars
        assert_eq!(mask_string("123456789012"), "********");
        assert_eq!(mask_string("short"), "********");
        // More than 12 chars: first 8 ... last 4
        assert_eq!(mask_string("1234567890123"), "12345678...0123");
        assert_eq!(mask_string("ABCDEFGHIJKLMNOP"), "ABCDEFGH...MNOP");
    }

    #[test]
    fn test_mask_sensitive_json() {
        let input_json = r#"{
            "accessToken": "very_secret_token_123456789",
            "normalField": "visible_data",
            "appSecret": "another_secret_987654321",
            "certificate": "cert_1234_long_string"
        }"#;

        let output_json = mask_sensitive_json(input_json);

        assert!(output_json.contains("\"normalField\": \"visible_data\""));
        assert!(!output_json.contains("very_secret_token_123456789"));
        assert!(output_json.contains("very_sec...6789"));
        assert!(!output_json.contains("another_secret_987654321"));
        assert!(output_json.contains("another_...4321"));
        
        let mask = mask_string("cert_1234_long_string");
        assert!(output_json.contains(&mask));
        assert!(!output_json.contains("cert_1234_long_string"));
    }
}
