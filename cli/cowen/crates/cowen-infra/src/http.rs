use reqwest::Client;

/// 创建统一配置的 HttpClient
pub fn create_client(user_agent: &str) -> Result<Client, reqwest::Error> {
    Client::builder()
        .user_agent(user_agent)
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
