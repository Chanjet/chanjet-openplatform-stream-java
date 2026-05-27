use cowen_config::ConfigManager;
use cowen_common::config::{AppConfig, Config};
use std::fs;
use tempfile::tempdir;
use tokio::time::{sleep, Duration};

fn clean_env() {
    std::env::remove_var("COWEN_STORE_TYPE");
    std::env::remove_var("COWEN_DB_URL");
    std::env::remove_var("COWEN_CACHE_TYPE");
    std::env::remove_var("COWEN_CACHE_URL");
}

#[tokio::test]
async fn test_app_config_hot_reload() {
    clean_env();
    let dir = tempdir().unwrap();
    let app_dir = dir.path().to_path_buf();
    
    // Initial app.yaml
    let app_yaml_path = app_dir.join("app.yaml");
    fs::write(&app_yaml_path, "storage:\n  store: local").unwrap();
    
    // Set env to use this directory
    std::env::set_var("COWEN_HOME", app_dir.to_str().unwrap());
    
    let mgr = ConfigManager::new().unwrap();
    let mut rx = mgr.subscribe_app_config();
    
    assert_eq!(rx.borrow().storage.store, "local");
    
    // Give background watcher time to register before writing
    sleep(Duration::from_millis(500)).await;

    // Update app.yaml
    fs::write(&app_yaml_path, "storage:\n  store: innerdb").unwrap();
    
    // Wait for notify to pick up changes (might take a bit depending on OS)
    let mut found = false;
    for _ in 0..20 {
        if rx.changed().await.is_ok() {
            if rx.borrow().storage.store == "innerdb" {
                found = true;
                break;
            }
        }
        sleep(Duration::from_millis(100)).await;
    }
    
    assert!(found, "AppConfig did not hot-reload");
}
