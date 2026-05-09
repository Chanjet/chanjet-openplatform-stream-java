use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use std::fs;
use std::sync::Arc;
use tokio::sync::OnceCell;
use crate::config::{Config, AppConfig, StorageConfig};
use crate::vault::Vault;
use crate::events::event_bus;

pub trait ConfigValidator: Send + Sync {
    fn validate_load(&self, profile: &str, config: &Config, is_distributed: bool, exists: bool) -> Result<()>;
    fn validate_save(&self, profile: &str, config: &Config, is_distributed: bool) -> Result<()>;
}

#[derive(Clone)]
pub struct ConfigManager {
    app_dir: PathBuf,
    vault: OnceCell<Arc<dyn Vault>>,
    validator: OnceCell<Arc<dyn ConfigValidator>>,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
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

    pub fn set_vault(&self, vault: Arc<dyn Vault>) {
        let _ = self.vault.set(vault);
    }

    pub fn set_validator(&self, validator: Arc<dyn ConfigValidator>) {
        let _ = self.validator.set(validator);
    }

    pub fn get_vault(&self) -> Option<Arc<dyn Vault>> {
        self.vault.get().cloned()
    }

    pub async fn exists(&self, profile: &str) -> bool {
        let path = self.app_dir.join(format!("{}.yaml", profile));
        if path.exists() {
            return true;
        }
        if let Some(vault) = self.vault.get() {
            if let Ok(configs) = vault.list_configs(profile).await {
                if !configs.is_empty() { return true; }
            }
        }
        false
    }

    pub fn get_profile_path(&self, profile: &str) -> PathBuf {
        self.app_dir.join(format!("{}.yaml", profile))
    }

    pub fn is_distributed_storage(&self, app_cfg: &AppConfig) -> bool {
        match app_cfg.storage.store.as_str() {
            "local" => false,
            "innerdb" => {
                if let Some(url) = &app_cfg.storage.db_url {
                    if url == "innerdb" { return false; }
                    let db_path = self.app_dir.join("cowen.db");
                    let expected_sqlite = format!("sqlite://{}", db_path.to_string_lossy());
                    let expected_innerdb = format!("innerdb://{}", db_path.to_string_lossy());
                    url != &expected_sqlite && url != &expected_innerdb 
                        && !url.starts_with(&format!("{}?", expected_sqlite))
                        && !url.starts_with(&format!("{}?", expected_innerdb))
                } else {
                    false
                }
            },
            _ => true,
        }
    }

    pub async fn check_for_updates(&self, profile: &str, current_version: u64) -> Result<bool> {
        let app_cfg = self.load_app_config().await?;
        if !self.is_distributed_storage(&app_cfg) {
            return Ok(false);
        }

        if let Some(vault) = self.vault.get() {
            if let Ok((version, _)) = vault.get_config_metadata(profile, "system:manifest").await {
                return Ok(version != current_version);
            }
        }
        Ok(false)
    }

    pub async fn load(&self, profile: &str) -> Result<Config> {
        let app_cfg = self.load_app_config().await?;
        let is_db_mode = self.is_distributed_storage(&app_cfg);

        let (config, _exists) = if let Some(vault) = self.vault.get() {
            if is_db_mode {
                if let Ok(item) = vault.get_config_full(profile, "system:manifest").await {
                    match serde_yaml::from_str::<Config>(&item.value) {
                        Ok(mut config) => {
                            config.version = item.version;
                            let app_key = config.app_key.trim();
                            let global_profile = format!("app:{}", app_key);

                            if let Ok(s) = vault.get_secret(profile, "app_secret").await { config.app_secret = s; }
                            else if let Ok(s) = vault.get_secret(&global_profile, "app_secret").await { config.app_secret = s; }

                            if let Ok(cert) = vault.get_secret(profile, "certificate").await { config.certificate = cert; }
                            else if let Ok(cert) = vault.get_secret(&global_profile, "certificate").await { config.certificate = cert; }

                            if let Ok(ek) = vault.get_secret(profile, "encrypt_key").await { config.encrypt_key = ek; }
                            else if let Ok(ek) = vault.get_secret(&global_profile, "encrypt_key").await { config.encrypt_key = ek; }

                            (config, true)
                        },
                        Err(e) => {
                            tracing::error!(target: "sys", profile = %profile, error = %e, raw = %item.value, "Failed to parse manifest from Vault");
                            return Err(anyhow::anyhow!("Failed to parse manifest from Vault: {}", e));
                        }
                    }
                } else {
                    self.load_local_profile_with_status(profile).await?
                }
            } else {
                self.load_local_profile_with_status(profile).await?
            }
        } else {
            self.load_local_profile_with_status(profile).await?
        };

        if let Some(validator) = self.validator.get() {
            validator.validate_load(profile, &config, is_db_mode, _exists)?;
        }

        let mut config = config;
        config.apply_env_overrides();
        
        if let Ok(val) = std::env::var("COWEN_EXCLUSIVE") {
            config.exclusive = Some(val == "true" || val == "1");
        }

        Ok(config)
    }

