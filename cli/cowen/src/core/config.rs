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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            openapi_url: "https://api.chanjet.com".to_string(),
            stream_url: "wss://stream.chanjet.com".to_string(),
            webhook_target: "http://localhost:8080".to_string(),
            log: LogConfig { 
                level: "info".to_string(),
                rotation: default_rotation(),
                max_size_mb: default_max_size(),
                max_files: default_max_files(),
            },
            telemetry_enabled: true,
            ai_enabled: true,
            proxy_port: 8080,
            proxy_enabled: true,
            app_mode: crate::auth::models::AuthMode::Oauth2,
            app_secret: "".to_string(),
            certificate: "".to_string(),
            encrypt_key: "".to_string(),
            version: 0,
        }
    }
}

pub struct ConfigManager {
    app_dir: PathBuf,
    vault: tokio::sync::OnceCell<std::sync::Arc<dyn crate::core::vault::Vault>>,
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
        })
    }

    pub fn set_vault(&self, vault: std::sync::Arc<dyn crate::core::vault::Vault>) {
        let _ = self.vault.set(vault);
    }

    pub fn get_vault(&self) -> Option<std::sync::Arc<dyn crate::core::vault::Vault>> {
        self.vault.get().cloned()
    }

    pub async fn exists(&self, profile: &str) -> bool {
        // A profile exists if it has a local config file OR entries in the vault.
        // We don't just check list_profiles() because that includes the current default profile even if empty.
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

    pub async fn load(&self, profile: &str) -> Result<Config> {
        if let Some(vault) = self.vault.get() {
            if let Ok(item) = vault.get_config_full(profile, "system:manifest").await {
                if let Ok(mut config) = serde_yaml::from_str::<Config>(&item.value) {
                    config.version = item.version;
                    if let Ok(s) = vault.get_secret(profile, "app_secret").await { config.app_secret = s; }
                    if let Ok(c) = vault.get_secret(profile, "certificate").await { config.certificate = c; }
                    if let Ok(k) = vault.get_secret(profile, "encrypt_key").await { config.encrypt_key = k; }
                    return Ok(config);
                }
            }
        }

        let path = self.app_dir.join(format!("{}.yaml", profile));
        if !path.exists() {
            return Ok(Config::default_with_profile(profile));
        }
        let content = fs::read_to_string(path)?;
        let mut config: Config = serde_yaml::from_str(&content)?;

        if let Some(vault) = self.vault.get() {
            if let Ok(s) = vault.get_secret(profile, "app_secret").await { config.app_secret = s; }
            if let Ok(c) = vault.get_secret(profile, "certificate").await { config.certificate = c; }
            if let Ok(k) = vault.get_secret(profile, "encrypt_key").await { config.encrypt_key = k; }
            
            let manifest = serde_yaml::to_string(&config)?;
            let _ = vault.set_config(profile, "system:manifest", &manifest).await;
        }

        Ok(config)
    }

    pub async fn save(&self, profile: &str, config: &Config) -> Result<()> {
        let app_cfg = self.load_app_config().await?;
        let is_db_mode = app_cfg.storage.store != "local";

        if let Some(vault) = self.vault.get() {
            let _ = vault.set_secret(profile, "app_secret", &config.app_secret).await;
            let _ = vault.set_secret(profile, "certificate", &config.certificate).await;
            let _ = vault.set_secret(profile, "encrypt_key", &config.encrypt_key).await;
            
            let manifest = serde_yaml::to_string(config)?;
            if is_db_mode {
                vault.set_config_conditional(profile, "system:manifest", &manifest, config.version).await?;
            } else {
                let _ = vault.set_config(profile, "system:manifest", &manifest).await;
            }

            // Trigger notification for profile configuration change
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

            let app_config = if use_local {
                AppConfig { storage: StorageConfig { store: "local".to_string(), ..Default::default() } }
            } else {
                let db_path = self.app_dir.join("cowen.db");
                let db_url = format!("innerdb://{}", db_path.to_string_lossy());
                AppConfig { storage: StorageConfig { store: "innerdb".to_string(), db_url: Some(db_url), ..Default::default() } }
            };
            let _ = self.save_app_config(&app_config).await;
            return Ok(app_config);
        }
        let content = fs::read_to_string(path)?;
        let config: AppConfig = serde_yaml::from_str(&content)?;
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
        if let Ok(p) = fs::read_to_string(path) { return p.trim().to_string(); }
        "default".to_string()
    }

    pub fn set_default_profile(&self, profile: &str) -> Result<()> {
        let path = self.app_dir.join("current_profile");
        fs::write(path, profile)?;
        Ok(())
    }

    pub async fn list_profiles(&self) -> Result<Vec<String>> {
        let mut profiles = std::collections::HashSet::new();
        
        // 1. Scan Store (Distributed Registry)
        if let Some(vault) = self.vault.get() {
            if let Ok(remote_profiles) = vault.list_all_profiles().await {
                for p in remote_profiles {
                    profiles.insert(p);
                }
            }
        }

        // 2. Scan Filesystem (Local source)
        if let Ok(entries) = fs::read_dir(&self.app_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
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

    pub async fn find_free_port(&self) -> u16 {
        use std::net::TcpListener;
        for port in 8080..9000 {
            if TcpListener::bind(("127.0.0.1", port)).is_ok() {
                return port;
            }
        }
        8080
    }
}

pub fn get_app_dir() -> PathBuf {
    let home = directories::BaseDirs::new().unwrap().home_dir().to_path_buf();
    home.join(".cowen")
}
