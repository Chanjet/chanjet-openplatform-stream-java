use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;
use tokio::time::{sleep, Duration};

fn setup_config_env() -> (tempfile::TempDir, String) {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    let home = dir.path().join("home");
    fs::create_dir_all(&cowen_home).unwrap();
    fs::create_dir_all(&home).unwrap();

    let app_config_path = cowen_home.join("app.yaml");
    let app_config = serde_json::json!({
        "openapi_url": "http://127.0.0.1:8080",
        "stream_url": "ws://127.0.0.1:8080",
        "telemetry_enabled": false,
        "log": {
            "level": "info",
        }
    });
    fs::write(app_config_path, serde_yaml::to_string(&app_config).unwrap()).unwrap();

    (dir, home.to_str().unwrap().to_string())
}

#[tokio::test]
async fn test_config_hot_reload() {
    let (_dir, home) = setup_config_env();

    // 1. Initialize profile
    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &home);
    init_cmd.env("HOME", &home);
    init_cmd.args([
        "init",
        "--profile",
        "hot_reload_test",
        "--app-mode",
        "self-built",
        "--app-key",
        "k",
        "--app-secret",
        "s",
        "--certificate",
        "c",
        "--encrypt-key",
        "e",
        "--stream-url",
        "http://localhost:8080",
    ]);
    init_cmd.assert().success();

    // 2. Set config log level
    let mut set_log_cmd = Command::cargo_bin("cowen").unwrap();
    set_log_cmd.env("COWEN_HOME", &home);
    set_log_cmd.env("HOME", &home);
    set_log_cmd.args([
        "config",
        "set",
        "--profile",
        "hot_reload_test",
        "log.level",
        "debug",
    ]);
    set_log_cmd.assert().success();

    // 3. Start daemon in background
    let home_clone = home.clone();
    let mut daemon_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd.env("COWEN_HOME", &home_clone);
    daemon_cmd.env("HOME", &home_clone);
    daemon_cmd.args(["daemon", "start", "--profile", "hot_reload_test"]);
    let mut daemon_child = daemon_cmd.spawn().unwrap();

    sleep(Duration::from_secs(5)).await; // wait for daemon start

    // get pid
    let pid_file = format!("{}/master_daemon.pid", home);
    let pid_str = std::fs::read_to_string(&pid_file).unwrap_or_default();
    assert!(!pid_str.is_empty(), "Daemon failed to write pidfile");

    // 4. Verify config
    let mut get_log_cmd = Command::cargo_bin("cowen").unwrap();
    get_log_cmd.env("COWEN_HOME", &home);
    get_log_cmd.env("HOME", &home);
    get_log_cmd.args(["config", "--profile", "hot_reload_test"]);
    let output = get_log_cmd.assert().success().get_output().stdout.clone();
    let output_str = String::from_utf8(output).unwrap();
    assert!(output_str.contains("level: debug"));

    // 5. Hot-reload config
    let mut set_log_cmd2 = Command::cargo_bin("cowen").unwrap();
    set_log_cmd2.env("COWEN_HOME", &home);
    set_log_cmd2.env("HOME", &home);
    set_log_cmd2.args([
        "config",
        "set",
        "--profile",
        "hot_reload_test",
        "log.level",
        "info",
    ]);
    set_log_cmd2.assert().success();

    sleep(Duration::from_secs(2)).await; // wait for watcher

    // 6. Verify PID unchanged
    let current_pid_str = std::fs::read_to_string(&pid_file).unwrap_or_default();
    assert_eq!(
        pid_str, current_pid_str,
        "Daemon restarted on config reload"
    );

    // 7. Verify config level updated
    let mut get_log_cmd2 = Command::cargo_bin("cowen").unwrap();
    get_log_cmd2.env("COWEN_HOME", &home);
    get_log_cmd2.env("HOME", &home);
    get_log_cmd2.args(["config", "--profile", "hot_reload_test"]);
    let output2 = get_log_cmd2.assert().success().get_output().stdout.clone();
    let output_str2 = String::from_utf8(output2).unwrap();
    assert!(output_str2.contains("level: info"));

    // 8. Cleanup
    let mut stop_cmd = Command::cargo_bin("cowen").unwrap();
    stop_cmd.env("COWEN_HOME", &home);
    stop_cmd.env("HOME", &home);
    stop_cmd.args(["daemon", "stop", "--profile", "hot_reload_test"]);
    stop_cmd.assert().success();

    let _ = daemon_child.kill();
    let _ = daemon_child.wait();
}

