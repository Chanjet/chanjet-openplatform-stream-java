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

#[tokio::test(flavor = "multi_thread")]
async fn test_reset_behaviors() {
    let dir = tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();

    let run_init = |profile: &str, keys: bool, expect_success: bool| {
        let mut cmd = Command::cargo_bin("cowen").unwrap();
        cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
            "-p",
            profile,
            "init",
            "--app-mode",
            "self-built",
        ]);
        if keys {
            cmd.args([
                "--app-key",
                "K",
                "--app-secret",
                "S",
                "--certificate",
                "C",
                "--encrypt-key",
                "E",
            ]);
        }
        if expect_success {
            cmd.assert().success();
        } else {
            cmd.assert().failure();
        }
    };

    let run_reset = |profile: Option<&str>| {
        let mut cmd = Command::cargo_bin("cowen").unwrap();
        cmd.env("COWEN_HOME", &home).env("HOME", &home).arg("reset");
        if let Some(p) = profile {
            cmd.args(["-p", p]);
        }
        cmd.assert().success();
    };

    let check_profile_list = |profile: &str, should_exist: bool| {
        let mut cmd = Command::cargo_bin("cowen").unwrap();
        cmd.env("COWEN_HOME", &home)
            .env("HOME", &home)
            .args(["profile", "list"]);
        let out = cmd.output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        if should_exist {
            assert!(stdout.contains(profile));
        } else {
            assert!(!stdout.contains(profile));
        }
    };

    // Test 1: Reset specific profile
    let p1 = "p1";
    run_init(p1, true, true);
    assert!(dir.path().join(format!("{}.yaml", p1)).exists());
    run_reset(Some(p1));
    assert!(!dir.path().join(format!("{}.yaml", p1)).exists());
    check_profile_list(p1, false);

    // Test 2: Missing keys after reset
    let p2 = "p2";
    run_init(p2, true, true);
    run_reset(Some(p2));
    run_init(p2, false, false); // should fail because keys are gone

    // Test 3: Full reset
    let p3 = "p3";
    run_init(p3, true, true);
    run_reset(None);
    assert!(!dir.path().join(format!("{}.yaml", p3)).exists());
    check_profile_list(p3, false);
}

#[tokio::test]
async fn test_config_file_initialization_import() {
    let _profile = "case_82";
    let (dir, home) = setup_config_env();

    let port_p1 = get_unused_port();
    let proxy_port_p1 = get_unused_port();
    let _port_p2 = get_unused_port();
    let _proxy_port_p2 = get_unused_port();

    let template_p1 = dir.path().join("p1_template.yaml");
    std::fs::write(
        &template_p1,
        format!(
            r#"
app_key: "mock_app_key_1"
app_mode: "store-app"
webhook_target: "http://127.0.0.1:9299/callback"
proxy_port: {}
proxy_enabled: true
gateway:
  bind_address: "127.0.0.1:{}"
  routes:
    - path: "/**"
      upstream: "http://127.0.0.1:9299"
storage:
  type: "sqlite"
log:
  level: "info"
"#,
            proxy_port_p1, port_p1
        ),
    )
    .unwrap();

    let mut init_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "init",
        "--profile",
        "p1",
        "--file",
        template_p1.to_str().unwrap(),
        "--app-secret",
        "my_secret_key",
        "--encrypt-key",
        "1234567890123456",
    ]);
    init_cmd.status().unwrap();

    let cfg = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home)
        .env("HOME", &home)
        .args(["config", "--profile", "p1"])
        .output()
        .unwrap();
    let cfg_str = String::from_utf8_lossy(&cfg.stdout);
    assert!(cfg_str.contains("mock_app_key_1"));
    assert!(cfg_str.contains(&proxy_port_p1.to_string()));

    // B. With configured profile 'p1' (Pre-filled non-sensitive values)
    let temp_p1_exp = dir.path().join("temp_p1_exp.yaml");
    let exp = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home)
        .env("HOME", &home)
        .args(["config", "template", "--profile", "p1"])
        .output()
        .unwrap();
    std::fs::write(&temp_p1_exp, &exp.stdout).unwrap();
    let exp_str = String::from_utf8_lossy(&exp.stdout);
    assert!(exp_str.contains("mock_app_key_1"));
    assert!(!exp_str.lines().any(|l| l.starts_with("app_secret:")));
    assert!(!exp_str.lines().any(|l| l.starts_with("encrypt_key:")));

    // C. Re-initialization overrides config
    let template_p1_new = dir.path().join("p1_template_new.yaml");
    let new_proxy_port = get_unused_port();
    std::fs::write(
        &template_p1_new,
        format!(
            r#"
app_key: "mock_app_key_1"
app_mode: "store-app"
webhook_target: "http://127.0.0.1:9299/callback_new"
proxy_port: {}
proxy_enabled: true
gateway:
  bind_address: "127.0.0.1:{}"
"#,
            new_proxy_port, port_p1
        ),
    )
    .unwrap();

    let mut init_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "init",
        "--profile",
        "p1",
        "--file",
        template_p1_new.to_str().unwrap(),
    ]);
    init_cmd.status().unwrap();

    let cfg2 = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home)
        .env("HOME", &home)
        .args(["config", "--profile", "p1"])
        .output()
        .unwrap();
    let cfg_str2 = String::from_utf8_lossy(&cfg2.stdout);
    assert!(cfg_str2.contains("callback_new"));
    assert!(cfg_str2.contains(&new_proxy_port.to_string()));

    let secret = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home)
        .env("HOME", &home)
        .args(["config", "get", "--profile", "p1", "app_secret"])
        .output()
        .unwrap();
    let secret_str = String::from_utf8_lossy(&secret.stdout);
    assert!(secret_str.contains("my_secret_key"));
}

