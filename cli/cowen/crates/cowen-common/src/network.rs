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

/// 检查端口占用情况，返回 PID 和 进程名
pub fn check_port_occupancy(port: u16) -> Option<(u32, String)> {
    // 1. Try a quick bind to see if it's occupied at all
    if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
        return None;
    }

    // 2. It's occupied. Try to find the process using sysinfo
    use sysinfo::{System, ProcessesToUpdate};
    let mut s = System::new_all();
    s.refresh_processes(ProcessesToUpdate::All, true);
    
    // Scan all processes
    let bin_name = crate::utils::get_bin_name().to_lowercase();
    let port_str = port.to_string();
    
    for (pid, process) in s.processes() {
        let cmdline = process.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>();
        let cmd_str = cmdline.join(" ");
        
        let has_bin = process.name().to_string_lossy().to_lowercase().contains(&bin_name) || cmd_str.to_lowercase().contains(&bin_name);
        if !has_bin { continue; }

        let is_daemon = cmdline.iter().any(|arg| arg == "daemon") && cmdline.iter().any(|arg| arg == "start");
        let has_port = cmdline.iter().any(|arg| arg == "--proxy-port") &&
                       cmdline.windows(2).any(|w| w[0] == "--proxy-port" && w[1] == port_str);
        
        if is_daemon && has_port && pid.as_u32() != std::process::id() {
            return Some((pid.as_u32(), bin_name.clone()));
        }
    }

    // If we can't find the exact process but bind failed, it's occupied by "Something Else"
    // On Unix, we could potentially parse /proc/net/tcp or use lsof but that's complex for a common lib.
    Some((0, "Unknown Process".to_string()))
}

/// 从进程命令行中提取 Profile 名称
pub fn extract_profile_from_cmdline(pid: u32) -> Option<String> {
    use sysinfo::{System, ProcessesToUpdate, Pid};
    let mut s = System::new_all();
    s.refresh_processes(ProcessesToUpdate::All, true);
    if let Some(process) = s.process(Pid::from_u32(pid)) {
        let cmdline = process.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>();
        return cmdline.windows(2)
            .find(|w| w[0] == "--profile")
            .map(|w| w[1].to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn test_port_occupancy_detection() {
        // Find a random free port
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        
        // Should be occupied (by us)
        let occupancy = check_port_occupancy(port);
        assert!(occupancy.is_some());
        
        let (_, name) = occupancy.unwrap();
        // Since it's not a "cowen" process, it should be "Unknown Process"
        assert_eq!(name, "Unknown Process");
        
        // Drop listener
        drop(listener);
        
        // Should be free now (give it a tiny moment if needed, but bind usually releases immediately)
        assert!(check_port_occupancy(port).is_none());
    }
}
