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

#[tokio::test(flavor = "multi_thread")]
async fn test_init_deduplication() {
    let (mock_port, _state) = crate::e2e::rust::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);

    let (_temp, home, _killer) =
        crate::e2e::rust::common::setup_test_env("prof_a", "self-built", &mock_url);

    // 1. Initialize first profile
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.args([
        "init",
        "--profile",
        "prof_a",
        "--app-mode",
        "self-built",
        "--app-key",
        "KEY_DUP",
        "--app-secret",
        "SEC_DUP",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "CERT_DUP",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
    ]);
    cmd.assert().success();

    // 2. Initialize second profile with same credentials but different profile name
    let mut cmd2 = Command::cargo_bin("cowen").unwrap();
    cmd2.env("COWEN_HOME", &home);
    cmd2.env("HOME", &home);
    cmd2.args([
        "init",
        "--profile",
        "prof_b",
        "--app-mode",
        "self-built",
        "--app-key",
        "KEY_DUP",
        "--app-secret",
        "SEC_DUP",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "CERT_DUP",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
    ]);

    let _ = cmd2.output().unwrap();

    let list_cmd = Command::cargo_bin("cowen")
        .unwrap()
        .env("COWEN_HOME", &home)
        .arg("profile")
        .arg("list")
        .output()
        .unwrap();
    let list_out = String::from_utf8_lossy(&list_cmd.stdout);

    assert!(
        !list_out.contains("prof_b"),
        "prof_b should not have been created"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_init_default_app_mode() {
    let temp_dir = tempfile::tempdir().unwrap();
    let home = temp_dir.path().to_str().unwrap().to_string();

    // Running without app-mode should default to oauth2 and block, so we spawn it using std::process::Command
    let bin_path = assert_cmd::cargo::cargo_bin("cowen");
    let mut child = std::process::Command::new(bin_path)
        .env("COWEN_HOME", &home)
        .env("HOME", &home)
        .args([
            "init",
            "--profile",
            "default_oauth2",
            "--app-key",
            "dummy_key",
            "--app-secret",
            "dummy_secret",
            "--certificate",
            "dummy_cert",
            "--encrypt-key",
            "dummy_ek",
        ])
        .spawn()
        .unwrap();

    // Wait for the db to be initialized
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    let _ = child.kill();
    let _ = child.wait(); // Terminate the blocking init

    // Check database for oauth2 mode
    let db_path = std::path::PathBuf::from(&home).join("cowen.db");
    assert!(db_path.exists(), "cowen.db should exist");

    let mut cfg_cmd = Command::cargo_bin("cowen").unwrap();
    cfg_cmd.env("COWEN_HOME", &home);
    cfg_cmd.env("HOME", &home);
    cfg_cmd.args(["config", "--profile", "default_oauth2"]);
    let cfg_out = String::from_utf8_lossy(&cfg_cmd.output().unwrap().stdout).to_string();
    assert!(
        cfg_out.contains("app_mode: oauth2") || cfg_out.contains("app_mode: \"oauth2\""),
        "Default app mode should be oauth2"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_init_permission_denied() {
    let temp_dir = tempfile::tempdir().unwrap();
    let home = temp_dir.path().to_str().unwrap().to_string();

    // Simulate read-only directory
    let mut perms = std::fs::metadata(&home).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&home, perms).unwrap();

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.args(["init", "--profile", "main"]);

    let output = cmd.output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // The exact message can be either 'Failed to create daemon logs directory' or 'Permission denied'
    let output_str = format!("{}{}", stdout, stderr);
    assert!(
        output_str.contains("Failed to create daemon logs directory")
            || output_str.contains("Permission denied")
            || output_str.contains("Read-only file system")
            || output_str.contains("No such file or directory"),
        "Expected permission denied message, got: {}",
        output_str
    );

    // Restore permissions for cleanup
    let mut perms = std::fs::metadata(&home).unwrap().permissions();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o444);
    }
    #[cfg(not(unix))]
    {
        perms.set_readonly(false);
    }

    let _ = std::fs::set_permissions(&home, perms);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_config_file_initialization_import() {
    let temp_dir = tempfile::tempdir().unwrap();
    let home_dir = temp_dir.path().join("home");
    std::fs::create_dir_all(&home_dir).unwrap();
    let home = home_dir.to_str().unwrap().to_string();

    // Scenario 1: Init with a config file outside of COWEN_HOME to avoid it being listed as a profile
    let template_path = temp_dir.path().join("template.yaml");
    let template_content = r#"
app_key: "mock_app_key_1"
app_mode: "store-app"
webhook_target: "http://127.0.0.1:9299/callback"
proxy_port: 8881
proxy_enabled: true
gateway:
  bind_address: "127.0.0.1:8882"
  routes:
    - path: "/**"
      upstream: "http://127.0.0.1:9299"
storage:
  type: "sqlite"
log:
  level: "info"
"#;
    std::fs::write(&template_path, template_content).unwrap();

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.args([
        "init",
        "--profile",
        "p1",
        "--file",
        template_path.to_str().unwrap(),
        "--app-secret",
        "my_secret_key",
        "--encrypt-key",
        "1234567890123456",
    ]);
    cmd.assert().success();

    // Verify config was parsed correctly
    let mut cmd_cfg = Command::cargo_bin("cowen").unwrap();
    cmd_cfg.env("COWEN_HOME", &home);
    cmd_cfg.env("HOME", &home);
    cmd_cfg.args(["config", "--profile", "p1"]);

    let cfg_out = String::from_utf8_lossy(&cmd_cfg.output().unwrap().stdout).to_string();
    assert!(cfg_out.contains("mock_app_key_1"));
    assert!(cfg_out.contains("8881"));
    assert!(cfg_out.contains("bind_address: 127.0.0.1:8882"));

    // Scenario 1.5: Config template export
    let mut cmd_tpl = Command::cargo_bin("cowen").unwrap();
    cmd_tpl.env("COWEN_HOME", &home);
    cmd_tpl.env("HOME", &home);
    cmd_tpl.args(["config", "template"]);

    let tpl_out = String::from_utf8_lossy(&cmd_tpl.output().unwrap().stdout).to_string();
    assert!(
        tpl_out.contains("app_mode: \"oauth2\"") || tpl_out.contains("app_mode: \"store-app\"")
    );

    let mut cmd_tpl_p1 = Command::cargo_bin("cowen").unwrap();
    cmd_tpl_p1.env("COWEN_HOME", &home);
    cmd_tpl_p1.env("HOME", &home);
    cmd_tpl_p1.args(["config", "template", "--profile", "p1"]);

    let tpl_p1_out = String::from_utf8_lossy(&cmd_tpl_p1.output().unwrap().stdout).to_string();
    assert!(tpl_p1_out.contains("app_key: \"mock_app_key_1\""));
    assert!(tpl_p1_out.contains("proxy_port: 8881"));
    assert!(!tpl_p1_out.contains("my_secret_key")); // Must not expose secret
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sidecar_startup() {
    // Migrate case_28_sidecar_startup.sh
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home_28");
    std::fs::create_dir_all(&home).unwrap();
    let home_str = home.to_str().unwrap();
    let profile = "env-auto-init";

    // Start daemon using env vars
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", home_str);
    cmd.env("HOME", home_str);
    cmd.env("COWEN_APP_MODE", "store-app");
    cmd.env("COWEN_APP_KEY", "AK_SIDECAR");
    cmd.env("COWEN_APP_SECRET", "AS_SIDECAR");
    cmd.env("COWEN_ENCRYPT_KEY", "1234567890123456");
    cmd.env("COWEN_WEBHOOK_TARGET", "http://127.0.0.1:8080/cb");
    cmd.env("COWEN_OPENAPI_URL", "http://127.0.0.1:9090");
    cmd.env("COWEN_STREAM_URL", "ws://127.0.0.1:9090");
    cmd.env("COWEN_PROXY_PORT", "0");

    let mut daemon_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd.env("COWEN_HOME", home_str);
    daemon_cmd.env("HOME", home_str);
    daemon_cmd.env("COWEN_APP_MODE", "store-app");
    daemon_cmd.env("COWEN_APP_KEY", "AK_SIDECAR");
    daemon_cmd.env("COWEN_APP_SECRET", "AS_SIDECAR");
    daemon_cmd.env("COWEN_ENCRYPT_KEY", "1234567890123456");
    daemon_cmd.env("COWEN_WEBHOOK_TARGET", "http://127.0.0.1:8080/cb");
    daemon_cmd.env("COWEN_OPENAPI_URL", "http://127.0.0.1:9090");
    daemon_cmd.env("COWEN_STREAM_URL", "ws://127.0.0.1:9090");
    daemon_cmd.env("COWEN_PROXY_PORT", "0");
    daemon_cmd.args(["--profile", profile, "daemon", "start", "--foreground"]);

    let mut child = daemon_cmd.spawn().unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let mut status_cmd = Command::cargo_bin("cowen").unwrap();
    status_cmd.env("COWEN_HOME", home_str);
    status_cmd.env("HOME", home_str);
    status_cmd.args(["--profile", profile, "status"]);

    let out = String::from_utf8_lossy(&status_cmd.output().unwrap().stdout).to_string();
    assert!(
        out.contains("ACTIVE") || out.contains("RUNNING"),
        "Daemon should be running via env init"
    );
    assert!(
        out.contains("Security (Vault):"),
        "Credentials should be injected"
    );

    let _ = child.kill();
    let _ = child.wait();

    // Test 2: Global Store Override (Need a fresh home)
    let home2 = dir.path().join("home_28_store_override");
    std::fs::create_dir_all(&home2).unwrap();
    let home2_str = home2.to_str().unwrap();

    let mut store_cmd = Command::cargo_bin("cowen").unwrap();
    let db_path = home2.join("overridden.db");
    let db_url = format!("innerdb://{}", db_path.display());
    store_cmd.env("COWEN_HOME", home2_str);
    store_cmd.env("HOME", home2_str);
    store_cmd.env("COWEN_STORE_TYPE", "innerdb");
    store_cmd.env("COWEN_DB_URL", &db_url);
    store_cmd.args(["store", "status"]);

    let store_out = String::from_utf8_lossy(&store_cmd.output().unwrap().stdout).to_string();
    assert!(
        store_out.contains("innerdb://"),
        "Store URL should be overridden. Actual: {}",
        store_out
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_exclusive_connection() {
    let (mock_port, _) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}/connect", mock_port);

    let dir = tempfile::tempdir().unwrap();

    // P1 Setup
    let p1_home = dir.path().join("p1");
    std::fs::create_dir_all(&p1_home).unwrap();
    let p1_home_str = p1_home.to_str().unwrap();

    let mut init_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd.env("COWEN_HOME", p1_home_str);
    init_cmd.env("HOME", p1_home_str);
    init_cmd.env("COWEN_EXCLUSIVE", "true");
    init_cmd.args([
        "init",
        "--profile",
        "p1",
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_EXCLUSIVE",
        "--app-secret",
        "AS_EXC",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "CERT_EXC",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--proxy-port",
        "0",
    ]);
    assert!(init_cmd.status().unwrap().success());

    // Check connection count for P1
    let client = reqwest::Client::new();
    let mut conn_count = 0;
    for _ in 0..10 {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        if let Ok(res) = client
            .get(format!("{}/control/connection_count", mock_url))
            .send()
            .await
        {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(count) = json.get("count").and_then(|c| c.as_u64()) {
                    conn_count = count;
                    if count >= 1 {
                        break;
                    }
                }
            }
        }
    }
    assert_eq!(conn_count, 1, "P1 should be connected");

    // P2 Setup
    let p2_home = dir.path().join("p2");
    std::fs::create_dir_all(&p2_home).unwrap();
    let p2_home_str = p2_home.to_str().unwrap();

    let mut init_cmd2 = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd2.env("COWEN_HOME", p2_home_str);
    init_cmd2.env("HOME", p2_home_str);
    init_cmd2.env("COWEN_EXCLUSIVE", "true");
    init_cmd2.args([
        "init",
        "--profile",
        "p2",
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_EXCLUSIVE",
        "--app-secret",
        "AS_EXC",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "CERT_EXC",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--proxy-port",
        "0",
    ]);
    assert!(init_cmd2.status().unwrap().success());

    // Wait for P2 to connect and evict P1
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Check connection count again. It should be 1 because P1 is evicted
    conn_count = 0;
    if let Ok(res) = client
        .get(format!("{}/control/connection_count", mock_url))
        .send()
        .await
    {
        if let Ok(json) = res.json::<serde_json::Value>().await {
            if let Some(count) = json.get("count").and_then(|c| c.as_u64()) {
                conn_count = count;
            }
        }
    }
    assert_eq!(
        conn_count, 1,
        "Only one connection should remain active (Exclusive mode working)"
    );

    // Cleanup P1 and P2
    let mut kill_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    kill_cmd.env("COWEN_HOME", p1_home_str);
    kill_cmd.args(["daemon", "stop"]);
    let _ = kill_cmd.status();

    let mut kill_cmd2 = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    kill_cmd2.env("COWEN_HOME", p2_home_str);
    kill_cmd2.args(["daemon", "stop"]);
    let _ = kill_cmd2.status();
}
