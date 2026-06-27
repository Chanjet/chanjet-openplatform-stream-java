use assert_cmd::Command;
use serde_json::json;
use std::fs;
use std::sync::atomic::{AtomicU16, Ordering};
use tempfile::tempdir;

static NEXT_PORT: AtomicU16 = AtomicU16::new(0);

fn get_next_port() -> u16 {
    let port = NEXT_PORT.load(Ordering::SeqCst);
    if port == 0 {
        let base = std::process::id() as u16 % 15000;
        let new_port = 17000 + base;
        let _ = NEXT_PORT.compare_exchange(0, new_port, Ordering::SeqCst, Ordering::SeqCst);
    }
    NEXT_PORT.fetch_add(1, Ordering::SeqCst)
}

fn setup_daemon_env(
    profile: &str,
    mode: &str,
) -> (
    tempfile::TempDir,
    String,
    crate::e2e::rust::common::DaemonKiller,
) {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    fs::create_dir_all(&cowen_home).unwrap();

    let proxy_port = get_next_port();

    // Create profile config
    let config_path = cowen_home.join(format!("{}.yaml", profile));
    let config = json!({
        "app_key": "test_key",
        "app_mode": mode,
        "encrypt_key": "1234567890123456", // 16-byte dummy key to pass validation rules
        "webhook_target": "http://localhost:8080",
        "auto_start": false,
        "proxy_port": proxy_port,
        "version": 1
    });
    fs::write(config_path, serde_yaml::to_string(&config).unwrap()).unwrap();

    let port = get_next_port();

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

    (
        dir,
        cowen_home.to_str().unwrap().to_string(),
        crate::e2e::rust::common::DaemonKiller {
            home: cowen_home.to_str().unwrap().to_string(),
        },
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn test_daemon_oauth2_webhook_no_stream_crash() {
    let profile = "test_oauth2_daemon";
    let (dir, home, _killer) = setup_daemon_env(profile, "oauth2");

    // Seed dummy token in vault to pass auth checks on startup
    let app_cfg = cowen_common::config::AppConfig {
        openapi_url: "http://localhost:12345".to_string(),
        stream_url: "http://localhost:12345".to_string(),
        ..Default::default()
    };
    let vault =
        cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint")
            .await
            .unwrap();
    vault
        .set_config(profile, "app_key", "test_key")
        .await
        .unwrap();
    vault
        .set_secret(profile, "app_secret", "test_secret")
        .await
        .unwrap();
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
    // Given: The cowen daemon is running and listening to Open Platform
    let mut cmd_start = Command::cargo_bin("cowen").unwrap();
    cmd_start.env("COWEN_HOME", &home);
    cmd_start.env("HOME", &home);
    cmd_start.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_start.env("COWEN_SKIP_DAEMON_RECOVERY", "true");

    // In dev environment, we explicitly point COWEN_DAEMON_BIN to the compiled executable if needed,
    // but the CLI might find it automatically via relative paths. Let's provide COWEN_DAEMON_PATH just in case.
    let bin_path = std::env::current_dir()
        .unwrap()
        .join("../../bin/macos-aarch64/cowen-daemon");
    cmd_start.env("COWEN_DAEMON_PATH", bin_path.to_str().unwrap());

    cmd_start
        .arg("--profile")
        .arg(profile)
        .arg("daemon")
        .arg("start");

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
    cmd_stop
        .arg("--profile")
        .arg(profile)
        .arg("daemon")
        .arg("stop");
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

fn setup_daemon_env_https(
    profile: &str,
    mode: &str,
) -> (
    tempfile::TempDir,
    String,
    crate::e2e::rust::common::DaemonKiller,
) {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    fs::create_dir_all(&cowen_home).unwrap();

    let proxy_port = get_next_port();

    // Create profile config
    let config_path = cowen_home.join(format!("{}.yaml", profile));
    let config = json!({
        "app_key": "test_key",
        "app_mode": mode,
        "encrypt_key": "1234567890123456", // 16-byte dummy key to pass validation rules
        "webhook_target": "http://localhost:8080",
        "auto_start": false,
        "proxy_port": proxy_port,
        "version": 1
    });
    fs::write(config_path, serde_yaml::to_string(&config).unwrap()).unwrap();

    let port = get_next_port();

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

    (
        dir,
        cowen_home.to_str().unwrap().to_string(),
        crate::e2e::rust::common::DaemonKiller {
            home: cowen_home.to_str().unwrap().to_string(),
        },
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn test_daemon_https_crash_prevention() {
    let profile = "test_selfbuilt_daemon_https";
    let (dir, home, _killer) = setup_daemon_env_https(profile, "self-built");

    // Seed dummy token in vault to pass auth checks on startup
    let app_cfg = cowen_common::config::AppConfig {
        openapi_url: "https://localhost:12345".to_string(),
        stream_url: "https://localhost:12345".to_string(),
        ..Default::default()
    };
    let vault =
        cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint")
            .await
            .unwrap();
    vault
        .set_config(profile, "app_key", "test_key")
        .await
        .unwrap();
    vault
        .set_secret(profile, "app_secret", "test_secret")
        .await
        .unwrap();
    vault
        .set_secret(profile, "encrypt_key", "1234567890123456")
        .await
        .unwrap();

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

    // Given: The cowen daemon is running and listening to Open Platform
    let mut cmd_start = Command::cargo_bin("cowen").unwrap();
    cmd_start.env("COWEN_HOME", &home);
    cmd_start.env("HOME", &home);
    cmd_start.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_start.env("COWEN_SKIP_DAEMON_RECOVERY", "true");

    // Explicitly point COWEN_DAEMON_PATH to the compiled executable in workspace bin
    let bin_path = std::env::current_dir()
        .unwrap()
        .join("../../bin/macos-aarch64/cowen-daemon");
    cmd_start.env("COWEN_DAEMON_PATH", bin_path.to_str().unwrap());

    cmd_start
        .arg("--profile")
        .arg(profile)
        .arg("daemon")
        .arg("start");

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
    cmd_stop
        .arg("--profile")
        .arg(profile)
        .arg("daemon")
        .arg("stop");
    let _ = cmd_stop.output();

    let log_path = std::path::Path::new(&home).join("logs/daemon.stderr.log");
    let log_content = if log_path.exists() {
        fs::read_to_string(&log_path).unwrap_or_default()
    } else {
        "No log file".to_string()
    };

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

#[tokio::test(flavor = "multi_thread")]
async fn test_daemon_reset_and_status_empty_profiles() {
    let profile = "test_reset_profile";
    let (_dir, home, _killer) = setup_daemon_env(profile, "self-built");

    // Seed dummy token in vault to pass auth checks on startup
    let app_cfg = cowen_common::config::AppConfig {
        openapi_url: "http://localhost:12345".to_string(),
        stream_url: "http://localhost:12345".to_string(),
        ..Default::default()
    };
    let vault =
        cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint")
            .await
            .unwrap();
    vault
        .set_config(profile, "app_key", "test_key")
        .await
        .unwrap();
    vault
        .set_secret(profile, "app_secret", "test_secret")
        .await
        .unwrap();

    // Given: The cowen daemon is running and listening to Open Platform
    let mut cmd_start = Command::cargo_bin("cowen").unwrap();
    cmd_start.env("COWEN_HOME", &home);
    cmd_start.env("HOME", &home);
    cmd_start.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_start.env("COWEN_SKIP_DAEMON_RECOVERY", "true");

    let bin_path = std::env::current_dir()
        .unwrap()
        .join("../../bin/macos-aarch64/cowen-daemon");
    cmd_start.env("COWEN_DAEMON_PATH", bin_path.to_str().unwrap());

    cmd_start
        .arg("--profile")
        .arg(profile)
        .arg("daemon")
        .arg("start");
    let _ = cmd_start.output().unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // 2. Write empty status file to test Profile: '' bug
    let empty_status_file = std::path::Path::new(&home).join("_status.json");
    fs::write(empty_status_file, "{}").unwrap();

    // 3. Reset profile
    let mut cmd_reset = Command::cargo_bin("cowen").unwrap();
    cmd_reset.env("COWEN_HOME", &home);
    cmd_reset.env("HOME", &home);
    cmd_reset.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_reset.env("COWEN_SKIP_DAEMON_RECOVERY", "true");
    cmd_reset.arg("reset").arg("-p").arg(profile);
    let reset_out = cmd_reset.output().unwrap();
    assert!(
        reset_out.status.success(),
        "Reset should succeed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&reset_out.stdout),
        String::from_utf8_lossy(&reset_out.stderr)
    );

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // 4. Status --all
    let mut cmd_status = Command::cargo_bin("cowen").unwrap();
    cmd_status.env("COWEN_HOME", &home);
    cmd_status.env("HOME", &home);
    cmd_status.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_status.env("COWEN_SKIP_DAEMON_RECOVERY", "true");
    cmd_status.arg("status").arg("--all");
    let status_out = cmd_status.output().unwrap();
    let status_str = String::from_utf8_lossy(&status_out.stdout);

    // Profile should not exist anymore, and no empty profile
    assert!(
        !status_str.contains(&format!("Profile: '{}'", profile)),
        "Profile should be physically deleted"
    );
    assert!(
        !status_str.contains("Profile: ''"),
        "Empty profile should not exist"
    );

    // 5. Cleanup
    let mut cmd_stop = Command::cargo_bin("cowen").unwrap();
    cmd_stop.env("COWEN_HOME", &home);
    cmd_stop.env("HOME", &home);
    cmd_stop
        .arg("--profile")
        .arg(profile)
        .arg("daemon")
        .arg("stop");
    let _ = cmd_stop.output();
}

#[tokio::test]
async fn test_webhook_forwarding() {
    // Given: A mocked Open Platform environment and a webhook sink
    let (mock_port, _state) = crate::e2e::rust::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);
    let sink_url = format!("{}/webhook_sink", mock_url);
    let profile = "fwd";
    let dir = tempfile::tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    std::fs::create_dir_all(&cowen_home).unwrap();
    let home_str = cowen_home.to_str().unwrap().to_string();

    // Given: An initialized cowen environment using 'self-built' mode
    let mut cmd_init = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_init.env("COWEN_HOME", &home_str);
    cmd_init.env("HOME", &home_str);
    cmd_init.args([
        "init",
        "--profile",
        profile,
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_FWD",
        "--app-secret",
        "AS_FWD",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "CERT_FWD",
        "--webhook-target",
        &sink_url,
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
    ]);
    cmd_init.assert().success();

    // Given: The cowen daemon is running and listening to Open Platform
    let mut cmd_start = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_start.env("COWEN_HOME", &home_str);
    cmd_start.env("HOME", &home_str);
    cmd_start.args(["daemon", "start", "--profile", profile, "--all"]);
    cmd_start.assert().success();

    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    // When: A webhook event is pushed from Open Platform to the daemon via WebSocket
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "msg_type": "DATA_PUSH",
        "msgType": "DATA_PUSH",
        "msgId": "mock_broadcast_123",
        "appKey": "AK_FWD",
        "biz_content": {
            "orderId": "ORD123",
            "amount": "99.9"
        },
        "bizContent": {
            "orderId": "ORD123",
            "amount": "99.9"
        }
    });

    let res = client
        .post(format!("{}/control/broadcast", mock_url))
        .json(&payload)
        .send()
        .await
        .expect("Failed to broadcast");
    assert!(res.status().is_success());

    // Then: The event should be correctly forwarded to the configured local webhook sink
    let mut found = false;
    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let res = client
            .get(format!("{}/control/webhooks", mock_url))
            .send()
            .await
            .expect("Failed to get webhooks");
        let body: serde_json::Value = res.json().await.unwrap();
        if let Some(arr) = body.as_array() {
            if arr
                .iter()
                .any(|v| v.get("msgType").and_then(|t| t.as_str()) == Some("DATA_PUSH"))
            {
                found = true;
                break;
            }
        }
    }

    let _killer = crate::e2e::rust::common::DaemonKiller {
        home: cowen_home.to_str().unwrap().to_string(),
    };
    if !found {
        let log_path = cowen_home.join("logs/daemon.stderr.log");
        let stdout_path = cowen_home.join("logs/daemon.stdout.log");
        let err_log = std::fs::read_to_string(log_path).unwrap_or_default();
        let out_log = std::fs::read_to_string(stdout_path).unwrap_or_default();
        println!("DAEMON STDOUT:\n{}", out_log);
        println!("DAEMON STDERR:\n{}", err_log);
        panic!("Webhook NOT found at sink");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_dlq_retries() {
    let (mock_port, _state) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);
    let bad_sink = "http://127.0.0.1:9999/broken".to_string();
    let profile = "dlq_prof";
    let dir = tempfile::tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    std::fs::create_dir_all(&cowen_home).unwrap();
    let home_str = cowen_home.to_str().unwrap().to_string();

    // Given: An initialized cowen environment with a BROKEN webhook sink
    let mut cmd_init = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_init.env("COWEN_HOME", &home_str);
    cmd_init.env("HOME", &home_str);
    cmd_init.args([
        "init",
        "--profile",
        profile,
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_DLQ",
        "--app-secret",
        "AS_DLQ",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "CERT_DLQ",
        "--webhook-target",
        &bad_sink,
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
    ]);
    cmd_init.assert().success();

    // Start daemon
    let mut cmd_start = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_start.env("COWEN_HOME", &home_str);
    cmd_start.env("HOME", &home_str);
    cmd_start.args(["daemon", "start", "--profile", profile, "--all"]);
    cmd_start.assert().success();

    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    // Login (Auth)
    let mut cmd_auth = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_auth.env("COWEN_HOME", &home_str);
    cmd_auth.env("HOME", &home_str);
    cmd_auth.args(["auth", "login", "--profile", profile, "--force"]);
    cmd_auth.assert().success();

    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    // When: A DLQ_TRIGGER event is pushed via WS but fails to forward
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "msg_type": "DLQ_TRIGGER",
        "msgType": "DLQ_TRIGGER",
        "msgId": "mock_dlq_123",
        "appKey": "AK_DLQ",
        "biz_content": { "test": "fail" },
        "bizContent": { "test": "fail" },
        "time": "2026-06-26 12:00:00"
    });

    let res = client
        .post(format!("{}/control/broadcast", mock_url))
        .json(&payload)
        .send()
        .await
        .expect("Failed to broadcast");
    assert!(res.status().is_success());

    // Then: The message should eventually be recorded in the DLQ
    let mut found = false;
    for _ in 0..15 {
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        let mut cmd_dlq = assert_cmd::Command::cargo_bin("cowen").unwrap();
        cmd_dlq.env("COWEN_HOME", &home_str);
        cmd_dlq.env("HOME", &home_str);
        cmd_dlq.args(["dlq", "list", "--profile", profile]);

        let output = cmd_dlq.output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.contains("DLQ_TRIGGER") {
            found = true;
            break;
        }
    }

    let _killer = crate::e2e::rust::common::DaemonKiller {
        home: cowen_home.to_str().unwrap().to_string(),
    };

    if !found {
        let log_path = cowen_home.join("logs/daemon.stderr.log");
        let stdout_path = cowen_home.join("logs/daemon.stdout.log");
        let err_log = std::fs::read_to_string(log_path).unwrap_or_default();
        let out_log = std::fs::read_to_string(stdout_path).unwrap_or_default();
        println!("DAEMON STDOUT:\n{}", out_log);
        println!("DAEMON STDERR:\n{}", err_log);
        panic!("Message NOT found in DLQ");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_daemon_reconnect_resilience() {
    let (mock_port, _state) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);
    let webhook_sink = format!("http://127.0.0.1:{}/webhook_sink", mock_port);
    let profile = "reconnect_test";

    let dir = tempfile::tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    std::fs::create_dir_all(&cowen_home).unwrap();
    let home_str = cowen_home.to_str().unwrap().to_string();

    // 1. Initialization
    let mut cmd_init = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_init.env("COWEN_HOME", &home_str);
    cmd_init.env("HOME", &home_str);
    cmd_init.args([
        "init",
        "--profile",
        profile,
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_RECONNECT",
        "--app-secret",
        "AS_RECONNECT",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "CERT_RECONNECT",
        "--webhook-target",
        &webhook_sink,
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
    ]);
    cmd_init.assert().success();

    // 2. Start Daemon & Establish Connection
    let mut cmd_start = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_start.env("COWEN_HOME", &home_str);
    cmd_start.env("HOME", &home_str);
    cmd_start.args(["daemon", "start", "--profile", profile, "--all"]);
    cmd_start.assert().success();

    // Wait for bridge connection
    let mut connected = false;
    for _ in 0..15 {
        let mut cmd_status = assert_cmd::Command::cargo_bin("cowen").unwrap();
        cmd_status.env("COWEN_HOME", &home_str);
        cmd_status.env("HOME", &home_str);
        cmd_status.args(["status"]);
        let out = cmd_status.output().unwrap();
        let out_str = String::from_utf8_lossy(&out.stdout);
        if out_str.contains("Connected") {
            connected = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }
    assert!(connected, "Daemon failed to connect initially");

    // 3. Simulate Service Rolling Update (Force Close WS)
    let client = reqwest::Client::new();
    client
        .post(format!("{}/control/kill_connections", mock_url))
        .send()
        .await
        .unwrap();

    // Verify it's disconnected or reconnecting
    let mut detected = false;
    for _ in 0..30 {
        let mut cmd_status = assert_cmd::Command::cargo_bin("cowen").unwrap();
        cmd_status.env("COWEN_HOME", &home_str);
        cmd_status.env("HOME", &home_str);
        cmd_status.args(["status"]);
        let out = cmd_status.output().unwrap();
        let out_str = String::from_utf8_lossy(&out.stdout);
        if out_str.contains("Disconnected")
            || out_str.contains("Reconnecting")
            || out_str.contains("Connecting")
        {
            detected = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }
    assert!(
        detected,
        "Daemon still thinks it's connected after mock server killed WS!"
    );

    // 4. Verify Automatic Reconnection
    let mut reconnected = false;
    for _ in 0..20 {
        let mut cmd_status = assert_cmd::Command::cargo_bin("cowen").unwrap();
        cmd_status.env("COWEN_HOME", &home_str);
        cmd_status.env("HOME", &home_str);
        cmd_status.args(["status"]);
        let out = cmd_status.output().unwrap();
        let out_str = String::from_utf8_lossy(&out.stdout);
        if out_str.contains("Connected") && !out_str.contains("Disconnected") {
            reconnected = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }
    assert!(reconnected, "Daemon failed to reconnect automatically!");

    // 5. Functional Check after Reconnection
    client.post(format!("{}/control/broadcast", mock_url))
        .json(&serde_json::json!({"msg_type": "RECONNECT_TEST", "payload": {"status": "ok_after_retry"}}))
        .send().await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(4000)).await;

    let res = client
        .get(format!("{}/control/webhooks", mock_url))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        res.contains("RECONNECT_TEST"),
        "Failed to receive message after reconnection"
    );

    // Cleanup: stop daemon
    let mut cmd_stop = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_stop.env("COWEN_HOME", &home_str);
    cmd_stop.env("HOME", &home_str);
    cmd_stop.args(["daemon", "stop", "--profile", profile]);
    cmd_stop.assert().success();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_proxy_interception() {
    let (mock_port, _state) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);
    let proxy_port = get_next_port();
    let profile = "pxt";

    let dir = tempfile::tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    std::fs::create_dir_all(&cowen_home).unwrap();
    let home_str = cowen_home.to_str().unwrap().to_string();

    // 1. Initialization
    let mut cmd_init = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_init.env("COWEN_HOME", &home_str);
    cmd_init.env("HOME", &home_str);
    cmd_init.args([
        "init",
        "--profile",
        profile,
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_PXT",
        "--app-secret",
        "AS_PXT",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "CERT_PXT",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--proxy-port",
        &proxy_port.to_string(),
    ]);
    cmd_init.assert().success();

    // 2. Start Daemon
    let mut cmd_start = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_start.env("COWEN_HOME", &home_str);
    cmd_start.env("HOME", &home_str);
    cmd_start.args(["daemon", "start", "--profile", profile]);
    cmd_start.assert().success();

    // Wait for daemon to be ready (wait for proxy port to open)
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    // 3. Login to acquire a token
    let mut cmd_login = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_login.env("COWEN_HOME", &home_str);
    cmd_login.env("HOME", &home_str);
    cmd_login.args(["auth", "login", "--profile", profile, "--force"]);
    cmd_login.assert().success();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let client = reqwest::Client::new();

    // Call mock API through local proxy (port proxy_port)
    let proxy_url = format!("http://127.0.0.1:{}/v1/mock/secure", proxy_port);
    let resp = client.get(&proxy_url).send().await.unwrap();
    assert!(resp.status().is_success());
    let resp_text = resp.text().await.unwrap();
    assert!(
        resp_text.contains("verified"),
        "Proxy failed to inject token and forward properly"
    );

    // 4. Whitelist Enforcement
    let fail_url = format!("http://127.0.0.1:{}/v1/unauthorized/path", proxy_port);
    let resp_fail = client.get(&fail_url).send().await.unwrap();
    assert_eq!(
        resp_fail.status(),
        404,
        "Whitelist not enforced, expected 404"
    );

    // Cleanup: stop daemon
    let mut cmd_stop = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_stop.env("COWEN_HOME", &home_str);
    cmd_stop.env("HOME", &home_str);
    cmd_stop.args(["daemon", "stop", "--profile", profile]);
    cmd_stop.assert().success();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_token_lifecycle_transparent_refresh() {
    let (mock_port, _state) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);
    let proxy_port = get_next_port();
    let profile = "life";

    let dir = tempfile::tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    std::fs::create_dir_all(&cowen_home).unwrap();
    let home_str = cowen_home.to_str().unwrap().to_string();

    // 1. Initialization
    let mut cmd_init = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_init.env("COWEN_HOME", &home_str);
    cmd_init.env("HOME", &home_str);
    cmd_init.args([
        "init",
        "--profile",
        profile,
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_LIFE",
        "--app-secret",
        "AS_LIFE",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "CERT_LIFE",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--proxy-port",
        &proxy_port.to_string(),
    ]);
    cmd_init.assert().success();

    // 2. Start Daemon
    let mut cmd_start = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_start.env("COWEN_HOME", &home_str);
    cmd_start.env("HOME", &home_str);
    cmd_start.args(["daemon", "start", "--profile", profile]);
    cmd_start.assert().success();

    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    // 3. Login to acquire a token
    let mut cmd_login = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_login.env("COWEN_HOME", &home_str);
    cmd_login.env("HOME", &home_str);
    cmd_login.args(["auth", "login", "--profile", profile, "--force"]);
    cmd_login.assert().success();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // 4. Force token expiration via sqlite3
    let db_path = cowen_home.join("cowen.db");
    let update_query = "UPDATE cowen_app_token SET token_value = 'expired-token', expires_at = '2000-01-01T00:00:00Z' WHERE app_key = 'AK_LIFE';";
    let status = std::process::Command::new("sqlite3")
        .arg(db_path.to_str().unwrap())
        .arg(update_query)
        .status()
        .expect("Failed to run sqlite3");
    assert!(status.success(), "Failed to tamper with token in sqlite");

    // 5. Transparent Refresh via Proxy
    let client = reqwest::Client::new();
    let proxy_url = format!("http://127.0.0.1:{}/v1/mock/secure", proxy_port);
    let resp = client.get(&proxy_url).send().await.unwrap();

    assert!(resp.status().is_success());
    let resp_text = resp.text().await.unwrap();
    assert!(
        resp_text.contains("verified"),
        "Proxy failed to transparently refresh and forward: {}",
        resp_text
    );

    // Verify token is rotated (not expired-token)
    let output = std::process::Command::new("sqlite3")
        .arg(db_path.to_str().unwrap())
        .arg("SELECT token_value FROM cowen_app_token WHERE app_key = 'AK_LIFE';")
        .output()
        .expect("Failed to run sqlite3");
    let new_token = String::from_utf8_lossy(&output.stdout);
    assert!(
        !new_token.contains("expired-token"),
        "Token was not rotated in the database!"
    );

    // Cleanup: stop daemon
    let mut cmd_stop = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_stop.env("COWEN_HOME", &home_str);
    cmd_stop.env("HOME", &home_str);
    cmd_stop.args(["daemon", "stop", "--profile", profile]);
    cmd_stop.assert().success();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_concurrent_proxy_stress() {
    let (mock_port, _state) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);
    let proxy_port = get_next_port();
    let profile = "stress";

    let dir = tempfile::tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    std::fs::create_dir_all(&cowen_home).unwrap();
    let home_str = cowen_home.to_str().unwrap().to_string();

    // 1. Initialization
    let mut cmd_init = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_init.env("COWEN_HOME", &home_str);
    cmd_init.env("HOME", &home_str);
    cmd_init.args([
        "init",
        "--profile",
        profile,
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_STRESS",
        "--app-secret",
        "AS_STRESS",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "CERT_STRESS",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--proxy-port",
        &proxy_port.to_string(),
    ]);
    cmd_init.assert().success();

    // 2. Start Daemon
    let mut cmd_start = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_start.env("COWEN_HOME", &home_str);
    cmd_start.env("HOME", &home_str);
    cmd_start.args(["daemon", "start", "--profile", profile]);
    cmd_start.assert().success();

    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    // 3. Concurrent Proxy Requests (Stress)
    let mut handles = vec![];
    for _ in 0..20 {
        let proxy_url = format!("http://127.0.0.1:{}/v1/mock/ping", proxy_port);
        let handle = tokio::spawn(async move {
            let client = reqwest::Client::new();
            let resp = client.get(&proxy_url).send().await.unwrap();
            let status = resp.status();
            let text = resp.text().await.unwrap();
            assert!(
                status.is_success(),
                "Request failed with status {}: {}",
                status,
                text
            );
            text
        });
        handles.push(handle);
    }

    // Wait for all requests
    for handle in handles {
        handle.await.unwrap();
    }

    // 4. Audit Log Check
    // Allow daemon async task to flush vault sqlite writes
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    let mut cmd_audit = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_audit.env("COWEN_HOME", &home_str);
    cmd_audit.env("HOME", &home_str);
    cmd_audit.args(["log", "view", "audit", "--profile", profile, "-n", "50"]);

    let output = cmd_audit
        .output()
        .expect("Failed to execute cowen log view audit");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Request successfully proxied"),
        "Audit log did not contain proxy entries: {}",
        stdout
    );

    // Cleanup: stop daemon
    let mut cmd_stop = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_stop.env("COWEN_HOME", &home_str);
    cmd_stop.env("HOME", &home_str);
    cmd_stop.args(["daemon", "stop", "--profile", profile]);
    cmd_stop.assert().success();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_distributed_lb() {
    let (mock_port, _state) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);

    // Given: COWEN_HOME isolated paths for two nodes (node_a, node_b)
    let dir_a = tempfile::tempdir().unwrap();
    let home_a = dir_a.path().join("node_a");
    std::fs::create_dir_all(&home_a).unwrap();
    let home_a_str = home_a.to_str().unwrap().to_string();

    let dir_b = tempfile::tempdir().unwrap();
    let home_b = dir_b.path().join("node_b");
    std::fs::create_dir_all(&home_b).unwrap();
    let home_b_str = home_b.to_str().unwrap().to_string();

    let setup_node = |home_str: &str, proxy_port: u16| {
        let mut cmd_init = assert_cmd::Command::cargo_bin("cowen").unwrap();
        cmd_init.env("COWEN_HOME", home_str);
        cmd_init.env("HOME", home_str);
        cmd_init.args([
            "init",
            "--profile",
            "main",
            "--app-mode",
            "self-built",
            "--app-key",
            "AK_DIST",
            "--app-secret",
            "AS_DIST",
            "--encrypt-key",
            "1234567890123456",
            "--certificate",
            "CERT_DIST",
            "--openapi-url",
            &mock_url,
            "--stream-url",
            &mock_ws,
            "--webhook-target",
            &format!("{}/webhook_sink", mock_url),
            "--proxy-port",
            &proxy_port.to_string(),
        ]);
        cmd_init.assert().success();
    };

    setup_node(&home_a_str, get_next_port());
    setup_node(&home_b_str, get_next_port());

    // When: Both nodes run cowen daemon start
    let start_daemon = |home_str: &str| {
        let mut cmd_start = assert_cmd::Command::cargo_bin("cowen").unwrap();
        cmd_start.env("COWEN_HOME", home_str);
        cmd_start.env("HOME", home_str);
        cmd_start.env("COWEN_EXCLUSIVE", "false"); // Disable exclusive mode for concurrent nodes
        cmd_start.args(["daemon", "start", "--profile", "main"]);
        cmd_start.assert().success();
    };

    start_daemon(&home_a_str);
    start_daemon(&home_b_str);

    // Wait for 2 connections
    tokio::time::sleep(std::time::Duration::from_millis(3000)).await;

    // When: The server sends 10 messages with mode: "lb"
    let client = reqwest::Client::new();
    let broadcast_url = format!("{}/control/broadcast", mock_url);
    for i in 1..=10 {
        let payload = serde_json::json!({
            "msg_type": "DIST_TEST",
            "mode": "lb",
            "payload": {"seq": i}
        });
        client
            .post(&broadcast_url)
            .json(&payload)
            .send()
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    // Then: Both nodes successfully share the load. The webhook sink receives precisely 10 events.
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
    let webhooks_url = format!("{}/control/webhooks", mock_url);
    let resp = client.get(&webhooks_url).send().await.unwrap();
    let webhooks: Vec<serde_json::Value> = resp.json().await.unwrap();

    let lb_messages: Vec<_> = webhooks
        .into_iter()
        .filter(|w| w.get("msg_type").and_then(|v| v.as_str()) == Some("DIST_TEST"))
        .collect();

    assert_eq!(
        lb_messages.len(),
        10,
        "Expected exactly 10 webhooks in LB mode, got {}",
        lb_messages.len()
    );

    // Cleanup: stop daemons
    let stop_daemon = |home_str: &str| {
        let mut cmd_stop = assert_cmd::Command::cargo_bin("cowen").unwrap();
        cmd_stop.env("COWEN_HOME", home_str);
        cmd_stop.env("HOME", home_str);
        cmd_stop.args(["daemon", "stop", "--profile", "main"]);
        cmd_stop.assert().success();
    };

    stop_daemon(&home_a_str);
    stop_daemon(&home_b_str);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_shared_storage() {
    let (mock_port, _state) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);

    let proxy_port_1 = get_next_port();
    let proxy_port_2 = get_next_port();

    let dir = tempfile::tempdir().unwrap();
    let shared_db = dir.path().join("shared_db.sqlite");
    let shared_db_url = format!(
        "sqlite://{}",
        shared_db.to_str().unwrap().replace("\\", "/")
    );

    // Given: Node 1 initialized with shared db
    let home_1 = dir.path().join("node_1");
    std::fs::create_dir_all(&home_1).unwrap();
    let home_1_str = home_1.to_str().unwrap().to_string();

    let app_config_1 = serde_json::json!({
        "storage": {
            "store": "sqlite",
            "db_url": &shared_db_url
        },
        "log": {"level": "debug"},
        "openapi_url": &mock_url,
        "stream_url": &mock_ws,
        "telemetry_enabled": false,
        "ai_enabled": false
    });
    std::fs::write(
        home_1.join("app.yaml"),
        serde_yaml::to_string(&app_config_1).unwrap(),
    )
    .unwrap();

    let mut cmd_init = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_init.env("COWEN_HOME", &home_1_str);
    cmd_init.env("HOME", &home_1_str);
    cmd_init.args([
        "init",
        "--profile",
        "main",
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_SYNC",
        "--app-secret",
        "AS_SYNC",
        "--certificate",
        "CERT_SYNC",
        "--encrypt-key",
        "1234567890123456",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--proxy-port",
        &proxy_port_1.to_string(),
    ]);
    cmd_init.assert().success();

    // Wait for Token 1
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
    let mut cmd_token_1 = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_token_1.env("COWEN_HOME", &home_1_str);
    cmd_token_1.env("HOME", &home_1_str);
    cmd_token_1.env("COWEN_RAW_OUTPUT", "true");
    cmd_token_1.args(["auth", "token", "--profile", "main", "--format", "json"]);
    let out_1 = String::from_utf8_lossy(&cmd_token_1.output().unwrap().stdout).to_string();
    assert!(
        out_1.contains("mock_at_sb_"),
        "Node 1 did not acquire token: {}",
        out_1
    );

    let token_1: serde_json::Value = serde_json::from_str(&out_1).unwrap();
    let token_1_val = token_1["access_token"].as_str().unwrap().to_string();

    // When: Node 2 starts without init but points to the same shared DB
    let home_2 = dir.path().join("node_2");
    std::fs::create_dir_all(&home_2).unwrap();
    let home_2_str = home_2.to_str().unwrap().to_string();

    let app_config_2 = serde_json::json!({
        "storage": {
            "store": "sqlite",
            "db_url": &shared_db_url
        },
        "log": {"level": "debug"},
        "openapi_url": &mock_url,
        "stream_url": &mock_ws,
        "telemetry_enabled": false,
        "ai_enabled": false
    });
    std::fs::write(
        home_2.join("app.yaml"),
        serde_yaml::to_string(&app_config_2).unwrap(),
    )
    .unwrap();

    let mut cmd_start_2 = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_start_2.env("COWEN_HOME", &home_2_str);
    cmd_start_2.env("HOME", &home_2_str);
    cmd_start_2.env("COWEN_EXCLUSIVE", "false");
    cmd_start_2.args([
        "daemon",
        "start",
        "--profile",
        "main",
        "--proxy-port",
        &proxy_port_2.to_string(),
    ]);
    cmd_start_2.assert().success();

    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    // Then: Node 2 should read the same token from DB
    let mut token_2_val = String::new();
    for _ in 0..10 {
        let mut cmd_token_2 = assert_cmd::Command::cargo_bin("cowen").unwrap();
        cmd_token_2.env("COWEN_HOME", &home_2_str);
        cmd_token_2.env("HOME", &home_2_str);
        cmd_token_2.env("COWEN_RAW_OUTPUT", "true");
        cmd_token_2.args(["auth", "token", "--profile", "main", "--format", "json"]);
        let out_2 = String::from_utf8_lossy(&cmd_token_2.output().unwrap().stdout).to_string();
        if let Ok(token_2) = serde_json::from_str::<serde_json::Value>(&out_2) {
            if let Some(v) = token_2["access_token"].as_str() {
                token_2_val = v.to_string();
                if token_2_val == token_1_val {
                    break;
                }
            }
        }
        let mut cmd_reload = assert_cmd::Command::cargo_bin("cowen").unwrap();
        cmd_reload.env("COWEN_HOME", &home_2_str);
        cmd_reload.env("HOME", &home_2_str);
        cmd_reload.args(["daemon", "reload", "--profile", "main"]);
        let _ = cmd_reload.output();
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }
    assert_eq!(
        token_1_val, token_2_val,
        "Node 2 token did not sync to Node 1's token"
    );

    // When: Node 1 refreshes token
    let mut cmd_refresh_1 = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_refresh_1.env("COWEN_HOME", &home_1_str);
    cmd_refresh_1.env("HOME", &home_1_str);
    cmd_refresh_1.env("COWEN_RAW_OUTPUT", "true");
    cmd_refresh_1.args([
        "auth",
        "token",
        "--profile",
        "main",
        "--refresh",
        "--format",
        "json",
    ]);
    let out_ref_1 = String::from_utf8_lossy(&cmd_refresh_1.output().unwrap().stdout).to_string();
    let token_v2_json: serde_json::Value = serde_json::from_str(&out_ref_1).unwrap();
    let token_v2_val = token_v2_json["access_token"].as_str().unwrap().to_string();

    // Stop Node 1 to prevent it from refreshing again
    let mut cmd_stop_1 = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_stop_1.env("COWEN_HOME", &home_1_str);
    cmd_stop_1.env("HOME", &home_1_str);
    cmd_stop_1.args(["daemon", "stop", "--profile", "main"]);
    cmd_stop_1.assert().success();

    // Then: Node 2 token updates to the new token
    let mut token_2_v2_val = String::new();
    for _ in 0..10 {
        let mut cmd_token_2 = assert_cmd::Command::cargo_bin("cowen").unwrap();
        cmd_token_2.env("COWEN_HOME", &home_2_str);
        cmd_token_2.env("HOME", &home_2_str);
        cmd_token_2.env("COWEN_RAW_OUTPUT", "true");
        cmd_token_2.args(["auth", "token", "--profile", "main", "--format", "json"]);
        let out_2 = String::from_utf8_lossy(&cmd_token_2.output().unwrap().stdout).to_string();
        if let Ok(token_2) = serde_json::from_str::<serde_json::Value>(&out_2) {
            if let Some(v) = token_2["access_token"].as_str() {
                token_2_v2_val = v.to_string();
                if token_2_v2_val == token_v2_val {
                    break;
                }
            }
        }
        let mut cmd_reload = assert_cmd::Command::cargo_bin("cowen").unwrap();
        cmd_reload.env("COWEN_HOME", &home_2_str);
        cmd_reload.env("HOME", &home_2_str);
        cmd_reload.args(["daemon", "reload", "--profile", "main"]);
        let _ = cmd_reload.output();
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }
    assert_eq!(
        token_v2_val, token_2_v2_val,
        "Node 2 token did not sync to the refreshed Token V2"
    );

    // Then: Node 2 proxy uses the new token
    let client = reqwest::Client::new();
    let proxy_url = format!("http://127.0.0.1:{}/v1/mock/secure", proxy_port_2);
    let mut proxy_success = false;
    for _ in 0..10 {
        if let Ok(resp) = client.get(&proxy_url).send().await {
            if resp.status().is_success() {
                proxy_success = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }
    assert!(proxy_success, "Node 2 Proxy unreachable");

    // Stop Node 2
    let mut cmd_stop_2 = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_stop_2.env("COWEN_HOME", &home_2_str);
    cmd_stop_2.env("HOME", &home_2_str);
    cmd_stop_2.args(["daemon", "stop", "--profile", "main"]);
    cmd_stop_2.assert().success();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_store_app_shared_storage() {
    let (mock_port, _state) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);

    let dir = tempdir().unwrap();
    let home_1 = dir.path().join("node_1");
    let home_2 = dir.path().join("node_2");
    let shared_db = dir.path().join("store_app_shared.db");

    let home_1_str = home_1.to_str().unwrap();
    let home_2_str = home_2.to_str().unwrap();

    std::fs::create_dir_all(&home_1).unwrap();
    let app_yaml_1 = serde_json::json!({
        "storage": {
            "store": "sqlite",
            "db_url": format!("sqlite://{}", shared_db.to_str().unwrap()),
        },
        "log": { "level": "debug" },
        "openapi_url": mock_url,
        "stream_url": mock_ws,
        "telemetry_enabled": false,
        "ai_enabled": false,
    });
    std::fs::write(
        home_1.join("app.yaml"),
        serde_yaml::to_string(&app_yaml_1).unwrap(),
    )
    .unwrap();

    let proxy_port_1 = get_next_port();
    let mut cmd_init = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_init.env("COWEN_HOME", home_1_str);
    cmd_init.env("HOME", home_1_str);
    cmd_init.args([
        "init",
        "--profile",
        "main",
        "--app-mode",
        "store-app",
        "--app-key",
        "AK_STORE",
        "--app-secret",
        "AS_STORE",
        "--encrypt-key",
        "1234567890123456",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--webhook-target",
        "http://127.0.0.1:9299/webhook_sink",
        "--proxy-port",
        &proxy_port_1.to_string(),
    ]);
    cmd_init.assert().success();

    std::fs::create_dir_all(&home_2).unwrap();
    let app_yaml_2 = serde_json::json!({
        "storage": {
            "store": "sqlite",
            "db_url": format!("sqlite://{}", shared_db.to_str().unwrap()),
        },
        "log": { "level": "debug" },
        "openapi_url": mock_url,
        "stream_url": mock_ws,
        "telemetry_enabled": false,
        "ai_enabled": false,
    });
    std::fs::write(
        home_2.join("app.yaml"),
        serde_yaml::to_string(&app_yaml_2).unwrap(),
    )
    .unwrap();

    let proxy_port_2 = get_next_port();
    let cowen_bin = assert_cmd::cargo::cargo_bin("cowen");
    let mut cmd_daemon = std::process::Command::new(cowen_bin);
    cmd_daemon.env("COWEN_HOME", home_2_str);
    cmd_daemon.env("HOME", home_2_str);
    cmd_daemon.args([
        "daemon",
        "start",
        "--profile",
        "main",
        "--proxy-port",
        &proxy_port_2.to_string(),
        "--foreground",
    ]);

    let mut daemon_child = cmd_daemon.spawn().unwrap();

    let client = reqwest::Client::new();
    let webhook_url = format!("http://127.0.0.1:{}/webhook", proxy_port_2);
    let mut proxy_up = false;
    for _ in 0..20 {
        if client.post(&webhook_url).send().await.is_ok() {
            proxy_up = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    assert!(proxy_up, "Node 2 proxy port did not start");

    let ticket_val = format!("mock_ticket_{}", chrono::Utc::now().timestamp());
    let mut webhook_success = false;
    for _ in 0..5 {
        if let Ok(resp) = client
            .post(&webhook_url)
            .json(&serde_json::json!({"type": "APP_TICKET", "app_ticket": ticket_val}))
            .send()
            .await
        {
            if resp.status().is_success() {
                webhook_success = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }
    assert!(webhook_success, "Failed to send webhook to Node 2");

    tokio::time::sleep(std::time::Duration::from_millis(3000)).await;

    let mut ticket_synced = false;
    for _ in 0..10 {
        let mut cmd_status = assert_cmd::Command::cargo_bin("cowen").unwrap();
        cmd_status.env("COWEN_HOME", home_1_str);
        cmd_status.env("HOME", home_1_str);
        cmd_status.args(["auth", "status", "--profile", "main", "--format", "json"]);
        let output = cmd_status.output().unwrap();
        let out_str = String::from_utf8_lossy(&output.stdout).to_string();
        if out_str.contains("[CACHED]") {
            ticket_synced = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
    }
    assert!(ticket_synced, "Node 1 did not see the synced ticket");

    let mut cmd_token = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_token.env("COWEN_HOME", home_2_str);
    cmd_token.env("HOME", home_2_str);
    cmd_token.env("COWEN_RAW_OUTPUT", "true");
    cmd_token.args(["auth", "token", "--profile", "main", "--format", "json"]);
    let output = cmd_token.output().unwrap();
    let out_str = String::from_utf8_lossy(&output.stdout).to_string();
    let token_json: serde_json::Value = serde_json::from_str(&out_str).unwrap();
    let token_val = token_json["access_token"].as_str().unwrap_or("");
    assert!(
        token_val.starts_with("mock_at_sa_"),
        "Node 2 token is invalid: {}",
        token_val
    );

    let mut cmd_stop_2 = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_stop_2.env("COWEN_HOME", home_2_str);
    cmd_stop_2.env("HOME", home_2_str);
    cmd_stop_2.args(["daemon", "stop", "--profile", "main"]);
    let _ = cmd_stop_2.output();

    let _ = daemon_child.kill();
    let _ = daemon_child.wait();
}

#[tokio::test]
async fn test_graceful_shutdown_drain() {
    let (mock_port, _mock_guard) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);

    let (_dir, home, _killer) = setup_daemon_env("main", &mock_url);

    // 1. Configure delay in mock server to simulate slow webhook processing
    let client = reqwest::Client::new();
    client
        .post(format!("{}/control/config", mock_url))
        .json(&serde_json::json!({"webhook_delay_ms": 3000}))
        .send()
        .await
        .unwrap();

    // 2. Initialize profile
    let mut init_cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &home);
    init_cmd.env("HOME", &home);
    init_cmd.args([
        "init",
        "--profile",
        "main",
        "--app-mode",
        "self-built",
        "--app-key",
        "test_key_shutdown",
        "--app-secret",
        "test_secret_shutdown",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "test_cert",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &format!("ws://127.0.0.1:{}", mock_port),
        "--webhook-target",
        &format!("{}/webhook_sink", mock_url),
    ]);
    init_cmd.assert().success();

    // 3. Start daemon
    let home_clone = home.clone();
    let mut daemon_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd.env("COWEN_HOME", &home_clone);
    daemon_cmd.env("HOME", &home_clone);
    daemon_cmd.args(["daemon", "start", "--profile", "main"]);
    let mut daemon_child = daemon_cmd.spawn().unwrap();
    let _ = daemon_child.wait(); // Reap the launcher process

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // 4. Trigger High-Latency Forwarding
    client
        .post(format!("{}/control/broadcast", mock_url))
        .header("appKey", "test_key_shutdown")
        .json(&serde_json::json!({
            "msg_type": "DATA_PUSH",
            "payload": {"some_data": "value_for_shutdown_test"}
        }))
        .send()
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // 5. Send Stop Command
    let mut stop_cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    stop_cmd.env("COWEN_HOME", &home);
    stop_cmd.env("HOME", &home);
    stop_cmd.args(["daemon", "stop", "--profile", "main"]);
    let _ = stop_cmd.assert().success();

    // 6. Wait for daemon to drain (up to 10s)
    let log_file = format!("{}/logs/daemon.stdout.log", home);
    let mut graceful_exit = false;
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if let Ok(content) = std::fs::read_to_string(&log_file) {
            if content.contains("Worker stopped gracefully")
                || content.contains("All active tasks completed gracefully")
                || content.contains("Timeout waiting for active tasks")
            {
                graceful_exit = true;
                break;
            }
        }
    }

    assert!(graceful_exit, "Failed to find graceful exit logs");

    let log_content = std::fs::read_to_string(&log_file).unwrap_or_default();

    assert!(
        log_content.contains("Stopping worker (Draining)")
            || log_content.contains("Shutdown signal received")
            || log_content.contains("Waiting for active tasks to complete"),
        "Missing drain logs. Actual logs:\n{}",
        log_content
    );

    // 7. Verify Log Separation
    let stderr_file = format!("{}/logs/daemon.stderr.log", home);
    let stderr_content = std::fs::read_to_string(&stderr_file).unwrap_or_default();
    assert!(!stderr_content.contains(" INFO "), "Log separation broken");
    assert!(
        !stderr_content.contains("\"msg_type\":\"ping\""),
        "Ping logged to stderr"
    );
}

#[tokio::test]
async fn test_daemon_lifecycle_race() {
    let (_dir, home, _killer) = setup_daemon_env("main", "http://127.0.0.1:8080");

    let mut init_cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &home);
    init_cmd.env("HOME", &home);
    init_cmd.args([
        "init",
        "--profile",
        "main",
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_SB",
        "--app-secret",
        "AS_SB",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "CERT_SB",
        "--webhook-target",
        "http://127.0.0.1:8080/cb",
    ]);
    init_cmd.assert().success();

    // Foreground start (launchd simulation)
    let home_clone = home.clone();
    let mut fg_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    fg_cmd.env("COWEN_HOME", &home_clone);
    fg_cmd.env("HOME", &home_clone);
    fg_cmd.env("COWEN_ALLOW_PORT_FALLBACK", "0");
    fg_cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "true");
    fg_cmd.args(["daemon", "start", "--profile", "main", "--foreground"]);
    let mut fg_child = fg_cmd.spawn().unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    let pid_file = format!("{}/master_daemon.pid", home);
    let original_pid = std::fs::read_to_string(&pid_file).unwrap_or_default();
    assert!(!original_pid.is_empty());

    // Concurrent background start
    let mut bg_cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    bg_cmd.env("COWEN_HOME", &home);
    bg_cmd.env("HOME", &home);
    bg_cmd.env("COWEN_ALLOW_PORT_FALLBACK", "0");
    bg_cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "true");
    bg_cmd.args(["daemon", "start", "--profile", "main"]);

    // Background start should either succeed (gracefully realize it's running) or just connect.
    // It shouldn't crash the foreground.
    bg_cmd.assert().success();

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Verify PID is identical
    let actual_pid = std::fs::read_to_string(&pid_file).unwrap_or_default();
    assert_eq!(
        original_pid, actual_pid,
        "Daemon PID changed during concurrent start!"
    );

    // Stop daemon
    let mut stop_cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    stop_cmd.env("COWEN_HOME", &home);
    stop_cmd.env("HOME", &home);
    stop_cmd.args(["daemon", "stop", "--profile", "main"]);
    stop_cmd.assert().success();

    let _ = fg_child.kill();
    let _ = fg_child.wait();
}
