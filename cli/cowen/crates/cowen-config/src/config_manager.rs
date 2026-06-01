use cowen_common::{CowenResult, CowenError};
use anyhow::Context;
use std::fs;
use std::path::PathBuf;
use cowen_common::config::{AppConfig, StorageConfig, Config};
use tokio::sync::OnceCell;
use std::sync::Arc;
use cowen_common::vault::Vault;
use cowen_common::events::event_bus;
use serde_yaml;
use cowen_infra::path::get_app_dir;

use crate::strategy::{ConfigStrategy, GlobalAppConfigStrategy, ProfileLockedStrategy, ProfileDefaultStrategy};

pub trait ConfigValidator: Send + Sync {
    fn validate_load(&self, profile: &str, config: &Config, is_distributed: bool, exists: bool) -> CowenResult<()>;
    fn validate_save(&self, profile: &str, config: &Config, is_distributed: bool) -> CowenResult<()>;
}

pub trait ConfigInterceptor: Send + Sync {
    fn validate(&self, key: &str, value: &str) -> CowenResult<()>;
}

#[derive(Debug, serde::Serialize)]
pub struct ConfigFieldDisplay {
    pub key: String,
    pub value: String,
    pub readonly: bool,
}

#[derive(Clone)]
pub struct ConfigManager {
    pub app_dir: PathBuf,
    vault: OnceCell<Arc<dyn Vault>>,
    validator: OnceCell<Arc<dyn ConfigValidator>>,
    interceptors: Arc<tokio::sync::Mutex<Vec<Arc<dyn ConfigInterceptor>>>>,
    strategies: Arc<Vec<Box<dyn ConfigStrategy>>>,
    app_config_tx: Arc<tokio::sync::watch::Sender<AppConfig>>,
    profile_txs: Arc<tokio::sync::Mutex<std::collections::HashMap<String, tokio::sync::watch::Sender<Config>>>>,
    manifest_cache: Arc<tokio::sync::Mutex<std::collections::HashMap<String, (Config, std::time::Instant)>>>,
}

impl ConfigManager {
    pub fn new() -> CowenResult<Self> {
        Self::new_with_dir(get_app_dir())
    }

    pub fn new_with_dir(app_dir: PathBuf) -> CowenResult<Self> {
        if !app_dir.exists() {
            fs::create_dir_all(&app_dir).context("Failed to create app directory")?;
        }

        let initial_app_cfg = Self::load_app_config_sync(&app_dir).unwrap_or_default();
        let (app_config_tx, _) = tokio::sync::watch::channel(initial_app_cfg);

        let mut default_interceptors: Vec<Arc<dyn ConfigInterceptor>> = Vec::new();
        default_interceptors.push(Arc::new(crate::interceptors::PortInterceptor));
        default_interceptors.push(Arc::new(crate::interceptors::UrlInterceptor));
        
        let strategies: Vec<Box<dyn ConfigStrategy>> = vec![
            Box::new(GlobalAppConfigStrategy),
            Box::new(ProfileLockedStrategy),
            Box::new(ProfileDefaultStrategy),
        ];

        let mgr = Self { 
            app_dir,
            vault: OnceCell::new(),
            validator: OnceCell::new(),
            interceptors: Arc::new(tokio::sync::Mutex::new(default_interceptors)),
            strategies: Arc::new(strategies),
            app_config_tx: Arc::new(app_config_tx),
            profile_txs: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            manifest_cache: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        };

        mgr.start_watcher();

        Ok(mgr)
    }

    pub async fn add_interceptor(&self, interceptor: Arc<dyn ConfigInterceptor>) {
        self.interceptors.lock().await.push(interceptor);
    }

    fn load_app_config_sync(app_dir: &std::path::Path) -> CowenResult<AppConfig> {
        let path = app_dir.join("app.yaml");
        let mut config = if !path.exists() {
            AppConfig::default()
        } else {
            let content = fs::read_to_string(path)?;
            serde_yaml::from_str(&content)?
        };

        // 🚀 SYNC: Environment variables have highest priority for storage settings
        if let (Ok(st), Ok(url)) = (std::env::var("COWEN_STORE_TYPE"), std::env::var("COWEN_DB_URL")) {
            config.storage.store = st;
            config.storage.db_url = Some(url);
        }

        config.apply_env_overrides();
        Ok(config)
    }

    pub fn subscribe_app_config(&self) -> tokio::sync::watch::Receiver<AppConfig> {
        self.app_config_tx.subscribe()
    }

