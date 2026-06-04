use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_openapi_url")]
    pub openapi_url: String,
}

fn default_openapi_url() -> String {
    "default".to_string()
}

fn main() {
    let s = r#"
storage:
  store: sqlite
  db_url: "sqlite:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/target/cowen_tests_macos/shared_storage_case_14.db"
log:
  level: debug
openapi_url: "http://127.0.0.1:9299"
stream_url: "ws://127.0.0.1:9299"
telemetry_enabled: false
ai_enabled: false
"#;
    let cfg: Result<AppConfig, _> = serde_yaml::from_str(s);
    println!("{:?}", cfg);
}
