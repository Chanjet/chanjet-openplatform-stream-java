use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub storage: StorageConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            storage: StorageConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_store")]
    pub store: String,
    pub db_url: Option<String>,
    #[serde(default = "default_cache")]
    pub cache: String,
    pub cache_url: Option<String>,
}

fn default_store() -> String { "innerdb".to_string() }
fn default_cache() -> String { "none".to_string() }

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            store: default_store(),
            db_url: None,
            cache: default_cache(),
            cache_url: None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub app_key: String,
    pub openapi_url: String,
    pub stream_url: String,
    pub webhook_target: String,
    pub log: LogConfig,
    #[serde(default = "default_true")]
    pub telemetry_enabled: bool,
    #[serde(default = "default_true")]
    pub ai_enabled: bool,
    #[serde(default = "default_8080")]
    pub proxy_port: u16,
    #[serde(default = "default_true")]
    pub proxy_enabled: bool,
    #[serde(default)]
    pub app_mode: crate::auth::models::AuthMode,
    #[serde(skip)]
    pub app_secret: String,
    #[serde(skip)]
    pub certificate: String,
    #[serde(skip)]
    pub encrypt_key: String,
    #[serde(default)]
    pub version: u64,
    #[serde(default)]
    pub exclusive: Option<bool>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::core::utils::mask_string;
        f.debug_struct("Config")
            .field("app_key", &self.app_key)
            .field("openapi_url", &self.openapi_url)
            .field("stream_url", &self.stream_url)
            .field("webhook_target", &self.webhook_target)
            .field("log", &self.log)
            .field("telemetry_enabled", &self.telemetry_enabled)
            .field("ai_enabled", &self.ai_enabled)
            .field("proxy_port", &self.proxy_port)
            .field("proxy_enabled", &self.proxy_enabled)
            .field("app_mode", &self.app_mode)
            .field("app_secret", &mask_string(&self.app_secret))
            .field("certificate", &mask_string(&self.certificate))
            .field("encrypt_key", &mask_string(&self.encrypt_key))
            .field("version", &self.version)
            .field("exclusive", &self.exclusive)
            .finish()
    }
}

fn default_true() -> bool { true }
fn default_8080() -> u16 { 8080 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub level: String,
    #[serde(default = "default_rotation")]
    pub rotation: String,
    #[serde(default = "default_max_size")]
    pub max_size_mb: u64,
    #[serde(default = "default_max_files")]
    pub max_files: usize,
}

fn default_rotation() -> String { "daily".to_string() }
fn default_max_size() -> u64 { 100 }
fn default_max_files() -> usize { 7 }

impl Config {
    pub fn default_with_profile(_p: &str) -> Self {
        Self {
            app_key: "".to_string(),
            openapi_url: env!("DEF_OPENAPI_URL").to_string(),
            stream_url: env!("DEF_STREAM_URL").to_string(),
            webhook_target: "http://localhost:8080".to_string(),
            log: LogConfig { 
                level: "info".to_string(),
                rotation: default_rotation(),
                max_size_mb: default_max_size(),
                max_files: default_max_files(),
            },
            telemetry_enabled: true,
            ai_enabled: true,
            proxy_port: 57612,
            proxy_enabled: true,
            app_mode: crate::auth::models::AuthMode::Oauth2,
            app_secret: "".to_string(),
            certificate: "".to_string(),
            encrypt_key: "".to_string(),
            version: 0,
            exclusive: None,
        }
    }
}

pub trait ConfigValidator: Send + Sync {
    fn validate_load(&self, profile: &str, config: &Config, is_distributed: bool, exists: bool) -> Result<()>;
    fn validate_save(&self, profile: &str, config: &Config, is_distributed: bool) -> Result<()>;
}

