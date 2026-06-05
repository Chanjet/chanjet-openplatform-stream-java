#[test]
fn test_parse() {
    let content = r#"
app_key: AK_LOG
app_mode: oauth2
openapi_url: http://127.0.0.1:9299
stream_url: http://127.0.0.1:9299
webhook_target: http://127.0.0.1:8080
log:
  level: info
  rotation: daily
  max_size_mb: 100
  max_files: 7
telemetry_enabled: true
ai_enabled: false
proxy_port: 8081
proxy_enabled: true
version: 1
"#;
    let cfg: Result<cowen_common::config::AppConfig, _> = serde_yaml::from_str(content);
    println!("{:?}", cfg);
}
