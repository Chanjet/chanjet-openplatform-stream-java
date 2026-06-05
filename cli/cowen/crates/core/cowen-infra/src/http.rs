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