    pub async fn subscribe_profile_config(&self, profile: &str) -> tokio::sync::watch::Receiver<Config> {
        let mut txs = self.profile_txs.lock().await;
        if let Some(tx) = txs.get(profile) {
            return tx.subscribe();
        }

        let initial_config = self.load(profile).await.unwrap_or_else(|_| Config::default_with_profile(profile));
        let (tx, rx) = tokio::sync::watch::channel(initial_config);
        txs.insert(profile.to_string(), tx);
        rx
    }

    fn start_watcher(&self) {
        use notify::{Watcher, RecursiveMode, EventKind};

        let app_dir = self.app_dir.clone();
        let mgr = self.clone();

        tokio::spawn(async move {
            let (tx, mut rx) = tokio::sync::mpsc::channel(1);
            
            let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            }).expect("Failed to create watcher");

            if let Err(e) = watcher.watch(&app_dir, RecursiveMode::NonRecursive) {
                tracing::error!(target: "sys", error = %e, "Failed to start file watcher");
                return;
            }

            let _watcher = watcher;

            while let Some(event) = rx.recv().await {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        for path in event.paths {
                            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                                if filename == "app.yaml" {
                                    if let Ok(new_cfg) = mgr.load_app_config().await {
                                        let _ = mgr.app_config_tx.send(new_cfg);
                                        tracing::info!(target: "sys", "AppConfig hot-reloaded");
                                    }
                                } else if filename.ends_with(".yaml") {
                                    let profile = filename.trim_end_matches(".yaml");
                                    let txs = mgr.profile_txs.lock().await;
                                    if let Some(tx) = txs.get(profile) {
                                        if let Ok(new_cfg) = mgr.load(profile).await {
                                            let _ = tx.send(new_cfg);
                                            tracing::info!(target: "sys", profile = %profile, "Profile config hot-reloaded");
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
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

    pub fn get_pid_file(&self, profile: &str) -> PathBuf {
        self.app_dir.join(format!("{}_daemon.pid", profile))
    }

    pub async fn find_free_port(&self) -> u16 {
        use rand::Rng;
        let start_range = std::env::var("COWEN_PORT_RANGE_START")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(16000);
            
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
        if let Ok(key) = std::env::var("COWEN_APP_KEY") {
            if !key.trim().is_empty() {
                return true;
            }
        }
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

        {
            let mut cache = self.manifest_cache.lock().await;
            if let Some((cfg, timestamp)) = cache.get(profile) {
                if timestamp.elapsed() < std::time::Duration::from_secs(3) {
                    return Ok(cfg.clone());
                } else {
                    cache.remove(profile);
                }
            }
        }

        if let Some(vault) = self.vault.get() {
            tracing::debug!(target: "sys", profile = %profile, "Attempting to load manifest from Vault");
            match vault.get_config_full(profile, "system:manifest").await {
                Ok(item) => {
                    tracing::info!(target: "sys", profile = %profile, version = %item.version, "Manifest loaded from Vault");
                    match serde_yaml::from_str::<Config>(&item.value) {
                        Ok(mut config) => {
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

                        if let Some(validator) = self.validator.get() {
                            validator.validate_load(profile, &config, is_db_mode, true)?;
                        }
                        let mut config = config;
                        config.apply_env_overrides();
                        {
                            let mut cache = self.manifest_cache.lock().await;
                            cache.insert(profile.to_string(), (config.clone(), std::time::Instant::now()));
                        }
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

        let (mut config, exists) = self.load_local_profile_with_status(profile).await?;
        
        if let Some(validator) = self.validator.get() {
            validator.validate_load(profile, &config, is_db_mode, exists)?;
        }

        config.apply_env_overrides();
        
        if let Ok(val) = std::env::var("COWEN_EXCLUSIVE") {
            config.exclusive = Some(val == "true" || val == "1");
        }

        {
            let mut cache = self.manifest_cache.lock().await;
            cache.insert(profile.to_string(), (config.clone(), std::time::Instant::now()));
        }

        Ok(config)
    }

    async fn load_local_profile_with_status(&self, profile: &str) -> CowenResult<(Config, bool)> {
        let profile_path = self.get_profile_path(profile);
        if profile_path.exists() {
            let content = fs::read_to_string(&profile_path)?;
            let mut config: Config = serde_yaml::from_str(&content)?;
            
            if config.version == 0 { config.version = 1; }

            if let Some(vault) = self.vault.get() {
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
                vault.set_config_conditional(profile, "system:manifest", &manifest, config.version).await?;
            } else {
                vault.set_config(profile, "system:manifest", &manifest).await?;
            }
            event_bus().publish(cowen_common::events::GlobalEvent::ConfigChanged { 
                profile: profile.to_string(), 
                key: "system:manifest".to_string() 
            });
            config.version += 1;
        }

        if !is_db_mode {
            let path = self.app_dir.join(format!("{}.yaml", profile));
            let content = serde_yaml::to_string(config)?;
            cowen_common::utils::secure_write(path, content)?;
        }

        {
            let mut cache = self.manifest_cache.lock().await;
            cache.remove(profile);
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
                    if url == "innerdb" { return false; }
                    
                    let db_path = self.app_dir.join("cowen.db");
                    let expected_sqlite = format!("sqlite://{}", db_path.to_string_lossy());
                    let expected_innerdb = format!("innerdb://{}", db_path.to_string_lossy());
                    
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

    fn get_strategy(&self, key: &str) -> &dyn ConfigStrategy {
        for strategy in self.strategies.iter() {
            if strategy.matches(key) {
                return strategy.as_ref();
            }
        }
        // Fallback should ideally never be reached because ProfileDefaultStrategy matches everything
        self.strategies.last().unwrap().as_ref()
    }

    pub async fn get_value(&self, profile: &str, key: &str) -> CowenResult<serde_json::Value> {
        let strategy = self.get_strategy(key);
        if strategy.is_global() {
            let app_cfg = self.load_app_config().await?;
            let val = serde_json::to_value(app_cfg)?;
            strategy.handle_get(key, &val)
        } else {
            let config = self.load(profile).await?;
            let val = serde_json::to_value(config)?;
            strategy.handle_get(key, &val)
        }
    }

    pub async fn set_value(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> {
        let mut final_value = value.to_string();
        if key == "log.level" {
            final_value = final_value.to_lowercase();
            let valid_levels = ["trace", "debug", "info", "warn", "error"];
            if !valid_levels.contains(&final_value.as_str()) {
                return Err(CowenError::Config(format!(
                    "Invalid log level: {}. Allowed levels: trace, debug, info, warn, error",
                    value
                )));
            }
        }
        let value = final_value.as_str();

        for interceptor in self.interceptors.lock().await.iter() {
            interceptor.validate(key, value)?;
        }

        let strategy = self.get_strategy(key);
        
        if strategy.is_global() {
            let mut app_cfg = self.load_app_config().await?;
            let mut val = serde_json::to_value(&app_cfg)?;
            strategy.handle_set(key, value, &mut val)?;
            app_cfg = serde_json::from_value(val)?;
            self.save_app_config(&app_cfg).await?;
        } else {
            let config = self.load(profile).await?;
            let mut val = serde_json::to_value(&config)?;
            strategy.handle_set(key, value, &mut val)?;
            let mut new_config: Config = serde_json::from_value(val)?;
            
            new_config.app_secret = config.app_secret;
            new_config.certificate = config.certificate;
            new_config.encrypt_key = config.encrypt_key;
            new_config.version = config.version;
            
            self.save(profile, &mut new_config).await?;
        }
        Ok(())
    }

    pub async fn unset_value(&self, profile: &str, key: &str) -> CowenResult<()> {
        let strategy = self.get_strategy(key);

        if strategy.is_global() {
            let mut app_cfg = self.load_app_config().await?;
            let mut val = serde_json::to_value(&app_cfg)?;
            strategy.handle_unset(key, &mut val)?;
            app_cfg = serde_json::from_value(val)?;
            self.save_app_config(&app_cfg).await?;
        } else {
            let config = self.load(profile).await?;
            let mut val = serde_json::to_value(&config)?;
            strategy.handle_unset(key, &mut val)?;
            let mut new_config: Config = serde_json::from_value(val)?;
            
            new_config.app_secret = config.app_secret;
            new_config.certificate = config.certificate;
            new_config.encrypt_key = config.encrypt_key;
            new_config.version = config.version;
            
            self.save(profile, &mut new_config).await?;
        }
        Ok(())
    }

    pub async fn auto_migrate(&self) -> CowenResult<()> {
        let local_profiles = self.list_local_profiles()?;
        let mut app_cfg = self.load_app_config().await?;
        let mut migrated = false;

        for profile in local_profiles {
            let path = self.app_dir.join(format!("{}.yaml", profile));
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut yaml) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    // Extract 'storage' from profile if present
                    if let Some(storage) = yaml.get_mut("storage") {
                        let mut should_remove = false;
                        
                        if let Ok(new_storage) = serde_yaml::from_value::<StorageConfig>(storage.clone()) {
                            // 🛡️ VALIDATION: Only migrate if it's a known valid store and has URL if needed
                            let is_valid = match new_storage.store.as_str() {
                                "local" | "innerdb" | "sqlite" => true,
                                "mysql" | "postgres" | "redis" | "mssql" => new_storage.db_url.as_ref().map(|u| !u.trim().is_empty()).unwrap_or(false),
                                _ => false,
                            };

                            if is_valid {
                                // Merge into app_cfg if app_cfg is default/empty
                                if app_cfg.storage.store == "local" || app_cfg.storage.store == "innerdb" {
                                    println!("📦 Migrating storage config from profile: {}", profile);
                                    app_cfg.storage = new_storage;
                                    migrated = true;
                                    should_remove = true;
                                } else if app_cfg.storage == new_storage {
                                    // Already matches global, safe to remove from profile
                                    should_remove = true;
                                }
                            }
                        }

                        if should_remove {
                            // Remove storage from profile yaml
                            if let Some(mapping) = yaml.as_mapping_mut() {
                                mapping.remove(serde_yaml::Value::String("storage".to_string()));
                            }

                            // Backup and Save updated profile
                            let backup_path = path.with_extension("yaml.bak");
                            if !backup_path.exists() {
                                let _ = fs::copy(&path, &backup_path);
                            }
                            if let Ok(updated) = serde_yaml::to_string(&yaml) {
                                 let _ = cowen_common::utils::secure_write(&path, updated);
                                 println!("  ✓ Removed storage config from {}.yaml (backup created: {}.yaml.bak)", profile, profile);
                            }
                        }
                    }
                }
            }
        }

        if migrated {
            self.save_app_config(&app_cfg).await?;
            println!("✨ Global app.yaml updated with migrated storage settings.");
        }

        Ok(())
    }

    pub async fn load_app_config(&self) -> CowenResult<AppConfig> {
        let app_file = self.app_dir.join("app.yaml");
        let mut config = if app_file.exists() {
            let content = tokio::fs::read_to_string(&app_file).await?;
            serde_yaml::from_str(&content).map_err(|e| CowenError::Config(e.to_string()))?
        } else {
            AppConfig::default()
        };

        if let (Ok(st), Ok(url)) = (std::env::var("COWEN_STORE_TYPE"), std::env::var("COWEN_DB_URL")) {
            config.storage.store = st;
            config.storage.db_url = Some(url);
        }

        config.apply_env_overrides();

        Ok(config)
    }

    pub async fn save_app_config(&self, config: &AppConfig) -> CowenResult<()> {
        let path = self.app_dir.join("app.yaml");
        let content = serde_yaml::to_string(config)?;
        cowen_common::utils::secure_write(path, content)?;
        event_bus().publish(cowen_common::events::GlobalEvent::ConfigChanged { 
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
        cowen_common::utils::secure_write(path, profile)?;
        Ok(())
    }

    pub async fn list_values(&self, profile: &str) -> CowenResult<serde_json::Value> {
        let app_cfg = self.load_app_config().await?;
        let config = self.load(profile).await?;
        
        let mut val = serde_json::json!({
            "global": app_cfg,
            "profile": config
        });
        
        let sensitive_fields = ["app_secret", "certificate", "encrypt_key", "db_url"];
        self.mask_value(&mut val, &sensitive_fields);
        
        Ok(val)
    }

    pub async fn list_all_values(&self) -> CowenResult<serde_json::Value> {
        let app_cfg = self.load_app_config().await?;
        
        let mut profiles_map = serde_json::Map::new();
        if let Ok(profiles) = self.list_local_profiles() {
            for p in profiles {
                if let Ok(config) = self.load(&p).await {
                    profiles_map.insert(p, serde_json::to_value(&config)?);
                }
            }
        }
        
        let mut val = serde_json::json!({
            "global": app_cfg,
            "profiles": profiles_map
        });
        
        let sensitive_fields = ["app_secret", "certificate", "encrypt_key", "db_url"];
        self.mask_value(&mut val, &sensitive_fields);
        
        Ok(val)
    }

    fn flatten_json(prefix: &str, val: &serde_json::Value, result: &mut std::collections::BTreeMap<String, serde_json::Value>) {
        if let Some(obj) = val.as_object() {
            for (k, v) in obj {
                let new_key = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
                if v.is_object() {
                    Self::flatten_json(&new_key, v, result);
                } else {
                    result.insert(new_key, v.clone());
                }
            }
        }
    }

    pub async fn get_global_display(&self) -> CowenResult<Vec<ConfigFieldDisplay>> {
        let app_cfg = self.load_app_config().await?;
        let mut global_json = serde_json::to_value(&app_cfg)?;

        let sensitive_fields = ["app_secret", "certificate", "encrypt_key"];
        self.mask_value(&mut global_json, &sensitive_fields);

        let mut global_map = std::collections::BTreeMap::new();
        Self::flatten_json("", &global_json, &mut global_map);

        let mut global_fields = Vec::new();
        for (k, v) in global_map {
            if v.is_null() { continue; }
            let mut val_str = match v {
                serde_json::Value::String(s) => s,
                _ => v.to_string(),
            };
            if k.ends_with("_url") && !val_str.is_empty() {
                val_str = cowen_common::utils::mask_url_query(&val_str);
            }
            if k == "log.max_size_mb" {
                val_str = format!("{} MB", val_str);
            }
            let readonly = self.get_strategy(&k).is_readonly(&k);
            global_fields.push(ConfigFieldDisplay { key: k, value: val_str, readonly });
        }

        Ok(global_fields)
    }

    pub async fn get_profile_display(&self, profile: &str) -> CowenResult<Vec<ConfigFieldDisplay>> {
        let config = self.load(profile).await?;
        let mut profile_json = serde_json::to_value(&config)?;

        let sensitive_fields = ["app_secret", "certificate", "encrypt_key"];
        self.mask_value(&mut profile_json, &sensitive_fields);

        let mut profile_map = std::collections::BTreeMap::new();
        Self::flatten_json("", &profile_json, &mut profile_map);

        let mut profile_fields = Vec::new();
        for (k, v) in profile_map {
            if v.is_null() { continue; }
            let mut val_str = match v {
                serde_json::Value::String(s) => s,
                _ => v.to_string(),
            };
            if k.ends_with("_url") && !val_str.is_empty() {
                val_str = cowen_common::utils::mask_url_query(&val_str);
            }
            let readonly = self.get_strategy(&k).is_readonly(&k);
            profile_fields.push(ConfigFieldDisplay { key: k, value: val_str, readonly });
        }

        Ok(profile_fields)
    }

    fn mask_value(&self, val: &mut serde_json::Value, sensitive_fields: &[&str]) {
        match val {
            serde_json::Value::Object(map) => {
                for (k, v) in map.iter_mut() {
                    if sensitive_fields.contains(&k.as_str()) {
                        *v = serde_json::Value::String("******".to_string());
                    } else {
                        self.mask_value(v, sensitive_fields);
                    }
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr.iter_mut() {
                    self.mask_value(v, sensitive_fields);
                }
            }
            _ => {}
        }
    }

    pub async fn list_profiles(&self) -> CowenResult<Vec<String>> {
        let mut profiles = std::collections::HashSet::new();
        if let Some(vault) = self.vault.get() {
            if let Ok(remote_profiles) = vault.list_all_profiles().await {
                let remote_profiles: Vec<String> = remote_profiles;
                for p in remote_profiles {
                    if !p.starts_with("app:") && p != "global" && p != "system" {
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

    pub fn list_local_profiles(&self) -> CowenResult<Vec<String>> {
        let mut profiles = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.app_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map(|s| s == "yaml").unwrap_or(false) {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        if !name.contains("_openapi") && name != "app" && name != "global" && name != "system" {
                            profiles.push(name.to_string());
                        }
                    }
                }
            }
        }
        Ok(profiles)
    }

    pub async fn find_profile_by_key(&self, app_key: &str) -> CowenResult<Option<String>> {
        let profiles = self.list_local_profiles()?;
        for profile in profiles {
            if let Ok(config) = self.load(&profile).await {
                if config.app_key == app_key {
                    return Ok(Some(profile));
                }
            }
        }
        Ok(None)
    }

    pub async fn find_profile_by_key_and_mode(&self, app_key: &str, mode: &cowen_common::models::AuthMode) -> CowenResult<Option<String>> {
        let profiles = self.list_profiles().await?;
        for profile in profiles {
            if let Ok(config) = self.load(&profile).await {
                if config.app_key == app_key && config.app_mode == *mode {
                    return Ok(Some(profile));
                }
            }
        }
        Ok(None)
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

        event_bus().publish(cowen_common::events::GlobalEvent::ProfileRenamed { 
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

        event_bus().publish(cowen_common::events::GlobalEvent::ConfigChanged { 
            profile: profile.to_string(), 
            key: "system:manifest".to_string() 
        });

        Ok(())
    }
}
