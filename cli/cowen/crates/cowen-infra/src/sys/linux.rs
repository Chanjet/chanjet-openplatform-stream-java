use crate::sys::unix::{UnixProcessManager, UnixIpcBinder};
use crate::sys::SysFingerprint;

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
        let hostname = hostname::get()?.to_string_lossy().to_string();
        let base = format!("linux-{}", hostname);
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(base.as_bytes());
        let hash = hasher.finalize();
        Ok(hash.iter().map(|b| format!("{:02x}", b)).collect())
    }
}