pub struct ConfigManager {
    app_dir: PathBuf,
    vault: tokio::sync::OnceCell<std::sync::Arc<dyn crate::core::vault::Vault>>,
    validator: tokio::sync::OnceCell<std::sync::Arc<dyn ConfigValidator>>,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let app_dir = get_app_dir();
        if !app_dir.exists() {
            fs::create_dir_all(&app_dir).context("Failed to create app directory")?;
        }
        Ok(Self { 
            app_dir,
            vault: tokio::sync::OnceCell::new(),
            validator: tokio::sync::OnceCell::new(),
        })
    }

    pub fn set_vault(&self, vault: std::sync::Arc<dyn crate::core::vault::Vault>) {
        let _ = self.vault.set(vault);
    }

    pub fn set_validator(&self, validator: std::sync::Arc<dyn ConfigValidator>) {
        let _ = self.validator.set(validator);
    }

    pub fn get_vault(&self) -> Option<std::sync::Arc<dyn crate::core::vault::Vault>> {
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
                    // Check for default literal or expanded local paths
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

    pub async fn load(&self, profile: &str) -> Result<Config> {
        let app_cfg = self.load_app_config().await?;
        let is_db_mode = self.is_distributed_storage(&app_cfg);

        let (config, _exists) = if let Some(vault) = self.vault.get() {
            if is_db_mode {
                if let Ok(item) = vault.get_config_full(profile, "system:manifest").await {
                    match serde_yaml::from_str::<Config>(&item.value) {
                        Ok(mut config) => {
                            config.version = item.version;
                            if let Ok(s) = vault.get_secret(profile, "app_secret").await { config.app_secret = s; }
                            if let Ok(cert) = vault.get_secret(profile, "certificate").await { config.certificate = cert; }
                            if let Ok(ek) = vault.get_secret(profile, "encrypt_key").await { config.encrypt_key = ek; }
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
        if let Ok(key) = std::env::var("COWEN_APP_KEY") { config.app_key = key; }
        if let Ok(secret) = std::env::var("COWEN_APP_SECRET") { config.app_secret = secret; }
        if let Ok(ek) = std::env::var("COWEN_ENCRYPT_KEY") { config.encrypt_key = ek; }
        if let Ok(target) = std::env::var("COWEN_WEBHOOK_TARGET") { config.webhook_target = target; }
        if let Ok(url) = std::env::var("COWEN_OPENAPI_URL") { config.openapi_url = url; }
        if let Ok(url) = std::env::var("COWEN_STREAM_URL") { config.stream_url = url; }
        if let Ok(port) = std::env::var("COWEN_PROXY_PORT") {
            if let Ok(p) = port.parse::<u16>() { config.proxy_port = p; }
        }
        if let Ok(mode) = std::env::var("COWEN_APP_MODE") {
            config.app_mode = match mode.as_str() {
                "self-built" => crate::auth::models::AuthMode::SelfBuilt,
                "store-app" => crate::auth::models::AuthMode::StoreApp,
                _ => crate::auth::models::AuthMode::Oauth2,
            };
        }
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
                if let Ok(s) = vault.get_secret(profile, "app_secret").await { config.app_secret = s; }
                if let Ok(cert) = vault.get_secret(profile, "certificate").await { config.certificate = cert; }
                if let Ok(ek) = vault.get_secret(profile, "encrypt_key").await { config.encrypt_key = ek; }
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
            if !config.app_secret.is_empty() {
                let _ = vault.set_secret(profile, "app_secret", &config.app_secret).await;
            }
            if !config.certificate.is_empty() {
                let _ = vault.set_secret(profile, "certificate", &config.certificate).await;
            }
            if !config.encrypt_key.is_empty() {
                let _ = vault.set_secret(profile, "encrypt_key", &config.encrypt_key).await;
            }
            
            let manifest = serde_yaml::to_string(config)?;
            if is_db_mode {
                vault.set_config_conditional(profile, "system:manifest", &manifest, config.version).await?;
            } else {
                let _ = vault.set_config(profile, "system:manifest", &manifest).await;
            }
            let _ = vault.notify_config_changed(profile, "system:manifest").await;
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
        if let Some(vault) = self.vault.get() {
            let _ = vault.notify_config_changed("system", "app").await;
        }
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

    pub async fn get_next_profile_name(&self) -> Result<String> {
        let profiles = self.list_profiles().await?;
        let mut i = 1;
        loop {
            let name = format!("profile{}", i);
            if !profiles.contains(&name) {
                return Ok(name);
            }
            i += 1;
        }
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
        use std::net::TcpListener;
        TcpListener::bind("127.0.0.1:0")
            .map(|l| l.local_addr().unwrap().port())
            .unwrap_or(8080)
    }
}

pub fn get_app_dir() -> PathBuf {
    if let Ok(path) = std::env::var("COWEN_HOME") {
        return PathBuf::from(path);
    }
    let home = directories::BaseDirs::new().unwrap().home_dir().to_path_buf();
    home.join(".cowen")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_distributed_storage() {
        let mgr = ConfigManager::new().unwrap();
        
        // 1. Local is NOT distributed
        let mut app_cfg = AppConfig::default();
        app_cfg.storage.store = "local".to_string();
        assert!(!mgr.is_distributed_storage(&app_cfg));

        // 2. Default innerdb literal is NOT distributed
        app_cfg.storage.store = "innerdb".to_string();
        app_cfg.storage.db_url = Some("innerdb".to_string());
        assert!(!mgr.is_distributed_storage(&app_cfg));

        // 3. Default innerdb full path is NOT distributed
        let db_path = mgr.app_dir.join("cowen.db");
        app_cfg.storage.db_url = Some(format!("innerdb://{}", db_path.to_string_lossy()));
        assert!(!mgr.is_distributed_storage(&app_cfg));

        // 4. Custom SQLite path IS distributed (potential sharing)
        app_cfg.storage.db_url = Some("sqlite:///tmp/other.db".to_string());
        assert!(mgr.is_distributed_storage(&app_cfg));

        // 5. MySQL IS distributed
        app_cfg.storage.store = "mysql".to_string();
        app_cfg.storage.db_url = Some("mysql://user:pass@host/db".to_string());
        assert!(mgr.is_distributed_storage(&app_cfg));
    }

    #[test]
    fn test_default_urls() {
        let config = Config::default_with_profile("test");
        assert_eq!(config.openapi_url, env!("DEF_OPENAPI_URL"));
        assert_eq!(config.stream_url, env!("DEF_STREAM_URL"));
    }
}