    async fn load_local_profile_with_status(&self, profile: &str) -> Result<(Config, bool)> {
        let profile_path = self.get_profile_path(profile);
        if profile_path.exists() {
            let content = fs::read_to_string(&profile_path)?;
            let mut config: Config = serde_yaml::from_str(&content)?;
            if let Some(vault) = self.vault.get() {
                let app_key = config.app_key.trim();
                let global_profile = format!("app:{}", app_key);
                
                if let Ok(s) = vault.get_secret(&global_profile, "app_secret").await { config.app_secret = s; }
                else if let Ok(s) = vault.get_secret(profile, "app_secret").await { config.app_secret = s; }

                if let Ok(cert) = vault.get_secret(&global_profile, "certificate").await { config.certificate = cert; }
                else if let Ok(cert) = vault.get_secret(profile, "certificate").await { config.certificate = cert; }

                if let Ok(ek) = vault.get_secret(&global_profile, "encrypt_key").await { config.encrypt_key = ek; }
                else if let Ok(ek) = vault.get_secret(profile, "encrypt_key").await { config.encrypt_key = ek; }
            }
            return Ok((config, true));
        }
        Ok((Config::default_with_profile(profile), false))
    }

    pub async fn save(&self, profile: &str, config: &Config) -> Result<()> {
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
            if is_db_mode {
                vault.set_config_conditional(profile, "system:manifest", &manifest, config.version).await?;
            } else {
                let _ = vault.set_config(profile, "system:manifest", &manifest).await;
            }
            event_bus().publish(crate::events::GlobalEvent::ConfigChanged { 
                profile: profile.to_string(), 
                key: "system:manifest".to_string() 
            });
        }

        if !is_db_mode {
            let path = self.app_dir.join(format!("{}.yaml", profile));
            let content = serde_yaml::to_string(config)?;
            fs::write(path, content)?;
        }
        Ok(())
    }

    pub async fn load_app_config(&self) -> Result<AppConfig> {
        let path = self.app_dir.join("app.yaml");
        if !path.exists() {
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

            let mut app_config = if use_local {
                AppConfig { storage: StorageConfig { store: "local".to_string(), ..Default::default() } }
            } else {
                let db_path = self.app_dir.join("cowen.db");
                let db_url = format!("innerdb://{}", db_path.to_string_lossy());
                AppConfig { storage: StorageConfig { store: "innerdb".to_string(), db_url: Some(db_url), ..Default::default() } }
            };

            if let Ok(store) = std::env::var("COWEN_STORE_TYPE") { app_config.storage.store = store; }
            if let Ok(db_url) = std::env::var("COWEN_DB_URL") { app_config.storage.db_url = Some(db_url); }
            if let Ok(cache) = std::env::var("COWEN_CACHE_TYPE") { app_config.storage.cache = cache; }
            if let Ok(cache_url) = std::env::var("COWEN_CACHE_URL") { app_config.storage.cache_url = Some(cache_url); }

            let _ = self.save_app_config(&app_config).await;
            return Ok(app_config);
        }
        let content = fs::read_to_string(path)?;
        let mut config: AppConfig = serde_yaml::from_str(&content)?;

        if let Ok(store) = std::env::var("COWEN_STORE_TYPE") { config.storage.store = store; }
        if let Ok(db_url) = std::env::var("COWEN_DB_URL") { config.storage.db_url = Some(db_url); }
        if let Ok(cache) = std::env::var("COWEN_CACHE_TYPE") { config.storage.cache = cache; }
        if let Ok(cache_url) = std::env::var("COWEN_CACHE_URL") { config.storage.cache_url = Some(cache_url); }

        Ok(config)
    }

    pub async fn save_app_config(&self, config: &AppConfig) -> Result<()> {
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

    pub fn set_default_profile(&self, profile: &str) -> Result<()> {
        let path = self.app_dir.join("current_profile");
        fs::write(path, profile)?;
        Ok(())
    }

    pub async fn list_profiles(&self) -> Result<Vec<String>> {
        let mut profiles = std::collections::HashSet::new();
        if let Some(vault) = self.vault.get() {
            if let Ok(remote_profiles) = vault.list_all_profiles().await {
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

    pub async fn delete(&self, profile: &str) -> Result<()> {
        let path = self.get_profile_path(profile);
        if path.exists() {
            fs::remove_file(path)?;
        }
        if let Some(vault) = self.vault.get() {
            let _ = vault.clear_profile(profile).await;
        }
        Ok(())
    }

    pub async fn find_free_port(&self) -> u16 {
        for port in 16000..19000 {
            if self.is_port_free(port).await { return port; }
        }
        0
    }

    async fn is_port_free(&self, port: u16) -> bool {
        std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
    }

    pub async fn get_next_profile_name(&self) -> Result<String> {
        let profiles = self.list_profiles().await?;
        let mut i = 1;
        loop {
            let name = format!("profile_{}", i);
            if !profiles.contains(&name) {
                return Ok(name);
            }
            i += 1;
        }
    }

}