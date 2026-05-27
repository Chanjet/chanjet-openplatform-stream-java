use crate::unix::{UnixProcessManager, UnixIpcBinder};
use cowen_infra::sys::SysFingerprint;

pub type LinuxProcessManager = UnixProcessManager;
pub type LinuxIpcBinder = UnixIpcBinder;

pub struct LinuxFingerprint;

impl LinuxFingerprint {
    pub fn new() -> Self {
        Self
    }
}

impl SysFingerprint for LinuxFingerprint {
    fn get_machine_id(&self) -> anyhow::Result<String> {
        // Read /etc/machine-id or /var/lib/dbus/machine-id
        if let Ok(id) = std::fs::read_to_string("/etc/machine-id") {
            let trimmed = id.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
        if let Ok(id) = std::fs::read_to_string("/var/lib/dbus/machine-id") {
            let trimmed = id.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
        
        // Fallback to basic fingerprint
        cowen_infra::sys::derive_fallback_fingerprint("linux")
    }
}

pub struct LinuxServiceManager;

impl LinuxServiceManager {
    pub fn new() -> Self {
        Self
    }
}

fn get_linux_service_path(bin_name: &str) -> anyhow::Result<std::path::PathBuf> {
    let home = directories::UserDirs::new()
        .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
        .home_dir()
        .to_path_buf();
    Ok(home.join(".config").join("systemd").join("user").join(format!("{}-daemon.service", bin_name)))
}

#[async_trait::async_trait]
impl cowen_infra::sys::ServiceManager for LinuxServiceManager {
    async fn install(&self, bin_name: &str, bin_path: &str, _log_dir: &str) -> anyhow::Result<()> {
        let service_path = get_linux_service_path(bin_name)?;
        
        let service_content = format!(r#"[Unit]
Description={prefix}.{bin_name} Daemon Autostart
After=network.target

[Service]
Type=simple
ExecStart={bin_path} daemon start --all --foreground
Restart=always

[Install]
WantedBy=default.target
"#, 
        prefix = cowen_infra::sys::SERVICE_PREFIX,
        bin_name = bin_name,
        bin_path = bin_path);

        if let Some(parent) = service_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&service_path, service_content)?;

        // Reload and enable
        let _ = std::process::Command::new("systemctl").arg("--user").arg("daemon-reload").status();
        let status = std::process::Command::new("systemctl")
            .arg("--user")
            .arg("enable")
            .arg("--now")
            .arg(format!("{}-daemon", bin_name))
            .status()?;

        if status.success() {
            println!("✅ Successfully installed and enabled systemd user service.");
            println!("📍 Config: {:?}", service_path);
            Ok(())
        } else {
            anyhow::bail!("Failed to enable systemd user service via systemctl. Config created at {:?}", service_path)
        }
    }

    async fn uninstall(&self, bin_name: &str) -> anyhow::Result<()> {
        let service_path = get_linux_service_path(bin_name)?;
        let unit_name = format!("{}-daemon", bin_name);

        if service_path.exists() {
            let _ = std::process::Command::new("systemctl").arg("--user").arg("disable").arg("--now").arg(&unit_name).status();
            std::fs::remove_file(&service_path)?;
            let _ = std::process::Command::new("systemctl").arg("--user").arg("daemon-reload").status();
            println!("✅ Successfully uninstalled systemd user service.");
        } else {
            println!("ℹ️  No service found to uninstall.");
        }
        Ok(())
    }

    async fn status(&self, bin_name: &str) -> anyhow::Result<String> {
        let service_path = get_linux_service_path(bin_name)?;
        let unit_name = format!("{}-daemon", bin_name);

        let output = std::process::Command::new("systemctl")
            .arg("--user")
            .arg("is-active")
            .arg(&unit_name)
            .output();

        let status_str = match output {
            Ok(out) => {
                let state = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if state == "active" {
                    cowen_infra::sys::STATUS_ACTIVE
                } else if state == "inactive" || state == "failed" {
                    cowen_infra::sys::STATUS_INACTIVE
                } else {
                    cowen_infra::sys::STATUS_UNKNOWN
                }
            }
            _ => cowen_infra::sys::STATUS_UNKNOWN,
        };

        Ok(cowen_infra::sys::format_service_status("Linux", &unit_name, service_path.exists(), status_str))
    }
}

/// 设置当前进程的显示名称 (Linux 实现)
pub fn set_process_name(name: &str) {
    use std::ffi::CString;
    if let Ok(c_name) = CString::new(name) {
        unsafe {
            libc::prctl(libc::PR_SET_NAME, c_name.as_ptr(), 0, 0, 0);
        }
    }
}
