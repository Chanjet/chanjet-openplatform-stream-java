use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default = "default_zero")]
    pub monitor_port: u16,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default = "default_log")]
    pub log: LogConfig,
    #[serde(default = "default_openapi_url")]
    pub openapi_url: String,
    #[serde(default = "default_stream_url")]
    pub stream_url: String,
    #[serde(default)]
    pub plugins: Vec<String>,
    #[serde(default = "default_true")]
    pub telemetry_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StorageConfig {
    pub store: String,
    pub db_url: Option<String>,
    pub cache: String,
    pub cache_url: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SecurityConfig {
    pub level: String,
    pub allow_cidr: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LogConfig {
    pub level: String,
    pub rotation: String,
    pub max_size_mb: u32,
    pub max_files: u32,
}
fn default_log() -> LogConfig {
    LogConfig {
        level: "info".to_string(),
        rotation: "daily".to_string(),
        max_size_mb: 100,
        max_files: 7,
    }
}
fn default_zero() -> u16 { 0 }
fn default_openapi_url() -> String { "".to_string() }
fn default_stream_url() -> String { "".to_string() }
fn default_true() -> bool { true }

fn main() {
    let yaml = r#"
storage:
  store: innerdb
  db_url: null
  cache: none
  cache_url: null
monitor_port: 55292
security:
  level: strict
  allow_cidr: []
log:
  level: info
  rotation: daily
  max_size_mb: 100
  max_files: 7
openapi_url: http://127.0.0.1:55278
stream_url: ws://127.0.0.1:55278/connect
plugins: []
telemetry_enabled: true
"#;
    match serde_yaml::from_str::<AppConfig>(yaml) {
        Ok(cfg) => println!("Parsed: {:?}", cfg),
        Err(e) => println!("Error: {}", e),
    }
}
