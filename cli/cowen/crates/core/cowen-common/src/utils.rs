use crate::CowenResult;
pub use cowen_infra::{
    get_bin_name, mask_sensitive_json, mask_string, mask_tail, mask_url, mask_url_query, obfs,
};
pub use cowen_sys::set_process_name;
use serde::Serialize;

pub fn render<T: Serialize>(data: &T, format: &str) -> CowenResult<()> {
    let output = match format {
        "json" => serde_json::to_string_pretty(data)?,
        "yaml" => serde_yaml::to_string(data)?,
        _ => serde_json::to_string_pretty(data)?,
    };

    if std::env::var("COWEN_RAW_OUTPUT").unwrap_or_default() == "true" {
        println!("{}", output);
    } else {
        println!("{}", mask_sensitive_json(&output));
    }
    Ok(())
}

pub fn print_error_json(err_msg: &str) {
    let mut map = serde_json::Map::new();
    map.insert(
        "status".to_string(),
        serde_json::Value::String("failed".to_string()),
    );
    map.insert(
        "error".to_string(),
        serde_json::Value::String(err_msg.to_string()),
    );
    if let Ok(json_str) = serde_json::to_string_pretty(&serde_json::Value::Object(map)) {
        println!("{}", json_str);
    }
}

pub fn sanitize_credential(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            c != '\u{200b}'
                && c != '\u{200c}'
                && c != '\u{200d}'
                && c != '\u{feff}'
                && !c.is_control()
        })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn secure_write<P: std::convert::AsRef<std::path::Path>, C: std::convert::AsRef<[u8]>>(
    path: P,
    contents: C,
) -> std::io::Result<()> {
    cowen_sys::fs::secure_write(path, contents)
}

pub fn is_cowen_process_name(name: &str, current_exe_name: Option<&str>) -> bool {
    let name_lower = name.to_lowercase();
    if name_lower.contains("cowen") {
        return true;
    }

    let is_static_match = matches!(name_lower.as_str(), "cowen" | "cowen.exe")
        || name_lower == get_bin_name().to_lowercase()
        || (name_lower.starts_with("cowen") && name_lower.contains("daemon"));

    is_static_match || current_exe_name.is_some_and(|curr| name_lower == curr.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_cowen_process_name() {
        assert!(is_cowen_process_name("cowen", None));
        assert!(is_cowen_process_name("cowen.exe", None));
        assert!(is_cowen_process_name("cowen-daemon", None));
        assert!(is_cowen_process_name("cowen-daemon.exe", None));
        assert!(is_cowen_process_name("cowen-daemon-something", None));
        assert!(is_cowen_process_name("cowen_daemon_something", None));
        assert!(is_cowen_process_name(
            "cowen_case_60",
            Some("cowen_case_60")
        ));
        assert!(!is_cowen_process_name("some-other-app", None));
    }

    #[test]
    fn test_mask_url() {
        assert_eq!(
            mask_url("redis://:password@localhost:6379"),
            "redis://:***@localhost:6379"
        );
        assert_eq!(
            mask_url("mysql://user:pass@127.0.0.1:3306/db"),
            "mysql://user:***@127.0.0.1:3306/db"
        );
        assert_eq!(
            mask_url("postgres://admin@localhost/mydb"),
            "postgres://***@localhost/mydb"
        );
        assert_eq!(
            mask_url("https://openapi.chanjet.com"),
            "https://openapi.chanjet.com"
        );
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
