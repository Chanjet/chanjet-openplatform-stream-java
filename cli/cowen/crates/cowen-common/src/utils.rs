use crate::CowenResult;
pub use cowen_infra::{
    get_bin_name, mask_string, mask_sensitive_json, mask_url_query, mask_tail, mask_url, set_process_name, obfs
};
use serde::Serialize;

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

pub fn sanitize_credential(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            c != '\u{200b}' && c != '\u{200c}' && c != '\u{200d}' && c != '\u{feff}' 
            && !c.is_control()
        })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_url() {
        assert_eq!(mask_url("redis://:password@localhost:6379"), "redis://:***@localhost:6379");
        assert_eq!(mask_url("mysql://user:pass@127.0.0.1:3306/db"), "mysql://user:***@127.0.0.1:3306/db");
        assert_eq!(mask_url("postgres://admin@localhost/mydb"), "postgres://***@localhost/mydb");
        assert_eq!(mask_url("https://openapi.chanjet.com"), "https://openapi.chanjet.com");
    }

    #[test]
    fn test_set_process_name() {
        set_process_name("cowen-test-process");
    }

    #[test]
    fn test_sanitize_credential() {
        // Contains standard spaces, tabs, newlines, zero-width space (\u{200b}), and control character (\u{0007} - Bell)
        let dirty = "\n\t \u{200b}1234567890123456\u{200c}\u{200d}\u{feff}\u{0007} \r\n";
        let clean = sanitize_credential(dirty);
        assert_eq!(clean, "1234567890123456");
    }
}
