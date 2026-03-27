use sha2::{Sha256, Digest};
use std::env;

pub fn get_machine_fingerprint() -> anyhow::Result<String> {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    let hostname = hostname::get()?.to_string_lossy().to_string();

    // Mirroring Go: fmt.Sprintf("%s-%s-%s", runtime.GOOS, runtime.GOARCH, hostname)
    let base = format!("{}-{}-{}", os, arch, hostname);
    
    let mut hasher = Sha256::new();
    hasher.update(base.as_bytes());
    let result = hasher.finalize();
    
    Ok(format!("{:x}", result))
}
