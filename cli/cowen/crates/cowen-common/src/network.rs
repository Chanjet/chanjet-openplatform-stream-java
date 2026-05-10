use crate::{CowenResult, CowenError};
use crate::config::Config;
use reqwest::Client;
use std::net::SocketAddr;

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
pub fn create_client(_config: &Config) -> CowenResult<Client> {
    Client::builder()
        .user_agent(get_user_agent())
        .build()
        .map_err(|e| CowenError::Network(e))
}

/// 校验绑定地址是否为回环地址 (127.0.0.1 或 ::1)
pub fn validate_loopback_addr(addr: &SocketAddr) -> CowenResult<()> {
    let ip = addr.ip();
    if ip.is_loopback() {
        Ok(())
    } else {
        Err(CowenError::Security(format!("Illegal binding: {}", ip)))
    }
}
