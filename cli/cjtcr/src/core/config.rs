use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use directories::UserDirs;
use anyhow::{Result, Context};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppMode {
    #[serde(rename = "self-built")]
    SelfBuilt,
}

impl Default for AppMode {
    fn default() -> Self {
        AppMode::SelfBuilt
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub app_key: String,
    
    #[serde(skip)]
    pub app_secret: String,
    
    #[serde(skip)]
    pub certificate: String,
    
    #[serde(skip)]
    pub encrypt_key: String,
    
    #[serde(default)]
    pub app_mode: AppMode,
    
    #[serde(default = "default_log_level")]
    pub log_level: String,
    
    #[serde(default = "default_openapi_url")]
    pub openapi_url: String,
    
    #[serde(default = "default_stream_url")]
    pub stream_url: String,
    
    #[serde(default)]
    pub webhook_target: String,
}

fn default_log_level() -> String { "info".to_string() }

fn default_openapi_url() -> String {
    option_env!("DEF_OPENAPI_URL")
        .unwrap_or("https://openapi.chanjet.com")
        .to_string()
}

fn default_stream_url() -> String {
    option_env!("DEF_STREAM_URL")
        .unwrap_or("https://stream-open.chanapp.chanjet.com")
        .to_string()
}

pub struct ConfigManager {
    base_dir: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let user_dirs = UserDirs::new().context("Failed to get user directories")?;
        let base_dir = user_dirs.home_dir().join(".cjtc");
        if !base_dir.exists() {
            fs::create_dir_all(&base_dir)?;
        }
        Ok(Self { base_dir })
    }

    pub fn load(&self, profile: &str) -> Result<Config> {
        let path = self.base_dir.join(format!("{}.yaml", profile));
        if !path.exists() {
            return Ok(Config::default_with_profile(profile));
        }

        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, profile: &str, config: &Config) -> Result<()> {
        let path = self.base_dir.join(format!("{}.yaml", profile));
        let content = serde_yaml::to_string(config)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn get_default_profile(&self) -> String {
        let current_profile_path = self.base_dir.join("current_profile");
        if let Ok(profile) = fs::read_to_string(&current_profile_path) {
            let active = profile.trim().to_string();
            if !active.is_empty() {
                return active;
            }
        }
        "default".to_string()
    }

    pub fn set_default_profile(&self, profile: &str) -> Result<()> {
        let current_profile_path = self.base_dir.join("current_profile");
        fs::write(current_profile_path, profile.trim())?;
        Ok(())
    }
}

impl Config {
    pub fn default_with_profile(_profile: &str) -> Self {
        Self {
            app_key: String::new(),
            app_secret: String::new(),
            certificate: String::new(),
            encrypt_key: String::new(),
            app_mode: AppMode::SelfBuilt,
            log_level: default_log_level(),
            openapi_url: default_openapi_url(),
            stream_url: default_stream_url(),
            webhook_target: String::new(),
        }
    }
}
