use crate::core::config::Config;
use anyhow::{Result, anyhow};
use reqwest::Client;
use std::net::SocketAddr;
use crate::core::security::SecurityError;

/// 生成标准的 User-Agent: Cowen/vX.Y.Z (OS; ARCH)
pub fn get_user_agent() -> String {
    format!(
        "Cowen/{} ({}; {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

/// 创建统一配置的 HttpClient
pub fn create_client(_config: &Config) -> Result<Client> {
    Client::builder()
        .user_agent(get_user_agent())
        .build()
        .map_err(|e| anyhow!("Failed to build http client: {}", e))
}

/// 校验绑定地址是否为回环地址 (127.0.0.1 或 ::1)
pub fn validate_loopback_addr(addr: &SocketAddr) -> Result<(), SecurityError> {
    let ip = addr.ip();
    if ip.is_loopback() {
        Ok(())
    } else {
        Err(SecurityError::IllegalBinding(ip.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_agent_format() {
        let ua = get_user_agent();
        println!("Generated UA: {}", ua);
        
        // 验证格式: Cowen/x.y.z (os; arch)
        assert!(ua.starts_with("Cowen/"));
        assert!(ua.contains("("));
        assert!(ua.contains(";"));
        assert!(ua.ends_with(")"));
        
        // 验证包含版本号 (至少包含一个数字)
        assert!(ua.chars().any(|c| c.is_numeric()));
    }

    #[test]
    fn test_validate_loopback_addr() {
        use std::net::IpAddr;
        
        // 1. Success cases
        let localhost_v4 = SocketAddr::new(IpAddr::V4("127.0.0.1".parse().unwrap()), 8080);
        assert!(validate_loopback_addr(&localhost_v4).is_ok());

        let localhost_v6 = SocketAddr::new(IpAddr::V6("::1".parse().unwrap()), 8080);
        assert!(validate_loopback_addr(&localhost_v6).is_ok());

        // 2. Failure cases
        let any_v4 = SocketAddr::new(IpAddr::V4("0.0.0.0".parse().unwrap()), 8080);
        let err = validate_loopback_addr(&any_v4).unwrap_err();
        assert!(err.to_string().contains("0.0.0.0"));

        let lan_ip = SocketAddr::new(IpAddr::V4("192.168.1.1".parse().unwrap()), 8080);
        assert!(validate_loopback_addr(&lan_ip).is_err());
    }
}
