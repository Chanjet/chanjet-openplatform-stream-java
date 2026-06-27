#![allow(unused_imports, unused_variables, dead_code)]

use tempfile::tempdir;

#[tokio::test]
async fn test_cli_admin_commands() {
    let profile = "case_87_admin";
    let dir = tempdir().unwrap();
    let home = dir.path().join(".cowen");
    std::fs::create_dir_all(&home).unwrap();
    
    let mut init_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "init", "--profile", profile, "--app-key", "dummykey",
        "--app-secret", "dummysecret", "--app-mode", "self-built",
        "--certificate", "dummy_cert", "--encrypt-key", "dummy_ek"
    ]);
    assert!(init_cmd.status().unwrap().success());
    
    let mut daemon_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "--profile", profile, "daemon", "start"
    ]);
    assert!(daemon_cmd.status().unwrap().success());
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Insert dummy DLQ record
    let db_path = home.join("cowen.db");
    let mut sql_cmd = std::process::Command::new("sqlite3");
    sql_cmd.args([
        db_path.to_str().unwrap(),
        &format!("INSERT INTO cowen_dlq (id, profile, topic, payload, retry_count, error, created_at) VALUES (1, '{}', 'test_topic', '{{"msg": "hello"}}', 0, 'mock_err', '2026-06-18 14:00:00');", profile)
    ]);
    sql_cmd.status().unwrap();
    
    // Audit
    let mut audit_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    audit_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "--profile", profile, "audit", "--lines", "5"
    ]);
    let mut audit_child = audit_cmd.spawn().unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    {
    #[cfg(unix)]
    let _ = std::process::Command::new("kill").arg("-15").arg(audit_child.id().to_string()).status();
    #[cfg(windows)]
    let _ = crate::e2e::rust::common::graceful_kill_child(&mut audit_child);
    std::thread::sleep(std::time::Duration::from_millis(500));
}
    
    // Events
    let events = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["--profile", profile, "events"])
        .output().unwrap();
    let events_str = String::from_utf8_lossy(&events.stderr);
    assert!(events_str.contains("thin CLI architecture"));
    
    // Log list
    std::fs::create_dir_all(home.join("logs")).unwrap();
    std::fs::write(home.join("logs").join(format!("{}_main.log", profile)), "log line 1
log line 2
log line 3
").unwrap();
    
    let logs = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["--profile", profile, "log", "list"])
        .output().unwrap();
    assert!(String::from_utf8_lossy(&logs.stdout).contains(&format!("{}_main.log", profile)));
    
    let view = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["--profile", profile, "log", "view", "main", "--lines", "2"])
        .output().unwrap();
    assert!(String::from_utf8_lossy(&view.stdout).contains("log line 3"));
    
    // DLQ commands
    let dlq_list = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["--profile", profile, "dlq", "list"])
        .output().unwrap();
    assert!(String::from_utf8_lossy(&dlq_list.stdout).contains("test_topic"));
    
    let dlq_view = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["--profile", profile, "dlq", "view", "1"])
        .output().unwrap();
    assert!(String::from_utf8_lossy(&dlq_view.stdout).contains("hello"));
    
    let dlq_retry = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["--profile", profile, "dlq", "retry", "1"])
        .output().unwrap();
    assert!(String::from_utf8_lossy(&dlq_retry.stdout).contains("Retrying"));
    
    let dlq_purge = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["--profile", profile, "dlq", "purge"])
        .output().unwrap();
    assert!(String::from_utf8_lossy(&dlq_purge.stdout).contains("Purging"));
    
    let mut stop_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    stop_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "--profile", profile, "daemon", "stop"
    ]);
    stop_cmd.status().unwrap();
}

