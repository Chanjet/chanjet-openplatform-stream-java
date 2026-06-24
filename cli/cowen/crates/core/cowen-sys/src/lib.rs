use std::sync::Arc;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
mod windows;

pub mod plugin;
pub use plugin::{discover_plugins, PluginLoader};

pub use cowen_infra::sys::{IpcBinder, ProcessManager, ServiceManager, SysFingerprint};

pub fn get_process_manager() -> Arc<dyn ProcessManager> {
    #[cfg(target_os = "macos")]
    return Arc::new(macos::MacProcessManager::new());

    #[cfg(target_os = "linux")]
    return Arc::new(linux::LinuxProcessManager::new());

    #[cfg(windows)]
    return Arc::new(windows::WinProcessManager::new());

    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    compile_error!("Unsupported platform!");
}

pub fn get_sys_fingerprint() -> Arc<dyn SysFingerprint> {
    #[cfg(target_os = "macos")]
    return Arc::new(macos::MacFingerprint::new());

    #[cfg(target_os = "linux")]
    return Arc::new(linux::LinuxFingerprint::new());

    #[cfg(windows)]
    return Arc::new(windows::WinFingerprint::new());

    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    compile_error!("Unsupported platform!");
}

pub fn get_ipc_binder() -> Arc<dyn IpcBinder> {
    #[cfg(target_os = "macos")]
    return Arc::new(macos::MacIpcBinder::new());

    #[cfg(target_os = "linux")]
    return Arc::new(linux::LinuxIpcBinder::new());

    #[cfg(windows)]
    return Arc::new(windows::WinIpcBinder::new());

    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    compile_error!("Unsupported platform!");
}

pub fn get_service_manager() -> Arc<dyn ServiceManager> {
    #[cfg(target_os = "macos")]
    return Arc::new(macos::MacServiceManager::new());

    #[cfg(target_os = "linux")]
    return Arc::new(linux::LinuxServiceManager::new());

    #[cfg(windows)]
    return Arc::new(windows::WinServiceManager::new());

    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    compile_error!("Unsupported platform!");
}

#[cfg(target_os = "linux")]
pub use linux::set_process_name;
#[cfg(target_os = "macos")]
pub use macos::set_process_name;
#[cfg(windows)]
pub use windows::set_process_name;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
pub fn set_process_name(name: &str) {
    let _ = name;
}

#[cfg(unix)]
pub use unix::fs;
#[cfg(windows)]
pub use windows::fs;

pub fn get_supported_plugin_extensions() -> &'static [&'static str] {
    #[cfg(windows)]
    return &["exe"];
    #[cfg(not(windows))]
    return &[""];
}

pub fn get_daemon_binary_name() -> &'static str {
    #[cfg(windows)]
    return "cowen-daemon.exe";
    #[cfg(not(windows))]
    return "cowen-daemon";
}

pub fn append_executable_extension(name: &str) -> String {
    #[cfg(windows)]
    {
        if name.ends_with(".exe") {
            name.to_string()
        } else {
            format!("{}.exe", name)
        }
    }
    #[cfg(not(windows))]
    {
        name.to_string()
    }
}

pub fn is_windows() -> bool {
    #[cfg(windows)]
    return true;
    #[cfg(not(windows))]
    return false;
}

pub fn get_system_plugin_search_paths() -> Vec<std::path::PathBuf> {
    let mut paths = vec![];
    #[cfg(unix)]
    {
        paths.push(std::path::PathBuf::from(
            "/usr/local/share/cowen/system_plugins",
        ));
    }
    #[cfg(windows)]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(parent) = exe_path.parent() {
                paths.push(parent.join("system_plugins"));
            }
        }
    }
    paths
}

pub fn handle_parent_signals_for_child(child_id: u32) {
    #[cfg(unix)]
    {
        tokio::spawn(async move {
            use tokio::signal::unix::{signal, SignalKind};
            if let (Ok(mut sigterm), Ok(mut sigint)) = (
                signal(SignalKind::terminate()),
                signal(SignalKind::interrupt()),
            ) {
                tokio::select! {
                    _ = sigterm.recv() => {
                        let pm = get_process_manager();
                        let _ = pm.kill_process(child_id, false).await;
                    }
                    _ = sigint.recv() => {
                        let _ = std::process::Command::new("kill").arg("-2").arg(child_id.to_string()).status();
                    }
                }
            }
        });
    }
    #[cfg(not(unix))]
    {
        let _ = child_id;
    }
}

