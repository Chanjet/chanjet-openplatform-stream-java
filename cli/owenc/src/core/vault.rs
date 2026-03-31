use anyhow::{Result, Context, anyhow};
use std::path::PathBuf;
use std::fs::{self, File};
use fs2::FileExt;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit},
    Aes256Gcm, Nonce,
};
use sha2::{Sha256, Digest};
use std::collections::HashMap;

pub trait Vault: Send + Sync {
    fn get(&self, profile: &str, key: &str) -> Result<String>;
    fn set(&self, profile: &str, key: &str, secret: &str) -> Result<()>;
    fn delete(&self, profile: &str, key: &str) -> Result<()>;
    fn clear(&self, profile: &str) -> Result<()>;
    
    /// Global lock for multi-process synchronization (e.g. during refresh)
    fn lock(&self, profile: &str) -> Result<Box<dyn std::any::Any + Send>>;
}

pub struct MultiVault {
    seal_path: PathBuf,
    master_key: Vec<u8>,
    lock_file: std::sync::Arc<std::sync::Mutex<File>>,
}

impl MultiVault {
    pub fn new(seal_path: PathBuf, master_key_str: &str) -> Result<Self> {
        let mut hasher = Sha256::new();
        hasher.update(master_key_str.as_bytes());
        let key = hasher.finalize().to_vec();

        let lock_path = seal_path.with_extension("lock");
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let lock_file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)?;

        Ok(Self {
            seal_path,
            master_key: key,
            lock_file: std::sync::Arc::new(std::sync::Mutex::new(lock_file)),
        })
    }

    fn get_full_key(&self, profile: &str, key: &str) -> String {
        format!("{}:{}", profile, key)
    }

    fn read_seal(&self) -> Result<HashMap<String, String>> {
        if !self.seal_path.exists() {
            return Ok(HashMap::new());
        }

        let encrypted_data = fs::read(&self.seal_path).context("Failed to read vault file")?;
        if encrypted_data.is_empty() {
            return Ok(HashMap::new());
        }

        let decrypted_data = match self.decrypt_aes_gcm(&encrypted_data) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(target: "sys", error = %e, "Vault decryption failed. This usually means the master key (fingerprint) has changed or the file is corrupted.");
                return Err(anyhow!("Vault decryption failed: {}", e));
            }
        };
        
        let data: HashMap<String, String> = serde_json::from_slice(&decrypted_data).context("Failed to parse vault JSON")?;
        Ok(data)
    }

    fn write_seal(&self, data: &HashMap<String, String>) -> Result<()> {
        let json_data = serde_json::to_vec(data)?;
        let encrypted_data = self.encrypt_aes_gcm(&json_data).context("Vault encryption failed")?;
        
        if let Some(parent) = self.seal_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        // Write to temp file then rename for atomicity (best effort)
        let tmp_path = self.seal_path.with_extension("tmp");
        fs::write(&tmp_path, encrypted_data)?;
        fs::rename(tmp_path, &self.seal_path)?;
        
        Ok(())
    }

    fn encrypt_aes_gcm(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(&self.master_key).map_err(|e| anyhow!(e))?;
        let nonce = Aes256Gcm::generate_nonce(&mut rand::thread_rng());
        let ciphertext = cipher.encrypt(&nonce, plaintext).map_err(|e| anyhow!(e))?;
        
        let mut result = nonce.to_vec();
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    fn decrypt_aes_gcm(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() < 12 {
            return Err(anyhow!("Ciphertext too short"));
        }
        let (nonce_bytes, actual_ciphertext) = ciphertext.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let cipher = Aes256Gcm::new_from_slice(&self.master_key).map_err(|e| anyhow!(e))?;
        
        cipher.decrypt(nonce, actual_ciphertext).map_err(|e| anyhow!(e))
    }
}

impl Vault for MultiVault {
    fn set(&self, profile: &str, key: &str, secret: &str) -> Result<()> {
        let f = self.lock_file.lock().map_err(|e| anyhow!("Mutex error: {}", e))?;
        f.lock_exclusive()?;
        
        let full_key = self.get_full_key(profile, key);
        let mut data = self.read_seal()?;
        data.insert(full_key, secret.to_string());
        self.write_seal(&data)?;
        
        f.unlock()?;
        Ok(())
    }

    fn get(&self, profile: &str, key: &str) -> Result<String> {
        let f = self.lock_file.lock().map_err(|e| anyhow!("Mutex error: {}", e))?;
        f.lock_shared()?;

        let full_key = self.get_full_key(profile, key);
        let data = self.read_seal()?;
        
        f.unlock()?;
        
        data.get(&full_key)
            .cloned()
            .context("Secret not found in vault")
    }

    fn delete(&self, profile: &str, key: &str) -> Result<()> {
        let f = self.lock_file.lock().map_err(|e| anyhow!("Mutex error: {}", e))?;
        f.lock_exclusive()?;

        let full_key = self.get_full_key(profile, key);
        let mut data = self.read_seal()?;
        data.remove(&full_key);
        self.write_seal(&data)?;
        
        f.unlock()?;
        Ok(())
    }

    fn clear(&self, profile: &str) -> Result<()> {
        let f = self.lock_file.lock().map_err(|e| anyhow!("Mutex error: {}", e))?;
        f.lock_exclusive()?;

        let prefix = format!("{}:", profile);
        let mut data = self.read_seal()?;
        data.retain(|k, _| !k.starts_with(&prefix));
        self.write_seal(&data)?;
        
        f.unlock()?;
        Ok(())
    }

    fn lock(&self, _profile: &str) -> Result<Box<dyn std::any::Any + Send>> {
        let f = self.lock_file.lock().map_err(|e| anyhow!("Mutex error: {}", e))?;
        f.lock_exclusive()?;
        
        // Wrap the file in a custom guard to release flock on drop
        // But wait, returned guard should keep the flock alive.
        // Returning the MutexGuard alone isn't easy due to lifetimes.
        // We'll return an Arc clone of the Mutex and a guard that holds it.
        
        struct LockGuard {
            lock_file: std::sync::Arc<std::sync::Mutex<File>>,
        }
        
        impl Drop for LockGuard {
            fn drop(&mut self) {
                if let Ok(f) = self.lock_file.lock() {
                    let _ = f.unlock();
                }
            }
        }
        
        Ok(Box::new(LockGuard { lock_file: self.lock_file.clone() }))
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

        vault.set("default", "app_secret", "secret123")?;
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

    #[test]
    fn test_nested_lock_deadlock() -> Result<()> {
        let tmp = tempdir()?;
        let seal_path = tmp.path().join(".seal");
        let vault = MultiVault::new(seal_path, "test_key")?;

        // Acquiring exclusive lock
        let _guard = vault.lock("default")?;
        
        // Attempting nested get (should not deadlock)
        // If it deadlocks, this test will hang.
        let result = vault.get("default", "nonexistent");
        
        assert!(result.is_err());
        Ok(())
    }
}
