use assert_cmd::Command;
use serde_json::json;
use std::fs;
use tempfile::tempdir;
use tokio::time::{sleep, Duration};

use super::mock_server::spawn_mock_server;

fn setup_chaos_env(
    _profile: &str,
    openapi_url: &str,
    stream_url: &str,
) -> (tempfile::TempDir, String) {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    let home = dir.path().join("home");
    fs::create_dir_all(&cowen_home).unwrap();
    fs::create_dir_all(&home).unwrap();

    let app_config_path = cowen_home.join("app.yaml");
    let app_config = json!({
        "openapi_url": openapi_url,
        "stream_url": stream_url,
        "telemetry_enabled": false,
        "log": {
            "level": "info",
        }
    });
    fs::write(app_config_path, serde_yaml::to_string(&app_config).unwrap()).unwrap();

    (dir, cowen_home.to_str().unwrap().to_string())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_chaos_stress_graceful_shutdown() {
    let (port, _state) = spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", port);
    let mock_ws = format!("ws://127.0.0.1:{}", port);
    let webhook_target = format!("{}/webhook_sink", mock_url);

    // Given: A daemon configured with self-built auth, connected to a slow webhook sink
    let client = reqwest::Client::new();
    client
        .post(format!("{}/control/config", mock_url))
        .json(&json!({"webhook_delay_ms": 1000}))
        .send()
        .await
        .unwrap();

    let profile = "chaos";
    let (_dir, cowen_home) = setup_chaos_env(profile, &mock_url, &mock_ws);

    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &cowen_home);
    init_cmd.env("COWEN_FS_FINGERPRINT", "chaos_fingerprint");
    init_cmd
        .arg("init")
        .arg("--profile")
        .arg(profile)
        .arg("--app-mode")
        .arg("self-built")
        .arg("--app-key")
        .arg("chaos_stress_key")
        .arg("--app-secret")
        .arg("chaos_stress_secret")
        .arg("--encrypt-key")
        .arg("1234567890123456")
        .arg("--certificate")
        .arg("chaos_cert")
        .arg("--webhook-target")
        .arg(&webhook_target);

    init_cmd.assert().success();

    // Remove IPC port if it exists to avoid race
    let _ = fs::remove_file(std::path::Path::new(&cowen_home).join("ipc.port"));

    // Start daemon
    let daemon_home = cowen_home.clone();
    let mut cmd = tokio::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    cmd.env("COWEN_HOME", &daemon_home);
    cmd.env("COWEN_FS_FINGERPRINT", "chaos_fingerprint");
    cmd.arg("daemon")
        .arg("start")
        .arg("--profile")
        .arg(profile)
        .arg("--foreground");

    let mut child = cmd.spawn().unwrap();

    // Give daemon time to start
    sleep(Duration::from_secs(3)).await;

    // When: 40 concurrent broadcasts are pushed via the websocket
    for i in 1..=40 {
        let mock_url_clone = mock_url.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let _ = client
                .post(format!("{}/control/broadcast", mock_url_clone))
                .header("appKey", "chaos_stress_key")
                .json(&json!({
                    "msg_type": "DATA_PUSH",
                    "payload": {
                        "index": i
                    }
                }))
                .send()
                .await;
        });
    }

    // Small sleep to let the messages hit the daemon and trigger forwarders
    sleep(Duration::from_millis(1500)).await;

    // Send SIGTERM signal
    let pid = child.id().unwrap() as i32;
    #[cfg(unix)]
    std::process::Command::new("kill")
        .arg("-15")
        .arg(pid.to_string())
        .status()
        .unwrap();
    #[cfg(windows)]
    let _ = child.kill(); // Fallback for Windows where SIGTERM is not supported natively in the same way

    // Then: The daemon must exit gracefully within the 25-second timeout
    let timeout_res = tokio::time::timeout(Duration::from_secs(25), child.wait()).await;
    assert!(timeout_res.is_ok(), "Daemon failed to exit within 25s");

    let status = timeout_res.unwrap().unwrap();
    // Verify it was successful (graceful shutdown)
    assert!(status.success(), "Daemon did not exit successfully");

    // Verify its SQLite and vault storage must remain uncorrupted (verified via cowen doctor)
    let mut doctor_cmd = Command::cargo_bin("cowen").unwrap();
    doctor_cmd.env("COWEN_HOME", &cowen_home);
    doctor_cmd.env("COWEN_FS_FINGERPRINT", "chaos_fingerprint");
    doctor_cmd.arg("doctor").arg("--fix");
    doctor_cmd.assert().success();

    // Verify its log must contain graceful shutdown markers
    let log_dir = std::path::Path::new(&cowen_home).join("logs");
    let mut found_marker = false;
    if log_dir.exists() {
        for entry in fs::read_dir(log_dir).unwrap().flatten() {
            if let Ok(contents) = fs::read_to_string(entry.path()) {
                if contents.contains("All active tasks completed gracefully")
                    || contents.contains("No active tasks, proceeding with shutdown")
                    || contents.contains("Shutdown signal received")
                {
                    found_marker = true;
                    break;
                }
            }
        }
    }

    assert!(found_marker, "Log missing shutdown protocol markers");
}
