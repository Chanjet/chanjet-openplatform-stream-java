use crate::sys::unix::{UnixProcessManager, UnixIpcBinder};
use crate::sys::SysFingerprint;

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
        let hostname = hostname::get()?.to_string_lossy().to_string();
        let base = format!("macos-{}", hostname);
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(base.as_bytes());
        let hash = hasher.finalize();
        Ok(hash.iter().map(|b| format!("{:02x}", b)).collect())
    }
}
