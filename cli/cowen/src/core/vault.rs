use std::path::PathBuf;
use std::fs::{File, OpenOptions};
use std::collections::HashMap;
use std::io::{Read, Write};
use anyhow::{Result, Context};
use crate::core::security;
use fs2::FileExt;
use std::sync::Mutex;

pub trait Vault: Send + Sync {
    fn get(&self, profile: &str, key: &str) -> Result<String>;
    fn set(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    fn delete(&self, profile: &str, key: &str) -> Result<()>;
    fn clear(&self, profile: &str) -> Result<()>;
    fn lock(&self, profile: &str) -> Result<File>;
}

pub struct MultiVault {
    path: PathBuf,
    key: [u8; 32],
    _cache: Mutex<HashMap<String, HashMap<String, String>>>,
    storage_lock_path: PathBuf,
    business_lock_path: PathBuf,
}

impl MultiVault {
    pub fn new(path: PathBuf, fingerprint: &str) -> Result<Self> {
        let key = security::derive_key(fingerprint);
        // DIFFERENT LOCK FILES to prevent deadlock between IO protection and business coordination
        let storage_lock_path = path.with_extension("lock");
        let business_lock_path = path.with_extension("bizlock");
        
        Ok(Self {
            path,
            key,
            _cache: Mutex::new(HashMap::new()),
            storage_lock_path,
            business_lock_path,
        })
    }

    fn with_storage_lock<F, R>(&self, f: F) -> Result<R> 
    where F: FnOnce() -> Result<R> 
    {
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&self.storage_lock_path)?;
            
        lock_file.lock_exclusive().context("Failed to acquire vault storage lock")?;
        let res = f();
        let _ = lock_file.unlock();
        res
    }

    fn load_all(&self) -> Result<HashMap<String, HashMap<String, String>>> {
        if !self.path.exists() {
            return Ok(HashMap::new());
        }

        let mut file = File::open(&self.path)?;
        let mut encrypted = Vec::new();
        file.read_to_end(&mut encrypted)?;

        if encrypted.is_empty() {
            return Ok(HashMap::new());
        }

        let decrypted = match security::decrypt(&encrypted, &self.key) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(target: "sys", error = %e, "Vault decryption failed. Data might be from an incompatible version. Starting fresh.");
                return Ok(HashMap::new());
            }
        };

        match serde_json::from_slice::<HashMap<String, HashMap<String, String>>>(&decrypted) {
            Ok(data) => Ok(data),
            Err(e) => {
                tracing::warn!(target: "sys", error = %e, "Vault parsing failed. Starting fresh.");
                Ok(HashMap::new())
            }
        }
    }

    fn save_all(&self, data: &HashMap<String, HashMap<String, String>>) -> Result<()> {
        let json = serde_json::to_vec(data)?;
        let encrypted = security::encrypt(&json, &self.key)?;
        
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)?;
            
        file.write_all(&encrypted)?;
        Ok(())
    }
}

impl Vault for MultiVault {
    fn get(&self, profile: &str, key: &str) -> Result<String> {
        self.with_storage_lock(|| {
            let data = self.load_all()?;
            data.get(profile)
                .and_then(|p| p.get(key))
                .cloned()
                .context(format!("Key '{}' not found in profile '{}'", key, profile))
        })
    }

    fn set(&self, profile: &str, key: &str, value: &str) -> Result<()> {
        self.with_storage_lock(|| {
            let mut data = self.load_all()?;
            data.entry(profile.to_string())
                .or_insert_with(HashMap::new)
                .insert(key.to_string(), value.to_string());
            self.save_all(&data)
        })
    }

    fn delete(&self, profile: &str, key: &str) -> Result<()> {
        self.with_storage_lock(|| {
            let mut data = self.load_all()?;
            if let Some(p) = data.get_mut(profile) {
                p.remove(key);
                self.save_all(&data)?;
            }
            Ok(())
        })
    }

    fn clear(&self, profile: &str) -> Result<()> {
        self.with_storage_lock(|| {
            let mut data = self.load_all()?;
            data.remove(profile);
            self.save_all(&data)
        })
    }

    fn lock(&self, _profile: &str) -> Result<File> {
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&self.business_lock_path)?;
        lock_file.lock_exclusive().context("Failed to acquire exclusive business vault lock")?;
        Ok(lock_file)
    }
}
