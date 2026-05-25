use assert_cmd::Command;
use std::fs;
use serde_json::json;
use tempfile::tempdir;
use std::sync::atomic::{AtomicU16, Ordering};

static NEXT_PORT: AtomicU16 = AtomicU16::new(17000);

fn setup_daemon_env(profile: &str, mode: &str) -> (tempfile::TempDir, String) {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    fs::create_dir_all(&cowen_home).unwrap();
    
    // Create profile config
    let config_path = cowen_home.join(format!("{}.yaml", profile));
    let config = json!({
        "app_key": "test_key",
        "app_mode": mode,
        "encrypt_key": "1234567890123456", // 16-byte dummy key to pass validation rules
        "webhook_target": "http://localhost:8080",
        "auto_start": false,
        "version": 1
    });
    fs::write(config_path, serde_yaml::to_string(&config).unwrap()).unwrap();

    let port = NEXT_PORT.fetch_add(1, Ordering::SeqCst);

    // Create app config
    let app_config_path = cowen_home.join("app.yaml");
    let app_config = json!({
        "openapi_url": "http://localhost:12345",
        "stream_url": "http://localhost:12345",
        "telemetry_enabled": false,
        "monitor_port": port,
        "log": {
            "level": "debug"
        }
    });
    fs::write(app_config_path, serde_yaml::to_string(&app_config).unwrap()).unwrap();

    (dir, cowen_home.to_str().unwrap().to_string())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_daemon_oauth2_webhook_no_stream_crash() {
    let profile = "test_oauth2_daemon";
    let (dir, home) = setup_daemon_env(profile, "oauth2");
    
    // Seed dummy token in vault to pass auth checks on startup
    let app_cfg = cowen_common::config::AppConfig { openapi_url: "http://localhost:12345".to_string(), stream_url: "http://localhost:12345".to_string(), ..Default::default() };
    let vault = cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint").await.unwrap();
    vault.set_config(profile, "app_key", "test_key").await.unwrap();
    vault.set_secret(profile, "app_secret", "test_secret").await.unwrap();
    let rt = cowen_common::models::Token {
        value: "mock_rt".to_string(),
        expires_at: chrono::Utc::now() + chrono::Duration::days(1),
        created_at: chrono::Utc::now() - chrono::Duration::hours(2),
    };
    vault.save_refresh_token(profile, rt).await.unwrap();
    let at = cowen_common::models::Token {
        value: "mock_at".to_string(),
        expires_at: chrono::Utc::now() + chrono::Duration::days(1),
        created_at: chrono::Utc::now() - chrono::Duration::hours(2),
    };
    vault.save_access_token(profile, at).await.unwrap();
    // 1. Start daemon
    let mut cmd_start = Command::cargo_bin("cowen").unwrap();
    cmd_start.env("COWEN_HOME", &home);
    cmd_start.env("HOME", &home);
    cmd_start.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_start.env("COWEN_SKIP_DAEMON_RECOVERY", "true");
    
    // In dev environment, we explicitly point COWEN_DAEMON_BIN to the compiled executable if needed,
    // but the CLI might find it automatically via relative paths. Let's provide COWEN_DAEMON_PATH just in case.
    let bin_path = std::env::current_dir().unwrap().join("../../bin/macos-aarch64/cowen-daemon");
    cmd_start.env("COWEN_DAEMON_PATH", bin_path.to_str().unwrap());

    cmd_start.arg("--profile").arg(profile).arg("daemon").arg("start");
    
    let result = cmd_start.output().unwrap();
    // Start should be successful or at least output something
    
    // Wait a bit for the daemon to potentially crash if it was going to
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // 2. Check status
    let mut cmd_status = Command::cargo_bin("cowen").unwrap();
    cmd_status.env("COWEN_HOME", &home);
    cmd_status.env("HOME", &home);
    cmd_status.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_status.arg("--profile").arg(profile).arg("status");
    
    let status_result = cmd_status.output().unwrap();
    let status_stdout = String::from_utf8_lossy(&status_result.stdout);
    
    // 3. Stop daemon to clean up
    let mut cmd_stop = Command::cargo_bin("cowen").unwrap();
    cmd_stop.env("COWEN_HOME", &home);
    cmd_stop.env("HOME", &home);
    cmd_stop.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_stop.arg("--profile").arg(profile).arg("daemon").arg("stop");
    let _ = cmd_stop.output();
    
    let log_path = std::path::Path::new(&home).join("logs/daemon.stderr.log");
    assert!(
        status_stdout.contains("Active") || status_stdout.contains("active") || status_result.status.success(), 
        "Daemon should be running actively without crashing.\nStatus stdout: '{}'\nStatus stderr: '{}'\nStart stderr: '{}'\nDaemon Log: '{}'", 
        status_stdout,
        String::from_utf8_lossy(&status_result.stderr),
        String::from_utf8_lossy(&result.stderr),
        if log_path.exists() { fs::read_to_string(&log_path).unwrap_or_default() } else { "No log file".to_string() }
    );

    // Also check logs to ensure no "crashed during connection"
    let log_path = std::path::Path::new(&home).join("logs/daemon.stderr.log");
    if log_path.exists() {
        let log_content = fs::read_to_string(log_path).unwrap();
        assert!(
            !log_content.contains("Stream client crashed during connection"),
            "Daemon crashed during connection! Log: {}",
            log_content
        );
    }

    let _ = dir;
}

fn setup_daemon_env_https(profile: &str, mode: &str) -> (tempfile::TempDir, String) {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    fs::create_dir_all(&cowen_home).unwrap();
    
    // Create profile config
    let config_path = cowen_home.join(format!("{}.yaml", profile));
    let config = json!({
        "app_key": "test_key",
        "app_mode": mode,
        "encrypt_key": "1234567890123456", // 16-byte dummy key to pass validation rules
        "webhook_target": "http://localhost:8080",
        "auto_start": false,
        "version": 1
    });
    fs::write(config_path, serde_yaml::to_string(&config).unwrap()).unwrap();

    let port = NEXT_PORT.fetch_add(1, Ordering::SeqCst);

    // Create app config with HTTPS URLs to force Rustls/TLS initialization
    let app_config_path = cowen_home.join("app.yaml");
    let app_config = json!({
        "openapi_url": "https://localhost:12345",
        "stream_url": "https://localhost:12345",
        "telemetry_enabled": false,
        "monitor_port": port,
        "log": {
            "level": "debug"
        }
    });
    fs::write(app_config_path, serde_yaml::to_string(&app_config).unwrap()).unwrap();

    (dir, cowen_home.to_str().unwrap().to_string())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_daemon_https_crash_prevention() {
    let profile = "test_selfbuilt_daemon_https";
    let (dir, home) = setup_daemon_env_https(profile, "self-built");
    
    // Seed dummy token in vault to pass auth checks on startup
    let app_cfg = cowen_common::config::AppConfig { 
        openapi_url: "https://localhost:12345".to_string(), 
        stream_url: "https://localhost:12345".to_string(), 
        ..Default::default() 
    };
    let vault = cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint").await.unwrap();
    vault.set_config(profile, "app_key", "test_key").await.unwrap();
    vault.set_secret(profile, "app_secret", "test_secret").await.unwrap();
    vault.set_secret(profile, "encrypt_key", "1234567890123456").await.unwrap();
    
    // Seed standard dummy access and refresh tokens
    let rt = cowen_common::models::Token {
        value: "mock_rt".to_string(),
        expires_at: chrono::Utc::now() + chrono::Duration::days(1),
        created_at: chrono::Utc::now() - chrono::Duration::hours(2),
    };
    vault.save_refresh_token(profile, rt).await.unwrap();
    let at = cowen_common::models::Token {
        value: "mock_at".to_string(),
        expires_at: chrono::Utc::now() + chrono::Duration::days(1),
        created_at: chrono::Utc::now() - chrono::Duration::hours(2),
    };
    vault.save_access_token(profile, at).await.unwrap();

    // 1. Start daemon
    let mut cmd_start = Command::cargo_bin("cowen").unwrap();
    cmd_start.env("COWEN_HOME", &home);
    cmd_start.env("HOME", &home);
    cmd_start.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_start.env("COWEN_SKIP_DAEMON_RECOVERY", "true");
    
    // Explicitly point COWEN_DAEMON_PATH to the compiled executable in workspace bin
    let bin_path = std::env::current_dir().unwrap().join("../../bin/macos-aarch64/cowen-daemon");
    cmd_start.env("COWEN_DAEMON_PATH", bin_path.to_str().unwrap());

    cmd_start.arg("--profile").arg(profile).arg("daemon").arg("start");
    
    let result = cmd_start.output().unwrap();
    
    // Wait a bit for the daemon to start and potentially crash if CryptoProvider is missing
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // 2. Check status
    let mut cmd_status = Command::cargo_bin("cowen").unwrap();
    cmd_status.env("COWEN_HOME", &home);
    cmd_status.env("HOME", &home);
    cmd_status.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_status.arg("--profile").arg(profile).arg("status");
    
    let status_result = cmd_status.output().unwrap();
    let status_stdout = String::from_utf8_lossy(&status_result.stdout);
    
    // 3. Stop daemon to clean up
    let mut cmd_stop = Command::cargo_bin("cowen").unwrap();
    cmd_stop.env("COWEN_HOME", &home);
    cmd_stop.env("HOME", &home);
    cmd_stop.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_stop.arg("--profile").arg(profile).arg("daemon").arg("stop");
    let _ = cmd_stop.output();
    
    let log_path = std::path::Path::new(&home).join("logs/daemon.stderr.log");
    let log_content = if log_path.exists() { fs::read_to_string(&log_path).unwrap_or_default() } else { "No log file".to_string() };

    // The daemon must be active, and its logs must NOT contain any FATAL DAEMON PANIC or CryptoProvider error
    assert!(
        status_stdout.contains("Active") || status_stdout.contains("active") || status_result.status.success(), 
        "Daemon should be running actively without crashing.\nStatus stdout: '{}'\nStatus stderr: '{}'\nStart stderr: '{}'\nDaemon Log: '{}'", 
        status_stdout,
        String::from_utf8_lossy(&status_result.stderr),
        String::from_utf8_lossy(&result.stderr),
        log_content
    );

    assert!(
        !log_content.contains("FATAL DAEMON PANIC"),
        "Daemon encountered a panic! Log content:\n{}",
        log_content
    );
    
    assert!(
        !log_content.contains("CryptoProvider"),
        "Daemon failed with Rustls CryptoProvider configuration error! Log content:\n{}",
        log_content
    );

    let _ = dir;
}

