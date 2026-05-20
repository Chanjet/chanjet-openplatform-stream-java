pub fn get_bin_name() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(real_exe) = std::fs::canonicalize(exe.clone()) {
            if let Some(name) = real_exe.file_name() {
                return name.to_string_lossy().to_string();
            }
        } else if let Some(name) = exe.file_name() {
            return name.to_string_lossy().to_string();
        }
    }
    "cowen".to_string()
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

/// 检查端口占用情况，返回 PID 和 进程名
pub fn check_port_occupancy(port: u16, bin_name: &str) -> Option<(u32, String)> {
    // 1. Try a quick bind to see if it's occupied at all
    if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
        return None;
    }

    // 2. It's occupied. Try to find the process using sysinfo
    use sysinfo::{System, ProcessesToUpdate};
    let mut s = System::new_all();
    s.refresh_processes(ProcessesToUpdate::All, true);
    
    // Scan all processes
    let bin_name_lower = bin_name.to_lowercase();
    let port_str = port.to_string();
    
    for (pid, process) in s.processes() {
        let cmdline = process.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>();
        let cmd_str = cmdline.join(" ");
        
        let has_bin = process.name().to_string_lossy().to_lowercase().contains(&bin_name_lower) || cmd_str.to_lowercase().contains(&bin_name_lower);
        if !has_bin { continue; }

        let is_daemon = cmdline.iter().any(|arg| arg == "daemon") && cmdline.iter().any(|arg| arg == "start");
        let has_port = cmdline.iter().any(|arg| arg == "--proxy-port") &&
                       cmdline.windows(2).any(|w| w[0] == "--proxy-port" && w[1] == port_str);
        
        if is_daemon && has_port && pid.as_u32() != std::process::id() {
            return Some((pid.as_u32(), bin_name.to_string()));
        }
    }

    Some((0, "Unknown Process".to_string()))
}

/// 从进程命令行中提取 Profile 名称
pub fn extract_profile_from_cmdline(pid: u32) -> Option<String> {
    use sysinfo::{System, ProcessesToUpdate, Pid};
    let mut s = System::new();
    let sys_pid = Pid::from_u32(pid);
    s.refresh_processes(ProcessesToUpdate::Some(&[sys_pid]), true);
    if let Some(process) = s.process(sys_pid) {
        let cmdline = process.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>();
        return cmdline.windows(2)
            .find(|w| w[0] == "--profile")
            .map(|w| w[1].to_string());
    }
    None
}
