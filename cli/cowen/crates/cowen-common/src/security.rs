use sha2::{Sha256, Digest};
use std::env;
use anyhow::{Result, anyhow};
use thiserror::Error;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};

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

pub fn derive_key(fingerprint: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(fingerprint.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

pub fn encrypt(data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0u8; 12];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|e| anyhow!("Encryption failed: {}", e))?;

    // Output: Nonce (12) + Ciphertext
    let mut combined = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    Ok(combined)
}

pub fn decrypt(combined: &[u8], key: &[u8; 32]) -> Result<Vec<u8>> {
    if combined.len() < 12 {
        return Err(anyhow!("Invalid encrypted data"));
    }

    let cipher = Aes256Gcm::new(key.into());
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("Decryption failed: {}", e))?;

    Ok(plaintext)
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
