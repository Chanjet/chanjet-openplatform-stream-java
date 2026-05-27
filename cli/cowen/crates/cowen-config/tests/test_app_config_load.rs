use cowen_config::ConfigManager;
use std::fs;
use tempfile::tempdir;
use std::sync::{Mutex, OnceLock};

static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

fn get_test_lock() -> &'static Mutex<()> {
    TEST_MUTEX.get_or_init(|| Mutex::new(()))
}

fn clean_env() {
    std::env::remove_var("COWEN_STORE_TYPE");
    std::env::remove_var("COWEN_DB_URL");
    std::env::remove_var("COWEN_CACHE_TYPE");
    std::env::remove_var("COWEN_CACHE_URL");
    std::env::remove_var("COWEN_MONITOR_PORT");
}

#[tokio::test]
async fn test_load_app_config_with_env_overrides_keeps_yaml_values() {
    let _guard = get_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    clean_env();
    let dir = tempdir().unwrap();
    let app_dir = dir.path().to_path_buf();

    // 1. Write an app.yaml with custom monitor_port and log config
    let app_yaml_path = app_dir.join("app.yaml");
    fs::write(
        &app_yaml_path,
        r#"monitor_port: 12345
storage:
  store: innerdb
  db_url: sqlite:///some/path/cowen.db
log:
  level: debug
  max_size_mb: 200
"#
    ).unwrap();

    std::env::set_var("COWEN_HOME", app_dir.to_str().unwrap());
    
    // 2. Simulate E2E test environment setting store/db env vars
    std::env::set_var("COWEN_STORE_TYPE", "postgres");
    std::env::set_var("COWEN_DB_URL", "postgres://localhost/test_db");

    let mgr = ConfigManager::new().unwrap();
    let app_cfg = mgr.load_app_config().await.unwrap();

    // 3. Assertions: Custom values from YAML should be preserved, while storage is overridden
    assert_eq!(app_cfg.monitor_port, 12345);
    assert_eq!(app_cfg.storage.store, "postgres");
    assert_eq!(app_cfg.storage.db_url, Some("postgres://localhost/test_db".to_string()));
    assert_eq!(app_cfg.log.level, "debug");
    assert_eq!(app_cfg.log.max_size_mb, 200);

    clean_env();
}