pub fn register_shutdown_signals(stop_tx: tokio::sync::mpsc::Sender<()>) {
    #[cfg(unix)]
    {
        tokio::spawn(async move {
            use tokio::signal::unix::{signal, SignalKind};
            if let Ok(mut stream) = signal(SignalKind::terminate()) {
                stream.recv().await;
                tracing::info!("SIGTERM received, sending shutdown signal...");
                let _ = stop_tx.send(()).await;
            }
        });
    }
    #[cfg(not(unix))]
    {
        let _ = stop_tx;
    }
}

pub fn check_port_occupancy(port: u16, bin_name: &str) -> Option<(u32, String)> {
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

    use sysinfo::{ProcessesToUpdate, System};
    let mut s = System::new();
    s.refresh_processes(ProcessesToUpdate::All, true);

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

pub fn create_sandboxed_command(
    binary_path: &std::path::Path,
    sandbox_path: &std::path::Path,
    allowed_roots: &[std::path::PathBuf],
) -> std::process::Command {
    #[cfg(target_os = "macos")]
    {
        let mut profile = format!(
            r#"(version 1)
(allow default)
(deny file-write*)
(allow file-write* (subpath "/private/var"))
(allow file-write* (subpath "/var/folders"))
(allow file-write* (subpath "/tmp"))
(allow file-write* (subpath "{}"))"#,
            sandbox_path.to_string_lossy()
        );

        // Dynamically append allowed roots as permitted file-write subpaths
        for path in allowed_roots {
            profile.push_str(&format!(
                "\n(allow file-write* (subpath \"{}\"))",
                path.to_string_lossy()
            ));
        }

        let mut cmd = std::process::Command::new("sandbox-exec");
        cmd.arg("-p").arg(profile).arg(binary_path);
        cmd
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = allowed_roots;
        let _ = sandbox_path;
        std::process::Command::new(binary_path)
    }
}

pub fn configure_socket_reuse(socket: &tokio::net::TcpSocket) -> std::io::Result<()> {
    socket.set_reuseaddr(true)?;
    #[cfg(unix)]
    {
        socket.set_reuseport(true)?;
    }
    Ok(())
}

pub async fn wait_for_terminate() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        sigterm.recv().await;
    }
    #[cfg(windows)]
    {
        let mut ctrl_break = tokio::signal::windows::ctrl_break()?;
        ctrl_break.recv().await;
    }
    #[cfg(not(any(unix, windows)))]
    {
        tokio::time::sleep(std::time::Duration::from_secs(u64::MAX)).await;
    }
    Ok(())
}

pub fn get_onnx_library_bytes() -> &'static [u8] {
    #[cfg(target_os = "windows")]
    return include_bytes!("../../../../dist_assets/windows/onnxruntime.dll");
    #[cfg(target_os = "macos")]
    return include_bytes!("../../../../dist_assets/macos/libonnxruntime.dylib");
    #[cfg(target_os = "linux")]
    return include_bytes!("../../../../dist_assets/linux/libonnxruntime.so");
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    return &[];
}

pub fn get_onnx_library_name() -> &'static str {
    #[cfg(windows)]
    return "onnxruntime.dll";
    #[cfg(target_os = "macos")]
    return "libonnxruntime.dylib";
    #[cfg(not(any(windows, target_os = "macos")))]
    return "libonnxruntime.so";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sys_fs_secure_operations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secure_test.txt");

        // Test secure_write
        assert!(fs::secure_write(&path, b"hello world").is_ok());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello world");

        // Test secure_open_write
        let open_res = fs::secure_open_write(&path);
        assert!(open_res.is_ok());

        // Test secure_open_append
        let append_res = fs::secure_open_append(&path);
        assert!(append_res.is_ok());

        // Test make_executable
        assert!(fs::make_executable(&path).is_ok());

        // Test is_file_secure
        assert!(fs::is_file_secure(&path));
    }

    #[test]
    fn test_create_sandboxed_command_roots() {
        use std::path::PathBuf;
        let binary = PathBuf::from("ls");
        let sandbox = PathBuf::from("/tmp");
        let allowed = vec![PathBuf::from("/Users/zhangliang/workspace")];
        let cmd = create_sandboxed_command(&binary, &sandbox, &allowed);

        #[cfg(target_os = "macos")]
        {
            let args: Vec<_> = cmd.get_args().collect();
            assert_eq!(args[0], "-p");
            let profile = args[1].to_str().unwrap();
            assert!(
                profile.contains("(allow file-write* (subpath \"/Users/zhangliang/workspace\"))"),
                "macOS Seatbelt profile must dynamically append allowed workspace subpaths"
            );
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = cmd;
        }
    }
}
