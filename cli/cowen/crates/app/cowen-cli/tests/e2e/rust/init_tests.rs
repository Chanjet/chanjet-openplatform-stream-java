use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;

use super::common::setup_test_env;
use super::mock_server::spawn_mock_server;

#[tokio::test(flavor = "multi_thread")]
async fn test_init_self_built() {
    let (mock_port, _state) = spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);

    let (_temp, home, _killer) = setup_test_env("main", "self-built", &mock_url);

    // When initializing self-built app
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.args([
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
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--proxy-port",
        "8888",
    ]);

    // Then it should succeed
    cmd.assert().success();

    // When starting daemon
    let bin_path = std::env::current_dir()
        .unwrap()
        .join("../../bin/macos-aarch64/cowen-daemon");

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.env("COWEN_DAEMON_PATH", bin_path.to_str().unwrap());
    cmd.args(["daemon", "start", "--profile", "main"]);
    cmd.assert().success();

    // Wait for daemon to establish websocket
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // When triggering auth login via CLI
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.args(["auth", "login", "--profile", "main", "--force"]);
    cmd.assert().success();

    tokio::time::sleep(Duration::from_millis(500)).await;

    // When checking config
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.args(["config", "--profile", "main"]);

    // Then output must be sanitized
    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);

    assert!(!stdout.contains("AS_SB"), "Output contains raw app-secret!");
    assert!(
        stdout.contains("***"),
        "Output should contain sanitized app-secret"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_init_cleanup() {
    let (mock_port, _state) = spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);

    let (_temp, home, _killer) = setup_test_env("dummy", "self-built", &mock_url);

    // Scenario 1: Self-Built Missing Params
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.args([
        "init",
        "--profile",
        "test_sb",
        "--app-mode",
        "self-built",
        "--app-key",
        "some-key",
    ]);
    cmd.assert().failure(); // Missing app-secret

    let mut list_cmd = Command::cargo_bin("cowen").unwrap();
    list_cmd.env("COWEN_HOME", &home);
    list_cmd.args(["profile", "list"]);
    list_cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("test_sb").not());

    // Scenario 2: Store-App Missing Params
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.args([
        "init",
        "--profile",
        "test_store",
        "--app-mode",
        "store-app",
        "--app-key",
        "some-key",
    ]);
    cmd.assert().failure();

    let mut list_cmd = Command::cargo_bin("cowen").unwrap();
    list_cmd.env("COWEN_HOME", &home);
    list_cmd.args(["profile", "list"]);
    list_cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("test_store").not());

    // Scenario 4: Preservation of EXISTING Profiles
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.args([
        "init",
        "--profile",
        "test_existing",
        "--app-mode",
        "self-built",
        "--app-key",
        "K",
        "--app-secret",
        "S",
        "--certificate",
        "C",
        "--encrypt-key",
        "1234567890123456",
    ]);
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.args([
        "init",
        "--profile",
        "test_existing",
        "--app-mode",
        "self-built",
        "--app-key",
        "ONLY_KEY",
    ]);
    cmd.assert().success();

    let mut list_cmd = Command::cargo_bin("cowen").unwrap();
    list_cmd.env("COWEN_HOME", &home);
    list_cmd.args(["profile", "list"]);
    list_cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("test_existing"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_init_store_app() {
    let (mock_port, _state) = spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);

    let temp_dir = tempfile::tempdir().unwrap();
    let home = temp_dir.path().to_str().unwrap().to_string();
    let _killer =
        crate::e2e::rust::common::setup_test_env_in_dir("sidecar", "store-app", &mock_url, &home);

    // 1. Initialization
    let cowen_home = std::path::PathBuf::from(&home).join(".cowen");
    let _app_ticket_file = cowen_home.join("sidecar_status.json");
    std::fs::remove_file(cowen_home.join("profiles").join("sidecar.yaml")).ok();

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", cowen_home.to_str().unwrap());
    cmd.env("HOME", &home);
    cmd.args([
        "init",
        "--profile",
        "sidecar",
        "--app-mode",
        "store-app",
        "--app-key",
        "AK_SA",
        "--app-secret",
        "AS_SA",
        "--encrypt-key",
        "1234567890123456",
        "--webhook-target",
        "http://127.0.0.1:8080/cb",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--proxy-port",
        "8888",
    ]);
    cmd.assert().success();

    // 2. Daemon Startup
    let bin_path = std::path::PathBuf::from(&home)
        .join("bin")
        .join("cowen-daemon");

    let mut cmd_start = Command::cargo_bin("cowen").unwrap();
    cmd_start.env("COWEN_HOME", cowen_home.to_str().unwrap());
    cmd_start.env("HOME", &home);
    cmd_start.env("COWEN_DAEMON_BIN", bin_path.to_str().unwrap());
    cmd_start.args(["daemon", "start", "--profile", "sidecar", "--all"]);
    cmd_start.assert().success();

    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Trigger AppTicket push via mock server
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/auth/appTicket/resend", mock_url))
        .header("appKey", "AK_SA")
        .send()
        .await
        .expect("Failed to trigger AppTicket push");
    assert!(res.status().is_success());

    // Wait for Token Acquisition and Check Token Validation via CLI
    let mut token_found = false;
    let mut last_stdout = String::new();

    for _ in 0..15 {
        tokio::time::sleep(Duration::from_millis(1000)).await;

        let mut cmd_token = Command::cargo_bin("cowen").unwrap();
        cmd_token.env("COWEN_HOME", cowen_home.to_str().unwrap());
        cmd_token.env("HOME", &home);
        cmd_token.env("COWEN_RAW_OUTPUT", "true");
        cmd_token.args(["auth", "token", "--profile", "sidecar", "--format", "json"]);

        if let Ok(output) = cmd_token.output() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            last_stdout = stdout.clone();
            if stdout.contains("mock_at_sa_") {
                token_found = true;
                break;
            }
        }
    }
    if !token_found {
        let log_file = cowen_home.join("logs").join("daemon.stdout.log");
        let log_content =
            std::fs::read_to_string(&log_file).unwrap_or_else(|_| "No log found".to_string());
        eprintln!("DAEMON LOG:\n{}", log_content);
    }

    assert!(
        token_found,
        "Expected token starting with mock_at_sa_, got: {}",
        last_stdout
    );

    // Check config sanitization
    let mut cmd_config = Command::cargo_bin("cowen").unwrap();
    cmd_config.env("COWEN_HOME", &home);
    cmd_config.env("HOME", &home);
    cmd_config.args(["config", "--profile", "sidecar"]);

    let config_assert = cmd_config.assert().success();
    let config_stdout = String::from_utf8_lossy(&config_assert.get_output().stdout);

    assert!(
        !config_stdout.contains("AS_SA"),
        "Output contains raw app-secret!"
    );
    assert!(
        config_stdout.contains("***"),
        "Output should contain sanitized app-secret"
    );
}
