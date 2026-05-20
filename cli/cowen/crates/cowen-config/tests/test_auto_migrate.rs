use cowen_config::ConfigManager;
use cowen_common::config::AppConfig;
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
}

#[tokio::test]
async fn test_auto_migrate_valid_sqlite() {
    let _guard = get_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    clean_env();
    let dir = tempdir().unwrap();
    let app_dir = dir.path().to_path_buf();
    
    // Create an initial empty app.yaml
    let app_yaml_path = app_dir.join("app.yaml");
    fs::write(&app_yaml_path, "storage:\n  store: local\n  db_url: null").unwrap();
    
    // Create a profile with a valid sqlite configuration
    let profile_path = app_dir.join("valid_sqlite.yaml");
    fs::write(
        &profile_path,
        r#"app_key: KEY123
openapi_url: http://localhost
stream_url: ws://localhost
webhook_target: http://target
storage:
  store: sqlite
  db_url: sqlite://test.db
"#
    ).unwrap();
    
    std::env::set_var("COWEN_HOME", app_dir.to_str().unwrap());
    
    let mgr = ConfigManager::new().unwrap();
    mgr.auto_migrate().await.unwrap();
    
    // Verify that app.yaml has been updated with the sqlite store configuration
    let updated_app_content = fs::read_to_string(&app_yaml_path).unwrap();
    let updated_app_cfg: AppConfig = serde_yaml::from_str(&updated_app_content).unwrap();
    assert_eq!(updated_app_cfg.storage.store, "sqlite");
    assert_eq!(updated_app_cfg.storage.db_url, Some("sqlite://test.db".to_string()));
    
    // Verify that the profile itself has had its store configuration migrated (i.e. 'store' under 'storage' is gone)
    let profile_content = fs::read_to_string(&profile_path).unwrap();
    let profile_val: serde_json::Value = serde_yaml::from_str(&profile_content).unwrap();
    assert!(profile_val.get("storage").and_then(|v| v.get("store")).is_none());
}

#[tokio::test]
async fn test_auto_migrate_invalid_store() {
    let _guard = get_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    clean_env();
    let dir = tempdir().unwrap();
    let app_dir = dir.path().to_path_buf();
    
    // Create an initial local app.yaml
    let app_yaml_path = app_dir.join("app.yaml");
    fs::write(&app_yaml_path, "storage:\n  store: local\n  db_url: null").unwrap();
    
    // Create a profile with an invalid unknown_store
    let profile_path = app_dir.join("invalid_store.yaml");
    fs::write(
        &profile_path,
        r#"app_key: KEY456
openapi_url: http://localhost
stream_url: ws://localhost
webhook_target: http://target
storage:
  store: unknown_store
"#
    ).unwrap();
    
    std::env::set_var("COWEN_HOME", app_dir.to_str().unwrap());
    
    let mgr = ConfigManager::new().unwrap();
    mgr.auto_migrate().await.unwrap();
    
    // Verify that app.yaml has NOT been updated and still has "local"
    let updated_app_content = fs::read_to_string(&app_yaml_path).unwrap();
    let updated_app_cfg: AppConfig = serde_yaml::from_str(&updated_app_content).unwrap();
    assert_eq!(updated_app_cfg.storage.store, "local");
    
    // Verify that the profile yaml still retains the "storage" field with "unknown_store" since migration was skipped
    let profile_content = fs::read_to_string(&profile_path).unwrap();
    let profile_val: serde_json::Value = serde_yaml::from_str(&profile_content).unwrap();
    assert_eq!(
        profile_val.get("storage").and_then(|v| v.get("store")).and_then(|s| s.as_str()),
        Some("unknown_store")
    );
}

#[tokio::test]
async fn test_auto_migrate_invalid_distributed_missing_url() {
    let _guard = get_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    clean_env();
    let dir = tempdir().unwrap();
    let app_dir = dir.path().to_path_buf();
    
    // Create an initial local app.yaml
    let app_yaml_path = app_dir.join("app.yaml");
    fs::write(&app_yaml_path, "storage:\n  store: local\n  db_url: null").unwrap();
    
    // Create a profile with a distributed mysql store but missing db_url
    let profile_path = app_dir.join("missing_url.yaml");
    fs::write(
        &profile_path,
        r#"app_key: KEY789
openapi_url: http://localhost
stream_url: ws://localhost
webhook_target: http://target
storage:
  store: mysql
  db_url: null
"#
    ).unwrap();
    
    std::env::set_var("COWEN_HOME", app_dir.to_str().unwrap());
    
    let mgr = ConfigManager::new().unwrap();
    mgr.auto_migrate().await.unwrap();
    
    // Verify that app.yaml has NOT been updated and still has "local"
    let updated_app_content = fs::read_to_string(&app_yaml_path).unwrap();
    let updated_app_cfg: AppConfig = serde_yaml::from_str(&updated_app_content).unwrap();
    assert_eq!(updated_app_cfg.storage.store, "local");
    
    // Verify that the profile yaml still retains the "storage" field with "mysql"
    let profile_content = fs::read_to_string(&profile_path).unwrap();
    let profile_val: serde_json::Value = serde_yaml::from_str(&profile_content).unwrap();
    assert_eq!(
        profile_val.get("storage").and_then(|v| v.get("store")).and_then(|s| s.as_str()),
        Some("mysql")
    );
}
