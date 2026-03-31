use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use directories::UserDirs;
use anyhow::Result;
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
    
    #[serde(default)]
    pub log: LogConfig,
    
    #[serde(default = "default_openapi_url")]
    pub openapi_url: String,
    
    #[serde(default = "default_stream_url")]
    pub stream_url: String,
    
    #[serde(default)]
    pub webhook_target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    
    #[serde(default = "default_log_rotation")]
    pub rotation: String, // daily, hourly, minutely, never
    
    #[serde(default = "default_log_max_size")]
    pub max_size_mb: u64,
    
    #[serde(default = "default_log_max_files")]
    pub max_files: usize,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            rotation: default_log_rotation(),
            max_size_mb: default_log_max_size(),
            max_files: default_log_max_files(),
        }
    }
}

fn default_log_level() -> String { "info".to_string() }
fn default_log_rotation() -> String { "daily".to_string() }
fn default_log_max_size() -> u64 { 500 }
fn default_log_max_files() -> usize { 3 }

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

pub fn get_app_dir_name() -> &'static str {
    option_env!("APP_DIR_NAME").unwrap_or(".owenc")
}

pub fn get_app_dir() -> PathBuf {
    UserDirs::new()
        .expect("Failed to get user home directory")
        .home_dir()
        .join(get_app_dir_name())
}

pub struct ConfigManager {
    base_dir: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        Self::new_with_dir(get_app_dir())
    }

    pub fn new_with_dir(base_dir: PathBuf) -> Result<Self> {
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
            log: LogConfig::default(),
            openapi_url: default_openapi_url(),
            stream_url: default_stream_url(),
            webhook_target: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_manager_lifecycle() -> Result<()> {
        let tmp = tempdir()?;
        let mgr = ConfigManager::new_with_dir(tmp.path().to_path_buf())?;

        // 1. Load default
        let mut cfg = mgr.load("test_profile")?;
        assert_eq!(cfg.app_key, "");

        // 2. Modify and Save
        cfg.app_key = "key123".to_string();
        mgr.save("test_profile", &cfg)?;

        // 3. Re-load and verify
        let cfg2 = mgr.load("test_profile")?;
        assert_eq!(cfg2.app_key, "key123");

        // 4. Default profile
        assert_eq!(mgr.get_default_profile(), "default");
        mgr.set_default_profile("prod")?;
        assert_eq!(mgr.get_default_profile(), "prod");

        Ok(())
    }
}
