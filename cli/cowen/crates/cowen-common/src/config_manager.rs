use crate::{CowenResult, CowenError};
use anyhow::{Result, Context};
use std::fs;
use std::path::PathBuf;
use crate::config::{AppConfig, StorageConfig, Config};
use tokio::sync::OnceCell;
use std::sync::Arc;
use crate::vault::Vault;
use crate::events::event_bus;
use serde_yaml;

pub trait ConfigValidator: Send + Sync {
    fn validate_load(&self, profile: &str, config: &Config, is_distributed: bool, exists: bool) -> CowenResult<()>;
    fn validate_save(&self, profile: &str, config: &Config, is_distributed: bool) -> CowenResult<()>;
}

#[derive(Clone)]
pub struct ConfigManager {
    pub app_dir: PathBuf,
    vault: OnceCell<Arc<dyn Vault>>,
    validator: OnceCell<Arc<dyn ConfigValidator>>,
}

impl ConfigManager {
    pub fn new() -> CowenResult<Self> {
        let app_dir = crate::config::get_app_dir();
        if !app_dir.exists() {
            fs::create_dir_all(&app_dir).context("Failed to create app directory")?;
        }
        Ok(Self { 
            app_dir,
            vault: OnceCell::new(),
            validator: OnceCell::new(),
        })
    }

    pub fn set_vault(&self, vault: Arc<dyn Vault>) -> CowenResult<()> {
        self.vault.set(vault).map_err(|_| CowenError::Internal("Vault already set".to_string()))
    }

    pub fn get_vault(&self) -> Option<Arc<dyn Vault>> {
        self.vault.get().cloned()
    }

    pub fn set_validator(&self, validator: Arc<dyn ConfigValidator>) -> CowenResult<()> {
        self.validator.set(validator).map_err(|_| CowenError::Internal("Validator already set".to_string()))
    }

    pub async fn find_free_port(&self) -> u16 {
        use rand::Rng;
        let start_range = std::env::var("COWEN_PORT_RANGE_START")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(16000);
            
        // Add random jitter to reduce collision risk in parallel tests
        let jitter = rand::thread_rng().gen_range(0..200);
        let start = start_range + jitter;

        for port in start..(start_range + 1500) {
            if tokio::net::TcpListener::bind(("127.0.0.1", port)).await.is_ok() {
                return port;
            }
        }
        0
    }

    pub async fn exists(&self, profile: &str) -> bool {
        if let Some(vault) = self.vault.get() {
            if let Ok(profiles) = vault.list_all_profiles().await {
                let profiles: Vec<String> = profiles;
                if profiles.contains(&profile.to_string()) { return true; }
            }
        }
        self.get_profile_path(profile).exists()
    }

    pub async fn get_next_profile_name(&self) -> CowenResult<String> {
        let profiles: Vec<String> = self.list_profiles().await?;
        let mut i = 1;
        loop {
            let name = format!("p{}", i);
            if !profiles.contains(&name) { return Ok(name); }
            i += 1;
        }
    }

