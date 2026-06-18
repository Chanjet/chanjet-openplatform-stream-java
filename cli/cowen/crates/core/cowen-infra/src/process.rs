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
    let _ = name;
}

/// 检查端口占用情况，返回 PID 和 进程名
pub fn check_port_occupancy(port: u16, bin_name: &str) -> Option<(u32, String)> {
    // 1. Try a quick bind to see if it's occupied at all
    if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
        return None;
    }

    #[cfg(unix)]
    {
        let output = std::process::Command::new("lsof")
            .arg("-i")
            .arg(format!("tcp:{}", port))
            .arg("-t")
            .output();
        if let Ok(out) = output {
            let pid_str = String::from_utf8_lossy(&out.stdout);
            for line in pid_str.lines() {
                if let Ok(pid) = line.trim().parse::<u32>() {
                    use sysinfo::ProcessesToUpdate;
                    let mut s = sysinfo::System::new();
                    let sys_pid = sysinfo::Pid::from_u32(pid);
                    s.refresh_processes(ProcessesToUpdate::Some(&[sys_pid]), true);
                    if let Some(process) = s.process(sys_pid) {
                        let name = process.name().to_string_lossy().to_string();
                        return Some((pid, name));
                    }
                    return Some((pid, "Unknown Process".to_string()));
                }
            }
        }
    }

    // 2. Fallback/Windows: It's occupied. Try to find the process using sysinfo
    use sysinfo::{ProcessesToUpdate, System};
    let mut s = System::new();
    s.refresh_processes(ProcessesToUpdate::All, true);

    // Scan all processes
    let bin_name_lower = bin_name.to_lowercase();

    for (pid, process) in s.processes() {
        let cmdline = process
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>();
        let cmd_str = cmdline.join(" ");

        let has_bin = process
            .name()
            .to_string_lossy()
            .to_lowercase()
            .contains(&bin_name_lower)
            || cmd_str.to_lowercase().contains(&bin_name_lower);
        if !has_bin {
            continue;
        }

        let is_daemon = cmdline
            .iter()
            .any(|arg| arg.contains("cowen-daemon") || arg == "daemon");
        let is_cowen_exe = process
            .name()
            .to_string_lossy()
            .to_lowercase()
            .contains("cowen-daemon");

        if (is_daemon || is_cowen_exe) && pid.as_u32() != std::process::id() {
            return Some((pid.as_u32(), bin_name.to_string()));
        }
    }

    Some((0, "Unknown Process".to_string()))
}

/// 从进程命令行中提取 Profile 名称
pub fn extract_profile_from_cmdline(pid: u32) -> Option<String> {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let mut s = System::new();
    let sys_pid = Pid::from_u32(pid);
    s.refresh_processes(ProcessesToUpdate::Some(&[sys_pid]), true);
    if let Some(process) = s.process(sys_pid) {
        let cmdline = process
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>();
        return cmdline
            .windows(2)
            .find(|w| w[0] == "--profile")
            .map(|w| w[1].to_string());
    }
    None
}
