use crate::{CowenResult, CowenError};
use anyhow::anyhow;
use sha2::{Sha256, Digest};
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce
};
use rand::{RngCore, thread_rng};
use std::path::Path;

pub fn get_machine_fingerprint() -> CowenResult<String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let hostname = hostname::get()?.to_string_lossy().to_string();
    
    let mut hasher = Sha256::new();
    hasher.update(os);
    hasher.update(arch);
    hasher.update(hostname);
    
    Ok(hex::encode(hasher.finalize()))
}

pub fn derive_key(fingerprint: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(fingerprint);
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

pub fn encrypt(data: &[u8], key: &[u8; 32]) -> CowenResult<Vec<u8>> {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0u8; 12];
    thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let ciphertext = cipher.encrypt(nonce, data)
        .map_err(|e| CowenError::Security(format!("Encryption failed: {}", e)))?;
        
    let mut combined = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    Ok(combined)
}

pub fn decrypt(combined: &[u8], key: &[u8; 32]) -> CowenResult<Vec<u8>> {
    if combined.len() < 12 {
        return Err(CowenError::Security("Invalid encrypted data".to_string()));
    }
    
    let cipher = Aes256Gcm::new(key.into());
    let nonce = Nonce::from_slice(&combined[..12]);
    let ciphertext = &combined[12..];
    
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| CowenError::Security(format!("Decryption failed: {}", e)))?;
        
    Ok(plaintext)
}

pub fn seal_config<P: AsRef<Path>>(path: P, data: &[u8], fingerprint: &str) -> CowenResult<()> {
    let key = derive_key(fingerprint);
    let encrypted = encrypt(data, &key)?;
    std::fs::write(path, encrypted)?;
    Ok(())
}

pub fn unseal_config<P: AsRef<Path>>(path: P, fingerprint: &str) -> CowenResult<Vec<u8>> {
    let key = derive_key(fingerprint);
    let encrypted = std::fs::read(path)?;
    decrypt(&encrypted, &key)
}
