use cowen_config::ConfigManager;
use std::fs;
use std::sync::{Mutex, OnceLock};
use tempfile::tempdir;

static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

fn get_test_lock() -> &'static Mutex<()> {
    TEST_MUTEX.get_or_init(|| Mutex::new(()))
}

fn clean_env() {
    std::env::remove_var("COWEN_STORE_TYPE");
    std::env::remove_var("COWEN_DB_URL");
    std::env::remove_var("COWEN_CACHE_TYPE");
    std::env::remove_var("COWEN_CACHE_URL");
}

#[tokio::test]
async fn test_set_value_log_level_lowercase_conversion() {
    let _guard = get_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    clean_env();
    let dir = tempdir().unwrap();
    let app_dir = dir.path().to_path_buf();

    // Create a default app.yaml
    let app_yaml_path = app_dir.join("app.yaml");
    fs::write(&app_yaml_path, "storage:\n  store: local\n  db_url: null").unwrap();

    // Create a mock profile
    let profile_path = app_dir.join("main.yaml");
    fs::write(
        &profile_path,
        r#"app_key: KEY123
openapi_url: http://localhost
stream_url: ws://localhost
webhook_target: http://target
log:
  level: debug
"#,
    )
    .unwrap();

    std::env::set_var("COWEN_HOME", app_dir.to_str().unwrap());

    let mgr = ConfigManager::new().unwrap();

    // Set level using uppercase "INFO"
    mgr.set_value("main", "log.level", "INFO").await.unwrap();

    // Get value to confirm it was converted to lowercase "info"
    let val = mgr.get_value("main", "log.level").await.unwrap();
    assert_eq!(val.as_str().unwrap(), "info");

    // Also double check file persistence contains lowercase "level: info"
    let file_content = fs::read_to_string(&app_yaml_path).unwrap();
    assert!(file_content.contains("level: info"));
    std::mem::forget(dir);
}

#[tokio::test]
async fn test_set_value_log_level_invalid() {
    let _guard = get_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    clean_env();
    let dir = tempdir().unwrap();
    let app_dir = dir.path().to_path_buf();

    let app_yaml_path = app_dir.join("app.yaml");
    fs::write(&app_yaml_path, "storage:\n  store: local\n  db_url: null").unwrap();

    let profile_path = app_dir.join("main.yaml");
    fs::write(
        &profile_path,
        r#"app_key: KEY123
openapi_url: http://localhost
stream_url: ws://localhost
webhook_target: http://target
log:
  level: debug
"#,
    )
    .unwrap();

    std::env::set_var("COWEN_HOME", app_dir.to_str().unwrap());

    let mgr = ConfigManager::new().unwrap();

    // Setting an invalid log level should return Err
    let res = mgr
        .set_value("main", "log.level", "invalid_log_level")
        .await;
    assert!(res.is_err());
}
