use std::net::SocketAddr;

/// 校验绑定地址是否为回环地址 (127.0.0.1 或 ::1)
pub fn validate_loopback_addr(addr: &SocketAddr) -> Result<(), String> {
    let ip = addr.ip();
    if ip.is_loopback() {
        Ok(())
    } else {
        Err(format!("Illegal binding: {}", ip))
    }
}