#[tokio::test]
async fn test_config_engine_comprehensive() {
    let (_dir, home) = setup_config_env();

    // Setup: Initialize
    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &home);
    init_cmd.env("HOME", &home);
    init_cmd.args([
        "init",
        "--app-mode",
        "self-built",
        "--app-key",
        "k",
        "--app-secret",
        "s",
        "--certificate",
        "c",
        "--encrypt-key",
        "e",
        "--stream-url",
        "http://localhost:8080",
    ]);
    init_cmd.assert().success();

    // Test 1: Set/Get Nested Config
    let mut cmd1 = Command::cargo_bin("cowen").unwrap();
    cmd1.env("COWEN_HOME", &home);
    cmd1.env("HOME", &home);
    cmd1.args(["config", "set", "proxy_port", "16001"]);
    cmd1.assert().success();

    let mut get1 = Command::cargo_bin("cowen").unwrap();
    get1.env("COWEN_HOME", &home);
    get1.env("HOME", &home);
    get1.args(["config", "get", "proxy_port"]);
    let out = String::from_utf8(get1.assert().success().get_output().stdout.clone()).unwrap();
    assert_eq!(out.trim(), "16001");

    let mut cmd2 = Command::cargo_bin("cowen").unwrap();
    cmd2.env("COWEN_HOME", &home);
    cmd2.env("HOME", &home);
    cmd2.args(["config", "set", "log.level", "debug"]);
    cmd2.assert().success();

    let mut get2 = Command::cargo_bin("cowen").unwrap();
    get2.env("COWEN_HOME", &home);
    get2.env("HOME", &home);
    get2.args(["config", "get", "log.level"]);
    let out2 = String::from_utf8(get2.assert().success().get_output().stdout.clone()).unwrap();
    assert_eq!(out2.trim().replace("\"", ""), "debug");

    // Test 2: Global Field Routing
    let mut cmd3 = Command::cargo_bin("cowen").unwrap();
    cmd3.env("COWEN_HOME", &home);
    cmd3.env("HOME", &home);
    cmd3.args(["config", "set", "monitor_port", "9090"]);
    cmd3.assert().success();

    let app_yaml_content = std::fs::read_to_string(format!("{}/app.yaml", home)).unwrap();
    assert!(app_yaml_content.contains("monitor_port: 9090"));

    // Test 3: Validation (Interceptors)
    let mut cmd4 = Command::cargo_bin("cowen").unwrap();
    cmd4.env("COWEN_HOME", &home);
    cmd4.env("HOME", &home);
    cmd4.args(["config", "set", "proxy_port", "80"]);
    cmd4.assert().failure(); // Should fail for port 80

    // Test 4: Locking (Locked Fields)
    let mut cmd5 = Command::cargo_bin("cowen").unwrap();
    cmd5.env("COWEN_HOME", &home);
    cmd5.env("HOME", &home);
    cmd5.args(["config", "set", "app_key", "my-new-key"]);
    cmd5.assert().failure(); // Should fail to lock app_key

    // Test 5: Data Masking
    let mut cmd6 = Command::cargo_bin("cowen").unwrap();
    cmd6.env("COWEN_HOME", &home);
    cmd6.env("HOME", &home);
    cmd6.args(["config", "set", "storage.store", "local"]);
    cmd6.assert().success();

    let mut cmd7 = Command::cargo_bin("cowen").unwrap();
    cmd7.env("COWEN_HOME", &home);
    cmd7.env("HOME", &home);
    cmd7.args([
        "config",
        "set",
        "storage.db_url",
        &format!("sqlite://{}/db.sqlite", home),
    ]);
    cmd7.assert().success();

    let mut list = Command::cargo_bin("cowen").unwrap();
    list.env("COWEN_HOME", &home);
    list.env("HOME", &home);
    list.args(["config", "list"]);
    let out3 = String::from_utf8(list.assert().success().get_output().stdout.clone()).unwrap();
    assert!(out3.contains("******"), "db_url not masked");
}
