use std::sync::Arc;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
mod windows;

pub mod plugin;
pub use plugin::{PluginLoader, discover_plugins};

pub use cowen_infra::sys::{ProcessManager, SysFingerprint, IpcBinder, ServiceManager};

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

#[cfg(target_os = "macos")]
pub use macos::set_process_name;
#[cfg(target_os = "linux")]
pub use linux::set_process_name;
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
            assert!(profile.contains("(allow file-write* (subpath \"/Users/zhangliang/workspace\"))"), "macOS Seatbelt profile must dynamically append allowed workspace subpaths");
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = cmd;
        }
    }
}

