use crate::core::config::Config;
use anyhow::{Result, anyhow};
use reqwest::Client;

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
        .use_rustls_tls() // Ensure rustls is used
        .tls_built_in_root_certs(true) // reqwest 0.11 with rustls-tls-native-roots uses this
        .build()
        .map_err(|e| anyhow!("Failed to build http client: {}", e))
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
}
