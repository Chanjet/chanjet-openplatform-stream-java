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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[test]
    fn test_validate_loopback_addr() {
        let loopback = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        assert!(validate_loopback_addr(&loopback).is_ok());

        let non_loopback = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        assert!(validate_loopback_addr(&non_loopback).is_err());
    }
}
