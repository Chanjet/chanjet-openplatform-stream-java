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
