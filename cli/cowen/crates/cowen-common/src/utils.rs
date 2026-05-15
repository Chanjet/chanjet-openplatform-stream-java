use crate::CowenResult;
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

pub fn mask_url(url: &str) -> String {
    use regex::Regex;
    // Pattern to match userinfo in URL: scheme://[user:pass@]host
    let re = Regex::new(r"^([^:]+://)([^@/]+@)(.*)$").unwrap();
    if let Some(caps) = re.captures(url) {
        let userinfo = &caps[2];
        if let Some(colon_idx) = userinfo.find(':') {
            // mask password part: user:***@
            format!("{}{}:***@{}", &caps[1], &userinfo[..colon_idx], &caps[3])
        } else {
            // no password, just user@: mask user: ***@
            format!("{}***@{}", &caps[1], &caps[3])
        }
    } else {
        url.to_string()
    }
}

/// 设置当前进程的显示名称 (跨平台实现)
pub fn set_process_name(name: &str) {
    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        if let Ok(c_name) = CString::new(name) {
            unsafe {
                libc::prctl(libc::PR_SET_NAME, c_name.as_ptr(), 0, 0, 0);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        if let Ok(c_name) = CString::new(name) {
            unsafe {
                libc::pthread_setname_np(c_name.as_ptr());
            }
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = name;
        // Unsupported platforms: Fallback to doing nothing
    }
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
        // This test ensures the function can be called without panic.
        // Verifying the actual name change is platform-dependent and usually requires 
        // external tools or reading OS-specific files, so we verify call stability here.
        set_process_name("cowen-test-process");
        
        #[cfg(unix)]
        {
            // Optional: on Linux/macOS, we could try to read back the name via sysinfo if we wanted to be very strict.
            // But for now, we just ensure it's integrated.
        }
    }
}