    pub async fn load(&self, profile: &str) -> CowenResult<Config> {
        let app_cfg = self.load_app_config().await?;
        let is_db_mode = self.is_distributed_storage(&app_cfg);

        // 1. Try Vault first (The Single Source of Truth)
        if let Some(vault) = self.vault.get() {
            tracing::debug!(target: "sys", profile = %profile, "Attempting to load manifest from Vault");
            match vault.get_config_full(profile, "system:manifest").await {
                Ok(item) => {
                    tracing::info!(target: "sys", profile = %profile, version = %item.version, "Manifest loaded from Vault");
                    match serde_yaml::from_str::<Config>(&item.value) {
                        Ok(mut config) => {
                            eprintln!("DEBUG: ConfigManager::load profile='{}' raw_yaml='{}' loaded_mode='{:?}'", profile, item.value, config.app_mode);
                            config.version = item.version;
                            let app_key = config.app_key.trim();
                            let global_profile = format!("app:{}", app_key);

                        if let Ok(s) = vault.get_secret(profile, "app_secret").await { 
                            let s: String = s;
                            if !s.is_empty() { config.app_secret = s; } 
                        }
                        else if let Ok(s) = vault.get_secret(&global_profile, "app_secret").await { config.app_secret = s; }

                        if let Ok(cert) = vault.get_secret(profile, "certificate").await { config.certificate = cert; }
                        else if let Ok(cert) = vault.get_secret(&global_profile, "certificate").await { config.certificate = cert; }

                        if let Ok(ek) = vault.get_secret(profile, "encrypt_key").await { config.encrypt_key = ek; }
                        else if let Ok(ek) = vault.get_secret(&global_profile, "encrypt_key").await { config.encrypt_key = ek; }

                        // Return here if found in Vault
                        if let Some(validator) = self.validator.get() {
                            validator.validate_load(profile, &config, is_db_mode, true)?;
                        }
                        let mut config = config;
                        config.apply_env_overrides();
                        return Ok(config);
                    },
                    Err(e) => {
                        tracing::error!(target: "sys", profile = %profile, error = %e, raw = %item.value, "Failed to parse manifest from Vault");
                    }
                }
                },
                Err(e) => {
                    tracing::debug!(target: "sys", profile = %profile, error = %e, "Manifest not found in Vault or Vault error");
                }
            }
        }

        // 2. Fallback to Local + Sync Version
        let (mut config, exists) = self.load_local_profile_with_status(profile).await?;
        
        if let Some(validator) = self.validator.get() {
            validator.validate_load(profile, &config, is_db_mode, exists)?;
        }

        config.apply_env_overrides();
        
        if let Ok(val) = std::env::var("COWEN_EXCLUSIVE") {
            config.exclusive = Some(val == "true" || val == "1");
        }

        Ok(config)
    }

    async fn load_local_profile_with_status(&self, profile: &str) -> CowenResult<(Config, bool)> {
        let profile_path = self.get_profile_path(profile);
        if profile_path.exists() {
            let content = fs::read_to_string(&profile_path)?;
            let mut config: Config = serde_yaml::from_str(&content)?;
            
            // Critical Fix: Version 1 by default for local files that are being synced for the first time
            if config.version == 0 { config.version = 1; }

            if let Some(vault) = self.vault.get() {
                // Critical: Sync version even when loading from local to avoid race conditions
                if let Ok(item) = vault.get_config_full(profile, "system:manifest").await {
                    config.version = item.version;
                }

                let app_key = config.app_key.trim();
                let global_profile = format!("app:{}", app_key);
                
                if let Ok(s) = vault.get_secret(&global_profile, "app_secret").await { config.app_secret = s; }
                else if let Ok(s) = vault.get_secret(profile, "app_secret").await { 
                    let s: String = s;
                    if !s.is_empty() { config.app_secret = s; } 
                }

                if let Ok(cert) = vault.get_secret(&global_profile, "certificate").await { config.certificate = cert; }
                else if let Ok(cert) = vault.get_secret(profile, "certificate").await { config.certificate = cert; }

                if let Ok(ek) = vault.get_secret(&global_profile, "encrypt_key").await { config.encrypt_key = ek; }
                else if let Ok(ek) = vault.get_secret(profile, "encrypt_key").await { config.encrypt_key = ek; }
            }
            return Ok((config, true));
        }
        Ok((Config::default_with_profile(profile), false))
    }

