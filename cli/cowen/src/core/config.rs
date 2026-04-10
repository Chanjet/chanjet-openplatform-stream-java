use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use std::fs;

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub app_key: String,
    pub openapi_url: String,
    pub stream_url: String,
    pub webhook_target: String,
    pub log: LogConfig,
    pub telemetry_enabled: bool,
    pub ai_enabled: bool,
    // Note: Secrets like app_secret are now in Vault, not Config file
    #[serde(skip)]
    pub app_secret: String,
    #[serde(skip)]
    pub certificate: String,
    #[serde(skip)]
    pub encrypt_key: String,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mask = |s: &str| -> String {
            if s.len() > 8 {
                format!("{}...{}", &s[..8], &s[s.len() - 4..])
            } else {
                "***".to_string()
            }
        };
        f.debug_struct("Config")
            .field("app_key", &self.app_key)
            .field("openapi_url", &self.openapi_url)
            .field("stream_url", &self.stream_url)
            .field("webhook_target", &self.webhook_target)
            .field("log", &self.log)
            .field("telemetry_enabled", &self.telemetry_enabled)
            .field("ai_enabled", &self.ai_enabled)
            .field("app_secret", &mask(&self.app_secret))
            .field("certificate", &mask(&self.certificate))
            .field("encrypt_key", &mask(&self.encrypt_key))
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub level: String,
    pub rotation: String,
    pub max_files: usize,
    pub max_size_mb: u64,
}

impl Config {
    pub fn default_with_profile(_profile: &str) -> Self {
        Self {
            app_key: "".to_string(),
            // Use defaults from compile-time env if available, otherwise fallback (obfuscated)
            openapi_url: option_env!("DEF_OPENAPI_URL").map(|s| s.to_string()).unwrap_or_else(|| obfs!("https://openapi.chanjet.com")),
            stream_url: option_env!("DEF_STREAM_URL").map(|s| s.to_string()).unwrap_or_else(|| obfs!("https://stream-open.chanapp.chanjet.com")),
            webhook_target: "http://127.0.0.1:8080/webhook".to_string(),
            app_secret: "".to_string(),
            certificate: "".to_string(),
            encrypt_key: "".to_string(),
            telemetry_enabled: true,
            ai_enabled: true,
            log: LogConfig {
                level: "error".to_string(),
                rotation: "daily".to_string(),
                max_files: 7,
                max_size_mb: 100,
            },
        }
    }
}

pub struct ConfigManager {
    pub app_dir: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let app_dir = get_app_dir();
        if !app_dir.exists() {
            fs::create_dir_all(&app_dir).context("Failed to create app directory")?;
        }
        Ok(Self { app_dir })
    }

    pub fn load(&self, profile: &str) -> Result<Config> {
        let path = self.app_dir.join(format!("{}.yaml", profile));
        if !path.exists() {
            return Ok(Config::default_with_profile(profile));
        }
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, profile: &str, config: &Config) -> Result<()> {
        let path = self.app_dir.join(format!("{}.yaml", profile));
        let content = serde_yaml::to_string(config)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn get_default_profile(&self) -> String {
        let path = self.app_dir.join("current_profile");
        if let Ok(p) = fs::read_to_string(path) {
            return p.trim().to_string();
        }
        "default".to_string()
    }

    pub fn set_default_profile(&self, profile: &str) -> Result<()> {
        let path = self.app_dir.join("current_profile");
        fs::write(path, profile)?;
        Ok(())
    }

    pub fn list_profiles(&self) -> Result<Vec<String>> {
        let mut profiles = Vec::new();
        for entry in fs::read_dir(&self.app_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map(|s| s == "yaml").unwrap_or(false) {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    profiles.push(name.to_string());
                }
            }
        }
        if profiles.is_empty() {
            profiles.push("default".to_string());
        }
        Ok(profiles)
    }
}

pub fn get_app_dir() -> PathBuf {
    let home = directories::UserDirs::new().expect("Could not find home directory");
    let dir_name = std::env::var("APP_DIR_NAME")
        .unwrap_or_else(|_| option_env!("APP_DIR_NAME").map(|s| s.to_string()).unwrap_or_else(|| format!(".{}", env!("CARGO_PKG_NAME"))));
    
    home.home_dir().join(dir_name)
}
