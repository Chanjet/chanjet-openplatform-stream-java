use anyhow::{Result, Context};
use std::path::PathBuf;
use std::fs;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit},
    Aes256Gcm, Nonce,
};
use sha2::{Sha256, Digest};
use std::collections::HashMap;

pub trait Vault: Send + Sync {
    fn get(&self, profile: &str, key: &str) -> Result<String>;
    fn set(&self, profile: &str, key: &str, secret: &str) -> Result<()>;
}

pub struct MultiVault {
    seal_path: PathBuf,
    master_key: Vec<u8>,
}

impl MultiVault {
    pub fn new(seal_path: PathBuf, master_key_str: &str) -> Result<Self> {
        let mut hasher = Sha256::new();
        hasher.update(master_key_str.as_bytes());
        let key = hasher.finalize().to_vec();

        Ok(Self {
            seal_path,
            master_key: key,
        })
    }

    fn get_full_key(&self, profile: &str, key: &str) -> String {
        format!("{}:{}", profile, key)
    }

    fn read_seal(&self) -> Result<HashMap<String, String>> {
        if !self.seal_path.exists() {
            return Ok(HashMap::new());
        }

        let encrypted_data = fs::read(&self.seal_path)?;
        let decrypted_data = self.decrypt_aes_gcm(&encrypted_data)?;
        let data: HashMap<String, String> = serde_json::from_slice(&decrypted_data)?;
        Ok(data)
    }

    fn write_seal(&self, data: &HashMap<String, String>) -> Result<()> {
        let json_data = serde_json::to_vec(data)?;
        let encrypted_data = self.encrypt_aes_gcm(&json_data)?;
        
        if let Some(parent) = self.seal_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.seal_path, encrypted_data)?;
        Ok(())
    }

    fn encrypt_aes_gcm(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(&self.master_key).map_err(|e| anyhow::anyhow!(e))?;
        let nonce = Aes256Gcm::generate_nonce(&mut rand::thread_rng()); // Default is 12 bytes
        let ciphertext = cipher.encrypt(&nonce, plaintext).map_err(|e| anyhow::anyhow!(e))?;
        
        // Go implementation: nonce + ciphertext
        let mut result = nonce.to_vec();
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    fn decrypt_aes_gcm(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() < 12 {
            return Err(anyhow::anyhow!("Ciphertext too short"));
        }
        let (nonce_bytes, actual_ciphertext) = ciphertext.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let cipher = Aes256Gcm::new_from_slice(&self.master_key).map_err(|e| anyhow::anyhow!(e))?;
        
        cipher.decrypt(nonce, actual_ciphertext).map_err(|e| anyhow::anyhow!(e))
    }
}

impl Vault for MultiVault {
    fn set(&self, profile: &str, key: &str, secret: &str) -> Result<()> {
        let full_key = self.get_full_key(profile, key);
        let mut data = self.read_seal()?;
        data.insert(full_key, secret.to_string());
        self.write_seal(&data)
    }

    fn get(&self, profile: &str, key: &str) -> Result<String> {
        let full_key = self.get_full_key(profile, key);
        let data = self.read_seal()?;
        data.get(&full_key)
            .cloned()
            .context("Secret not found in vault")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_vault_lifecycle() -> Result<()> {
        let tmp = tempdir()?;
        let seal_path = tmp.path().join(".seal");
        let vault = MultiVault::new(seal_path, "master_pwd")?;

        // 1. Set
        vault.set("default", "app_secret", "secret123")?;

        // 2. Get
        let val = vault.get("default", "app_secret")?;
        assert_eq!(val, "secret123");

        Ok(())
    }

    #[test]
    fn test_aes_gcm_direct() -> Result<()> {
        let vault = MultiVault::new(PathBuf::from("/tmp/nonexistent"), "pwd")?;
        let plaintext = b"hello world";
        
        let encrypted = vault.encrypt_aes_gcm(plaintext)?;
        let decrypted = vault.decrypt_aes_gcm(&encrypted)?;
        
        assert_eq!(plaintext.as_ref(), decrypted.as_slice());
        Ok(())
    }
}
