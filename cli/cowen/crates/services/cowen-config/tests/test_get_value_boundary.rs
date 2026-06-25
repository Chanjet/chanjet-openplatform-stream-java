use cowen_config::ConfigManager;
use std::fs;
use std::sync::OnceLock;
use tempfile::tempdir;
use tokio::sync::Mutex;

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
async fn test_get_value_boundary_conditions() {
    let _guard = get_test_lock().lock().await;
    clean_env();
    let dir = tempdir().unwrap();
    let app_dir = dir.path().to_path_buf();

    let app_yaml_path = app_dir.join("app.yaml");
    fs::write(&app_yaml_path, "storage:\n  store: local\n  db_url: null").unwrap();

    let profile_path = app_dir.join("boundary_test.yaml");
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

    // 1. Valid reading
    let val = mgr.get_value("boundary_test", "openapi_url").await;
    assert!(val.is_ok(), "Should be able to read existing fields");

    // 2. Read missing field in existing profile (should return None or Err or Null)
    let res = mgr.get_value("boundary_test", "non_existent_field").await;
    assert!(
        res.is_err() || res.unwrap().is_null(),
        "Getting a non-existent field should return error or null"
    );

    // 3. Read valid field in missing profile (cowen-config falls back to main profile!)
    let res = mgr.get_value("missing_profile", "log.level").await;
    assert!(
        res.is_ok(),
        "Getting a field from a missing profile should fall back to main"
    );

    // 4. Read empty key
    let res = mgr.get_value("boundary_test", "").await;
    assert!(
        res.is_err() || res.unwrap().is_null(),
        "Getting an empty key should return error or null"
    );

    // 5. Set missing field (boundary test for set)
    // cowen-config allows dynamic key insertion!
    let res = mgr
        .set_value("boundary_test", "non_existent_field", "value")
        .await;
    assert!(
        res.is_ok(),
        "Setting a new dynamic field should succeed in cowen-config"
    );

    std::mem::forget(dir);
}
