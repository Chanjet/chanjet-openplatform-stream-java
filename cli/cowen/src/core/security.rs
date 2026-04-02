use sha2::{Sha256, Digest};
use std::env;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{Result, anyhow};
use rand::{Rng, thread_rng};

pub fn get_machine_fingerprint() -> Result<String> {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    let hostname = hostname::get()?.to_string_lossy().to_string();

    let base = format!("{}-{}-{}", os, arch, hostname);
    
    let mut hasher = Sha256::new();
    hasher.update(base.as_bytes());
    let result = hasher.finalize();
    
    Ok(format!("{:x}", result))
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
    thread_rng().fill(&mut nonce_bytes);
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

    #[test]
    fn test_derive_key() {
        let key1 = derive_key("test-fingerprint");
        let key2 = derive_key("test-fingerprint");
        let key3 = derive_key("other-fingerprint");

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_encrypt_decrypt_cycle() {
        let key = derive_key("secret-key");
        let data = b"hello world secure message";
        
        let encrypted = encrypt(data, &key).expect("Encryption failed");
        assert!(encrypted.len() > data.len());
        
        let decrypted = decrypt(&encrypted, &key).expect("Decryption failed");
        assert_eq!(data, decrypted.as_slice());
    }

    #[test]
    fn test_decrypt_invalid_data() {
        let key = derive_key("secret-key");
        let too_short = vec![0u8; 11];
        let result = decrypt(&too_short, &key);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid encrypted data"));
    }

    #[test]
    fn test_decrypt_wrong_key() {
        let key1 = derive_key("key-1");
        let key2 = derive_key("key-2");
        let data = b"sensitive info";
        
        let encrypted = encrypt(data, &key1).unwrap();
        let result = decrypt(&encrypted, &key2);
        assert!(result.is_err());
    }
}
