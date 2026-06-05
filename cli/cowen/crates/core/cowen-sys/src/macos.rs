use crate::unix::{UnixProcessManager, UnixIpcBinder};
use cowen_infra::sys::SysFingerprint;

pub type MacProcessManager = UnixProcessManager;
pub type MacIpcBinder = UnixIpcBinder;

pub struct MacFingerprint;

impl MacFingerprint {
    pub fn new() -> Self {
        Self
    }
}

impl SysFingerprint for MacFingerprint {
    fn get_machine_id(&self) -> anyhow::Result<String> {
        // macOS Hardware UUID extraction
        if let Ok(output) = std::process::Command::new("ioreg")
            .args(&["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("IOPlatformUUID") {
                    let parts: Vec<&str> = line.split('"').collect();
                    if parts.len() >= 4 {
                        let uuid = parts[3].to_string();
                        if uuid.len() == 36 {
                            return Ok(uuid);
                        }
                    }
                }
            }
        }
        
        // Fallback to basic fingerprint if ioreg fails or doesn't have UUID
        cowen_infra::sys::derive_fallback_fingerprint("macos")
    }
}

pub struct MacServiceManager;

impl MacServiceManager {
    pub fn new() -> Self {
        Self
    }
}

fn get_macos_plist_path(bin_name: &str) -> anyhow::Result<std::path::PathBuf> {
    let home = directories::UserDirs::new()
        .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
        .home_dir()
        .to_path_buf();
    Ok(home.join("Library").join("LaunchAgents").join(format!("{}.{}.daemon.plist", cowen_infra::sys::SERVICE_PREFIX, bin_name)))
}

#[async_trait::async_trait]
impl cowen_infra::sys::ServiceManager for MacServiceManager {
    async fn install(&self, bin_name: &str, bin_path: &str, log_dir: &str) -> anyhow::Result<()> {
        let plist_path = get_macos_plist_path(bin_name)?;
        std::fs::create_dir_all(log_dir)?;

        let plist_content = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{prefix}.{bin_name}.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin_path}</string>
        <string>--auto-start-all</string>
    </array>
    <key>KeepAlive</key>
    <true/>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{log_path}/service.log</string>
    <key>StandardErrorPath</key>
    <string>{log_path}/service.error.log</string>
</dict>
</plist>"#, 
        prefix = cowen_infra::sys::SERVICE_PREFIX,
        bin_name = bin_name,
        bin_path = bin_path,
        log_path = log_dir);

        if let Some(parent) = plist_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&plist_path, plist_content)?;

        // If the service is already loaded, unload it first
        let _ = std::process::Command::new("launchctl")
            .arg("unload")
            .arg(&plist_path)
            .status();

        // Load the service
        let status = std::process::Command::new("launchctl")
            .arg("load")
            .arg(&plist_path)
            .status()?;

        if status.success() {
            println!("✅ Successfully installed and loaded macOS LaunchAgent.");
            println!("📍 Config: {:?}", plist_path);
            Ok(())
        } else {
            anyhow::bail!("Failed to load LaunchAgent via launchctl. Plist created at {:?}", plist_path)
        }
    }

    async fn uninstall(&self, bin_name: &str) -> anyhow::Result<()> {
        let plist_path = get_macos_plist_path(bin_name)?;
        if plist_path.exists() {
            let _ = std::process::Command::new("launchctl")
                .arg("unload")
                .arg(&plist_path)
                .status();
            std::fs::remove_file(&plist_path)?;
            println!("✅ Successfully uninstalled macOS LaunchAgent.");
        } else {
            println!("ℹ️  No service found to uninstall.");
        }
        Ok(())
    }

    async fn status(&self, bin_name: &str) -> anyhow::Result<String> {
        let plist_path = get_macos_plist_path(bin_name)?;
        let label = format!("{}.{}.daemon", cowen_infra::sys::SERVICE_PREFIX, bin_name);

        let output = std::process::Command::new("launchctl")
            .arg("list")
            .arg(&label)
            .output();

        let status_str = match output {
            Ok(out) if out.status.success() => cowen_infra::sys::STATUS_ACTIVE,
            _ => cowen_infra::sys::STATUS_NOT_REGISTERED,
        };

        Ok(cowen_infra::sys::format_service_status("macOS", &label, plist_path.exists(), status_str))
    }
}

/// 设置当前进程的显示名称 (macOS 实现)
pub fn set_process_name(name: &str) {
    use std::ffi::CString;
    if let Ok(c_name) = CString::new(name) {
        unsafe {
            libc::pthread_setname_np(c_name.as_ptr());
        }
    }
}