    pub async fn save(&self, profile: &str, config: &mut Config) -> CowenResult<()> {
        let app_cfg = self.load_app_config().await?;
        let is_db_mode = self.is_distributed_storage(&app_cfg);

        if let Some(validator) = self.validator.get() {
            validator.validate_save(profile, config, is_db_mode)?;
        }

        if let Some(vault) = self.vault.get() {
            let app_key = config.app_key.trim();
            let global_profile = format!("app:{}", app_key);

            if !config.app_secret.is_empty() {
                let _ = vault.set_secret(&global_profile, "app_secret", &config.app_secret).await;
            }
            if !config.certificate.is_empty() {
                let _ = vault.set_secret(&global_profile, "certificate", &config.certificate).await;
            }
            if !config.encrypt_key.is_empty() {
                let _ = vault.set_secret(&global_profile, "encrypt_key", &config.encrypt_key).await;
            }
            
            let manifest = serde_yaml::to_string(config)?;
            if is_db_mode && config.version > 0 {
                // If version is non-zero, we MUST use conditional update
                vault.set_config_conditional(profile, "system:manifest", &manifest, config.version).await?;
            } else {
                // For version 0, it might be a truly new profile or a legacy fallback
                vault.set_config(profile, "system:manifest", &manifest).await?;
            }
            event_bus().publish(crate::events::GlobalEvent::ConfigChanged { 
                profile: profile.to_string(), 
                key: "system:manifest".to_string() 
            });
            config.version += 1;
        }

        if !is_db_mode {
            let path = self.app_dir.join(format!("{}.yaml", profile));
            let content = serde_yaml::to_string(config)?;
            fs::write(path, content)?;
        }

        Ok(())
    }

    pub fn get_profile_path(&self, profile: &str) -> PathBuf {
        self.app_dir.join(format!("{}.yaml", profile))
    }

    pub fn is_distributed_storage(&self, app_cfg: &AppConfig) -> bool {
        match app_cfg.storage.store.as_str() {
            "local" => false,
            "innerdb" => {
                if let Some(url) = &app_cfg.storage.db_url {
                    // Check for default literal or expanded local paths
                    if url == "innerdb" { return false; }
                    
                    let db_path = self.app_dir.join("cowen.db");
                    let expected_sqlite = format!("sqlite://{}", db_path.to_string_lossy());
                    let expected_innerdb = format!("innerdb://{}", db_path.to_string_lossy());
                    
                    // Normalize url for comparison (e.g. handle relative paths)
                    let normalized_url = if url.starts_with("sqlite://") || url.starts_with("innerdb://") {
                        let scheme = if url.starts_with("sqlite://") { "sqlite://" } else { "innerdb://" };
                        let path_part = &url[scheme.len()..];
                        let path = std::path::Path::new(path_part.split('?').next().unwrap_or(path_part));
                        if path.is_relative() {
                             if let Ok(cwd) = std::env::current_dir() {
                                 format!("{}{}", scheme, cwd.join(path).to_string_lossy())
                             } else {
                                 url.to_string()
                             }
                        } else {
                             url.to_string()
                        }
                    } else {
                        url.to_string()
                    };

                    let res = normalized_url != expected_sqlite && normalized_url != expected_innerdb 
                        && !normalized_url.starts_with(&format!("{}?", expected_sqlite))
                        && !normalized_url.starts_with(&format!("{}?", expected_innerdb));
                    
                    tracing::debug!(target: "sys", "is_distributed_storage: url={}, normalized_url={}, expected_innerdb={}, res={}", url, normalized_url, expected_innerdb, res);
                    res
                } else {
                    false
                }
            },
            _ => true,
        }
    }

    pub async fn check_for_updates(&self, profile: &str, current_version: u64) -> CowenResult<bool> {
        let app_cfg = self.load_app_config().await?;
        if !self.is_distributed_storage(&app_cfg) {
            return Ok(false);
        }

        if let Some(vault) = self.vault.get() {
            if let Ok((remote_version, _)) = vault.get_config_metadata(profile, "system:manifest").await {
                return Ok(remote_version > current_version);
            }
        }
        Ok(false)
    }

