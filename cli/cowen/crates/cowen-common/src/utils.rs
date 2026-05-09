use crate::{CowenResult, CowenError};
use crate::obfs;
use serde::Serialize;

pub fn get_bin_name() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(name) = exe.file_name() {
            return name.to_string_lossy().to_string();
        }
    }
    "cowen".to_string()
}

pub fn render<T: Serialize>(data: &T, format: &str) -> CowenResult<()> {
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
    
    if std::env::var("COWEN_RAW_OUTPUT").unwrap_or_default() == "true" {
        println!("{}", output);
    } else {
        println!("{}", mask_sensitive_json(&output));
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
    
    let patterns = [
        obfs!(r#"(?i)("accessToken"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("access_token"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("orgAccessToken"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("userAccessToken"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("refreshToken"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("refresh_token"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("appSecret"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("app_secret"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("certificate"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("appTicket"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("app_ticket"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("encryptKey"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("encrypt_key"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("permanentAuthCode"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("userPermanentCode"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("user_auth_permanent_code"\s*:\s*")([^"]+)(")"#),
        obfs!(r#"(?i)("tempAuthCode"\s*:\s*")([^"]+)(")"#),
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
    
    let patterns = [
        obfs!(r#"(?i)([?&](accessToken|access_token|orgAccessToken|userAccessToken|refreshToken|refresh_token|token|openToken|appSecret|app_secret|appTicket|app_ticket|encryptKey|encrypt_key|permanentAuthCode|userPermanentCode|tempAuthCode)=)([^&]+)"#),
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
#[allow(dead_code)]
pub fn mask_tail(val: &str, show_len: usize) -> String {
    if val.len() <= show_len {
        return val.to_string();
    }
    let masked_len = val.len() - show_len;
    let mut result = "*".repeat(masked_len);
    result.push_str(&val[masked_len..]);
    result
}
