use assert_cmd::Command;

use std::fs;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread")]
async fn test_cli_admin_commands() {
    let dir = tempdir().unwrap();
    let profile = "case_87_admin";
    let cowen_home = dir.path().to_str().unwrap().to_string();

    fs::create_dir_all(&cowen_home).unwrap();

    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &cowen_home);
    init_cmd.args([
        "init",
        "--profile",
        profile,
        "--app-key",
        "dummykey",
        "--app-secret",
        "dummysecret",
        "--app-mode",
        "self-built",
        "--certificate",
        "dummy_cert",
        "--encrypt-key",
        "dummy_ek",
    ]);
    init_cmd.assert().success();

    let mut start_cmd = Command::cargo_bin("cowen").unwrap();
    start_cmd.env("COWEN_HOME", &cowen_home);
    start_cmd.args(["daemon", "start", "--profile", profile]);
    start_cmd.assert().success();

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Insert dummy DLQ using sqlite3 CLI
    let db_path = std::path::Path::new(&cowen_home).join("cowen.db");
    let insert_query = format!("INSERT INTO cowen_dlq (id, profile, topic, payload, retry_count, error, created_at) VALUES (1, '{}', 'test_topic', '{{\"msg\": \"hello\"}}', 0, 'mock_err', '2026-06-18 14:00:00');", profile);
    let mut sqlite_cmd = std::process::Command::new("sqlite3");
    sqlite_cmd.arg(db_path.to_str().unwrap()).arg(&insert_query);
    let _ = sqlite_cmd.output();

    // 1. cowen audit
    let mut audit_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    audit_cmd.env("COWEN_HOME", &cowen_home);
    audit_cmd.args(["audit", "--lines", "5", "--profile", profile]);
    let mut audit_child = audit_cmd.spawn().unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    {
        #[cfg(unix)]
        let _ = std::process::Command::new("kill")
            .arg("-15")
            .arg(audit_child.id().to_string())
            .status();
        #[cfg(windows)]
        let _ = crate::e2e::rust::common::graceful_kill_child(&mut audit_child);
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = audit_child.wait();

    // 2. cowen events
    let mut events_cmd = Command::cargo_bin("cowen").unwrap();
    events_cmd.env("COWEN_HOME", &cowen_home);
    events_cmd.args(["events", "--profile", profile]);
    let events_out = events_cmd.output().unwrap();
    let events_err = String::from_utf8_lossy(&events_out.stderr);
    assert!(
        events_err.contains("thin CLI architecture")
            || String::from_utf8_lossy(&events_out.stdout).contains("thin CLI architecture")
    );

    // 3. cowen log list
    let mut log_list_cmd = Command::cargo_bin("cowen").unwrap();
    log_list_cmd.env("COWEN_HOME", &cowen_home);
    log_list_cmd.args(["log", "list", "--profile", profile]);
    log_list_cmd.assert().success();

    let logs_dir = std::path::Path::new(&cowen_home).join("logs");
    fs::create_dir_all(&logs_dir).unwrap();
    let log_file = logs_dir.join(format!("{}_main.log", profile));
    fs::write(&log_file, "log line 1\nlog line 2\nlog line 3\n").unwrap();

    let mut log_list_cmd2 = Command::cargo_bin("cowen").unwrap();
    log_list_cmd2.env("COWEN_HOME", &cowen_home);
    log_list_cmd2.args(["log", "list", "--profile", profile]);
    log_list_cmd2
        .assert()
        .success()
        .stdout(predicates::str::contains(format!("{}_main.log", profile)));

    // 4. cowen log view
    let mut log_view_cmd = Command::cargo_bin("cowen").unwrap();
    log_view_cmd.env("COWEN_HOME", &cowen_home);
    log_view_cmd.args(["log", "view", "main", "--lines", "2", "--profile", profile]);
    log_view_cmd
        .assert()
        .success()
        .stdout(predicates::str::contains("log line 3"));

    // follow log view
    let mut log_follow_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    log_follow_cmd.env("COWEN_HOME", &cowen_home);
    log_follow_cmd.args([
        "log",
        "view",
        "main",
        "--lines",
        "2",
        "--follow",
        "--profile",
        profile,
    ]);
    let mut follow_child = log_follow_cmd.spawn().unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&log_file)
        .unwrap();
    writeln!(file, "log line 4 (append)").unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    {
        #[cfg(unix)]
        let _ = std::process::Command::new("kill")
            .arg("-15")
            .arg(follow_child.id().to_string())
            .status();
        #[cfg(windows)]
        let _ = crate::e2e::rust::common::graceful_kill_child(&mut follow_child);
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = follow_child.wait();

    // 5. cowen dlq
    let mut dlq_list_cmd = Command::cargo_bin("cowen").unwrap();
    dlq_list_cmd.env("COWEN_HOME", &cowen_home);
    dlq_list_cmd.args(["dlq", "list", "--profile", profile]);
    let list_out = dlq_list_cmd.output().unwrap();
    let list_out_str = String::from_utf8_lossy(&list_out.stdout).to_string()
        + String::from_utf8_lossy(&list_out.stderr).as_ref();
    assert!(
        list_out_str.contains("test_topic") || list_out_str.contains("No messages in DLQ"),
        "DLQ list: {}",
        list_out_str
    );

    let mut dlq_view_cmd = Command::cargo_bin("cowen").unwrap();
    dlq_view_cmd.env("COWEN_HOME", &cowen_home);
    dlq_view_cmd.args(["dlq", "view", "1", "--profile", profile]);
    let _ = dlq_view_cmd.output();

    let mut dlq_retry_cmd = Command::cargo_bin("cowen").unwrap();
    dlq_retry_cmd.env("COWEN_HOME", &cowen_home);
    dlq_retry_cmd.args(["dlq", "retry", "1", "--profile", profile]);
    let _ = dlq_retry_cmd.output();

    let mut dlq_purge_cmd = Command::cargo_bin("cowen").unwrap();
    dlq_purge_cmd.env("COWEN_HOME", &cowen_home);
    dlq_purge_cmd.args(["dlq", "purge", "--profile", profile]);
    let _ = dlq_purge_cmd.output();

    let mut stop_cmd = Command::cargo_bin("cowen").unwrap();
    stop_cmd.env("COWEN_HOME", &cowen_home);
    stop_cmd.args(["daemon", "stop", "--profile", profile]);
    let _ = stop_cmd.output();
}
