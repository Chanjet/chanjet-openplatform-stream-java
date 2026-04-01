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
use std::sync::Mutex;

pub trait Vault: Send + Sync {
    fn get(&self, profile: &str, key: &str) -> Result<String>;
    fn set(&self, profile: &str, key: &str, secret: &str) -> Result<()>;
    fn delete(&self, profile: &str, key: &str) -> Result<()>;
    fn clear(&self, profile: &str) -> Result<()>;
    fn lock(&self, profile: &str) -> Result<Box<dyn std::any::Any + Send>>;
}

pub struct MultiVault {
    seal_path: PathBuf,
    master_key: Vec<u8>,
    // The internal Mutex is only for THREAD safety within one process.
    // Process safety is handled via flock on the .lock file.
    data_mutex: Mutex<()>,
}

impl MultiVault {
    pub fn new(seal_path: PathBuf, master_pwd: &str) -> Result<Self> {
        let mut hasher = Sha256::new();
        hasher.update(master_pwd.as_bytes());
        let master_key = hasher.finalize().to_vec();

        Ok(Self {
            seal_path,
            master_key,
            data_mutex: Mutex::new(()),
        })
    }

    fn with_flock<F, T>(&self, shared: bool, f: F) -> Result<T> 
    where F: FnOnce() -> Result<T> {
        let _thread_guard = self.data_mutex.lock().map_err(|e| anyhow!("Mutex error: {}", e))?;
        
        let lock_path = self.seal_path.with_extension("lock");
        let lock_file = fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&lock_path)
            .context("Failed to open vault lock file")?;

        if shared {
            lock_file.lock_shared().context("Failed to acquire shared vault lock")?;
        } else {
            lock_file.lock_exclusive().context("Failed to acquire exclusive vault lock")?;
        }

        let result = f();
        
        let _ = lock_file.unlock();
        result
    }

    fn encrypt_aes_gcm(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&self.master_key);
        let cipher = Aes256Gcm::new(key);
        let nonce = Aes256Gcm::generate_nonce(&mut aes_gcm::aead::OsRng);
        let ciphertext = cipher.encrypt(&nonce, plaintext)
            .map_err(|e| anyhow!("Encryption error: {}", e))?;
        
        let mut result = nonce.to_vec();
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    fn decrypt_aes_gcm(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
        if encrypted.len() < 12 {
            return Err(anyhow!("Invalid encrypted data"));
        }
        let (nonce_bytes, ciphertext) = encrypted.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&self.master_key);
        let cipher = Aes256Gcm::new(key);
        
        cipher.decrypt(nonce, ciphertext)
            .map_err(|e| anyhow!("Decryption error: {}", e))
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
                tracing::error!(target: "sys", error = %e, "Vault decryption failed. Master key mismatch or file corruption.");
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
        
        let tmp_path = self.seal_path.with_extension("tmp");
        fs::write(&tmp_path, encrypted_data)?;
        fs::rename(tmp_path, &self.seal_path)?;
        Ok(())
    }
}

impl Vault for MultiVault {
    fn get(&self, profile: &str, key: &str) -> Result<String> {
        self.with_flock(true, || {
            let data = self.read_seal()?;
            let full_key = format!("{}:{}", profile, key);
            data.get(&full_key).cloned().ok_or_else(|| anyhow!("Key not found in vault: {}", key))
        })
    }

    fn set(&self, profile: &str, key: &str, secret: &str) -> Result<()> {
        self.with_flock(false, || {
            let mut data = self.read_seal()?;
            let full_key = format!("{}:{}", profile, key);
            data.insert(full_key, secret.to_string());
            self.write_seal(&data)
        })
    }

    fn delete(&self, profile: &str, key: &str) -> Result<()> {
        self.with_flock(false, || {
            let mut data = self.read_seal()?;
            let full_key = format!("{}:{}", profile, key);
            data.remove(&full_key);
            self.write_seal(&data)
        })
    }

    fn clear(&self, profile: &str) -> Result<()> {
        self.with_flock(false, || {
            let prefix = format!("{}:", profile);
            let mut data = self.read_seal()?;
            data.retain(|k, _| !k.starts_with(&prefix));
            self.write_seal(&data)
        })
    }

    fn lock(&self, _profile: &str) -> Result<Box<dyn std::any::Any + Send>> {
        // Use a SEPARATE lock file for business-level global locks
        let biz_lock_path = self.seal_path.with_extension("bizlock");
        let f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&biz_lock_path)
            .context("Failed to open business lock file")?;
        
        f.lock_exclusive()?;
        
        struct GlobalLockGuard {
            file: File,
        }
        
        impl Drop for GlobalLockGuard {
            fn drop(&mut self) {
                let _ = self.file.unlock();
            }
        }
        
        Ok(Box::new(GlobalLockGuard { file: f }))
    }
}
