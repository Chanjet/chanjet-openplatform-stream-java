use reqwest::Client;

/// 创建统一配置的 HttpClient
pub fn create_client(user_agent: &str) -> Result<Client, reqwest::Error> {
    let timeout_secs = std::env::var("COWEN_HTTP_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(300);

    Client::builder()
        .user_agent(user_agent)
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
}

/// 生成标准的 User-Agent
pub fn get_user_agent(version: &str) -> String {
    format!(
        "Cowen/{} ({}; {})",
        version,
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_user_agent() {
        let ua = get_user_agent("1.0.0");
        assert!(ua.starts_with("Cowen/1.0.0 ("));
        assert!(ua.contains(std::env::consts::OS));
        assert!(ua.contains(std::env::consts::ARCH));
    }

    #[test]
    fn test_create_client() {
        let client = create_client("test_agent");
        assert!(client.is_ok());
    }
}