fn get_unused_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[tokio::test]
async fn test_config_template_roundtrip() {
    let profile1 = "p1_case_83";
    let profile2 = "p2_case_83";
    let (dir, home) = setup_config_env();

    let port_gw = get_unused_port();
    let port_proxy = get_unused_port();
    let mock_url = "http://127.0.0.1:9999";

    let orig_yaml = dir.path().join("orig_template.yaml");
    std::fs::write(
        &orig_yaml,
        format!(
            r#"
app_key: "key_roundtrip_test"
app_mode: "store-app"
webhook_target: "{}/webhook"
proxy_port: {}
proxy_enabled: true
gateway:
  bind_address: "127.0.0.1:{}"
  auth_sync_hook: "{}/sync"
  auth_routing:
    mode: "STRICT"
    bypass_rules:
      - "/v1/ping"
      - "/static/**"
    require_rules:
      - "**"
  routes:
    - path: "/open-api/**"
      upstream: "openapi"
      strip_prefix: "/open-api"
    - path: "/**"
      upstream: "{}"
"#,
            mock_url, port_proxy, port_gw, mock_url, mock_url
        ),
    )
    .unwrap();

    let mut init_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "init",
        "--profile",
        profile1,
        "--file",
        orig_yaml.to_str().unwrap(),
        "--app-secret",
        "mysecret",
        "--encrypt-key",
        "1234567890123456",
    ]);
    init_cmd.status().unwrap();

    let exp_yaml = dir.path().join("exported_template.yaml");
    let exp = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home)
        .env("HOME", &home)
        .args(["config", "template", "--profile", profile1])
        .output()
        .unwrap();
    std::fs::write(&exp_yaml, &exp.stdout).unwrap();

    let exp_str = String::from_utf8_lossy(&exp.stdout);
    assert!(exp_str.contains("gateway:"));
    assert!(exp_str.contains(&format!("bind_address: \"127.0.0.1:{}\"", port_gw)));
    assert!(exp_str.contains("auth_sync_hook:"));
    assert!(exp_str.contains("mode: \"STRICT\""));
    assert!(exp_str.contains("- \"/v1/ping\""));

    // Reset profile1 to avoid port conflicts with profile2
    let mut reset_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    reset_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "reset",
        "--profile",
        profile1,
        "--no-telemetry",
    ]);
    reset_cmd.status().unwrap();

    // Init profile 2 with exported template
    let mut init2_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init2_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "init",
        "--profile",
        profile2,
        "--file",
        exp_yaml.to_str().unwrap(),
        "--app-secret",
        "mysecret",
        "--encrypt-key",
        "1234567890123456",
    ]);
    init2_cmd.status().unwrap();

    let cfg2 = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home)
        .env("HOME", &home)
        .args(["config", "--profile", profile2])
        .output()
        .unwrap();
    let cfg2_str = String::from_utf8_lossy(&cfg2.stdout);
    assert!(cfg2_str.contains("key_roundtrip_test"));
    assert!(cfg2_str.contains(&format!("bind_address: 127.0.0.1:{}", port_gw)));
    assert!(cfg2_str.contains("- /v1/ping"));
}
