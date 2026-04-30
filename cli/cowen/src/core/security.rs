use sha2::{Sha256, Digest};
use std::env;
use anyhow::Result;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SecurityError {
    #[error("Security Violation: Listening on illegal non-loopback address {0}. All local services must bind to 127.0.0.1 or ::1.")]
    IllegalBinding(String),
}

pub fn get_machine_fingerprint() -> Result<String> {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    let hostname = hostname::get()?.to_string_lossy().to_string();

    let base = format!("{}-{}-{}", os, arch, hostname);
    
    let mut hasher = Sha256::new();
    hasher.update(base.as_bytes());
    let result = hasher.finalize();
    
    Ok(hex::encode(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_machine_fingerprint() {
        let fingerprint = get_machine_fingerprint().unwrap();
        assert!(!fingerprint.is_empty());
        assert_eq!(fingerprint.len(), 64); // SHA256 hex
    }
}