    pub async fn load_app_config(&self) -> CowenResult<AppConfig> {
        let path = self.app_dir.join("app.yaml");
        let mut config = if !path.exists() {
            let seal_dir = self.app_dir.join(".seal");
            let mut use_local = seal_dir.exists();
            if !use_local {
                if let Ok(profiles) = self.list_profiles().await {
                    for p in profiles {
                        let p_path = self.app_dir.join(format!("{}.yaml", p));
                        if p_path.exists() {
                            if let Ok(c) = fs::read_to_string(&p_path) {
                                if c.contains("store: local") { use_local = true; break; }
                            }
                        }
                    }
                }
            }

            let app_config = if use_local {
                AppConfig { storage: StorageConfig { store: "local".to_string(), ..Default::default() } }
            } else {
                let db_path = self.app_dir.join("cowen.db");
                let db_url = format!("innerdb://{}", db_path.to_string_lossy());
                AppConfig { storage: StorageConfig { store: "innerdb".to_string(), db_url: Some(db_url), ..Default::default() } }
            };

            let _ = self.save_app_config(&app_config).await;
            app_config
        } else {
            let content = fs::read_to_string(path)?;
            serde_yaml::from_str(&content)?
        };

        // Environment variables ALWAYS override, even after loading from file
        if let Ok(store) = std::env::var("COWEN_STORE_TYPE") { config.storage.store = store; }
        if let Ok(db_url) = std::env::var("COWEN_DB_URL") { config.storage.db_url = Some(db_url); }
        if let Ok(cache) = std::env::var("COWEN_CACHE_TYPE") { config.storage.cache = cache; }
        if let Ok(cache_url) = std::env::var("COWEN_CACHE_URL") { config.storage.cache_url = Some(cache_url); }

        Ok(config)
    }

    pub async fn save_app_config(&self, config: &AppConfig) -> CowenResult<()> {
        let path = self.app_dir.join("app.yaml");
        let content = serde_yaml::to_string(config)?;
        fs::write(path, content)?;
        event_bus().publish(crate::events::GlobalEvent::ConfigChanged { 
            profile: "system".to_string(), 
            key: "app".to_string() 
        });
        Ok(())
    }

    pub fn get_default_profile(&self) -> String {
        let path = self.app_dir.join("current_profile");
        if let Ok(p) = fs::read_to_string(&path) { 
            p.trim().to_string() 
        } else {
            "default".to_string()
        }
    }

    pub fn set_default_profile(&self, profile: &str) -> CowenResult<()> {
        let path = self.app_dir.join("current_profile");
        fs::write(path, profile)?;
        Ok(())
    }

    pub async fn list_profiles(&self) -> CowenResult<Vec<String>> {
        let mut profiles = std::collections::HashSet::new();
        if let Some(vault) = self.vault.get() {
            if let Ok(remote_profiles) = vault.list_all_profiles().await {
                let remote_profiles: Vec<String> = remote_profiles;
                for p in remote_profiles {
                    if !p.starts_with("app:") {
                        profiles.insert(p);
                    }
                }
            }
        }
        if let Ok(entries) = fs::read_dir(&self.app_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map(|s| s == "yaml").unwrap_or(false) {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        if !name.contains("_openapi") && name != "app" {
                            profiles.insert(name.to_string());
                        }
                    }
                }
            }
        }
        let mut res: Vec<String> = profiles.into_iter().collect();
        res.sort();
        Ok(res)
    }

    pub async fn rename(&self, old_name: &str, new_name: &str) -> CowenResult<()> {
        let old_path = self.get_profile_path(old_name);
        let new_path = self.get_profile_path(new_name);

        if old_path.exists() {
            fs::rename(old_path, new_path)?;
        }

        if let Some(vault) = self.vault.get() {
            vault.rename_profile(old_name, new_name).await?;
        }

        event_bus().publish(crate::events::GlobalEvent::ProfileRenamed { 
            old: old_name.to_string(), 
            new: new_name.to_string() 
        });

        Ok(())
    }

    pub async fn delete(&self, profile: &str) -> CowenResult<()> {
        
        let path = self.get_profile_path(profile);
        if path.exists() {
            fs::remove_file(path)?;
        }

        if let Some(vault) = self.vault.get() {
            vault.clear_profile(profile).await?;
        }

        event_bus().publish(crate::events::GlobalEvent::ConfigChanged { 
            profile: profile.to_string(), 
            key: "system:manifest".to_string() 
        });

        Ok(())
    }
}