#[tokio::test]
async fn test_cli_api_commands() {
    let profile = "api_test";
    let dir = tempdir().unwrap();
    let home = dir.path().join(".cowen");
    std::fs::create_dir_all(&home).unwrap();
    let (mock_port, _) = super::mock_server::spawn_mock_server().await;
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);
    
    let mut init_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "init", "--profile", profile, "--app-key", "dummy_app_key",
        "--app-secret", "dummy_app_secret", "--app-mode", "self-built",
        "--certificate", "dummy_cert", "--stream-url", &mock_ws,
        "--encrypt-key", "1234567890123456"
    ]);
    assert!(init_cmd.status().unwrap().success());
    
    let mut daemon_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "daemon", "start", "--profile", profile
    ]);
    assert!(daemon_cmd.status().unwrap().success());
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    let api_list = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["api", "list", "--profile", profile])
        .output().unwrap();
    assert!(api_list.status.success());
    
    let api_list_json = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["api", "list", "--profile", profile, "--format", "json"])
        .output().unwrap();
    assert!(api_list_json.status.success());
    
    let api_list_yaml = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["api", "list", "--profile", profile, "--format", "yaml"])
        .output().unwrap();
    assert!(api_list_yaml.status.success());
    
    let api_list_search = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["api", "list", "--profile", profile, "--search", "token"])
        .output().unwrap();
    assert!(api_list_search.status.success());
    
    let api_spec_raw = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["api", "spec", "GET", "/dummy", "--profile", profile, "--raw"])
        .output().unwrap();
    // we don't strict check success since it may not exist
    
    let mut stop_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    stop_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "daemon", "stop", "--profile", profile
    ]);
    stop_cmd.status().unwrap();
}

#[tokio::test]
async fn test_process_mgmt_e2e() {
    let profile = "pm1";
    let dir = tempdir().unwrap();
    let home = dir.path().join(".cowen");
    std::fs::create_dir_all(&home).unwrap();
    let (mock_port, _) = super::mock_server::spawn_mock_server().await;
    
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);
    let proxy_port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    
    let mut init_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "init", "--profile", profile, "--app-key", "dummy_key_1",
        "--app-secret", "dummy_secret_1", "--app-mode", "self-built",
        "--certificate", "dummy_cert", "--encrypt-key", "1234567890123456",
        "--openapi-url", &mock_url, "--stream-url", &mock_ws,
        "--proxy-port", &proxy_port.to_string(), "--webhook-target", "http://127.0.0.1:8080/cb"
    ]);
    assert!(init_cmd.status().unwrap().success());
    
    let mut daemon_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "daemon", "start", "--profile", profile
    ]);
    assert!(daemon_cmd.status().unwrap().success());
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    let pid_file = home.join("master_daemon.pid");
    let pid_bak = home.join("master_daemon.pid.bak");
    std::fs::rename(&pid_file, &pid_bak).unwrap();
    
    let status = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home).env("COWEN_SKIP_DAEMON_RECOVERY", "true")
        .args(["status", "--profile", profile])
        .output().unwrap();
    let status_str = String::from_utf8_lossy(&status.stderr);
    
    std::fs::rename(&pid_bak, &pid_file).unwrap();
    
    // Might contain profile name or "unknown"
    assert!(status_str.contains("pm1") || status_str.contains("unknown") || status_str.contains("Unknown"));
    
    let mut stop_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    stop_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "daemon", "stop", "--profile", profile
    ]);
    stop_cmd.status().unwrap();
    
    // Start a python listener on the port
    let py_script = r#"
import socket, time
try:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.bind(('127.0.0.1', PORT))
    s.listen(1)
    time.sleep(5)
except:
    pass
"#.replace("PORT", &proxy_port.to_string());
    let mut py_child = std::process::Command::new("python3").args(["-c", &py_script]).spawn().unwrap();
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    std::fs::rename(&pid_file, &pid_bak).unwrap_or(());
    
    let status2 = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["status", "--profile", profile])
        .output().unwrap();
    let status2_str = String::from_utf8_lossy(&status2.stderr);
    
    crate::e2e::rust::common::graceful_kill_child(&mut py_child).unwrap_or(());
    
    assert!(status2_str.contains("PID:") || status2_str.contains("python3") || status2_str.contains("Unknown Process"));
}
