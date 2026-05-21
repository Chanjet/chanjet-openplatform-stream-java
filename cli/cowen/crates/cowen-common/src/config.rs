use serde::{Deserialize, Serialize};

pub const BUILTIN_CLIENT_ID: &str = "3x45dOtt";
pub const DEF_MARKET_URL: &str = "https://market.chanjet.com";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default = "default_zero")]
    pub monitor_port: u16,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            storage: StorageConfig::default(),
            monitor_port: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageConfig {
    #[serde(default = "default_store")]
    pub store: String,
    pub db_url: Option<String>,
    #[serde(default = "default_cache")]
    pub cache: String,
    pub cache_url: Option<String>,
}

fn default_store() -> String {
    "innerdb".to_string()
}
fn default_cache() -> String {
    "none".to_string()
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SecurityLevel {
    #[serde(rename = "strict")]
    #[default]
    Strict,
    #[serde(rename = "flexible")]
    Flexible,
    #[serde(rename = "disabled")]
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SecurityConfig {
    #[serde(default)]
    pub level: SecurityLevel,
    #[serde(default)]
    pub allow_cidr: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub app_key: String,
    pub openapi_url: String,
    pub stream_url: String,
    pub webhook_target: String,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default = "default_log")]
    pub log: LogConfig,
    #[serde(default = "default_true")]
    pub telemetry_enabled: bool,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default = "default_zero")]
    pub proxy_port: u16,
    #[serde(default = "default_true")]
    pub proxy_enabled: bool,
    #[serde(default)]
    pub app_mode: crate::models::AuthMode,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SearchConfig {
    #[serde(default)]
    pub plugins: Vec<PluginEntry>,
    #[serde(default)]
    pub enabled: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginEntry {
    pub name: String,
    pub path: String,
    pub r#type: String,
}

fn default_true() -> bool {
    true
}
fn default_zero() -> u16 {
    0
}

fn default_log() -> LogConfig {
    LogConfig {
        level: "info".to_string(),
        rotation: default_rotation(),
        max_size_mb: default_max_size(),
        max_files: default_max_files(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_rotation")]
    pub rotation: String,
    #[serde(default = "default_max_size")]
    pub max_size_mb: u64,
    #[serde(default = "default_max_files")]
    pub max_files: usize,
}

fn default_rotation() -> String {
    "daily".to_string()
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_max_size() -> u64 {
    100
}
fn default_max_files() -> usize {
    7
}

impl Config {
    pub fn default_with_profile(_p: &str) -> Self {
        Self {
            app_key: "".to_string(),
            openapi_url: "https://openapi.chanjet.com".to_string(),
            stream_url: "https://stream-open.chanapp.chanjet.com".to_string(),
            webhook_target: "http://localhost:8080".to_string(),
            security: SecurityConfig::default(),
            log: LogConfig {
                level: "info".to_string(),
                rotation: default_rotation(),
                max_size_mb: default_max_size(),
                max_files: default_max_files(),
            },
            telemetry_enabled: true,
            search: SearchConfig::default(),
            proxy_port: 0,
            proxy_enabled: true,
            app_mode: crate::models::AuthMode::Oauth2,
            app_secret: "".to_string(),
            certificate: "".to_string(),
            encrypt_key: "".to_string(),
            version: 0,
            exclusive: None,
        }
    }

    pub fn apply_env_overrides(&mut self) {
        if let Ok(key) = std::env::var("COWEN_APP_KEY") {
            self.app_key = key;
        }
        if let Ok(secret) = std::env::var("COWEN_APP_SECRET") {
            self.app_secret = secret;
        }
        if let Ok(ek) = std::env::var("COWEN_ENCRYPT_KEY") {
            self.encrypt_key = ek;
        }
        if let Ok(target) = std::env::var("COWEN_WEBHOOK_TARGET") {
            self.webhook_target = target;
        }
        if let Ok(url) = std::env::var("COWEN_OPENAPI_URL") {
            self.openapi_url = url;
        }
        if let Ok(url) = std::env::var("COWEN_STREAM_URL") {
            self.stream_url = url;
        }
        if let Ok(port) = std::env::var("COWEN_PROXY_PORT") {
            if let Ok(p) = port.parse::<u16>() {
                self.proxy_port = p;
            }
        }
        if let Ok(mode) = std::env::var("COWEN_APP_MODE") {
            self.app_mode = match mode.as_str() {
                "self-built" => crate::models::AuthMode::SelfBuilt,
                "store-app" => crate::models::AuthMode::StoreApp,
                _ => crate::models::AuthMode::Oauth2,
            };
        }
    }
}

pub use cowen_infra::path::get_app_dir;

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("app_key", &self.app_key)
            .field("app_mode", &self.app_mode)
            .field("version", &self.version)
            .finish()
    }
}
