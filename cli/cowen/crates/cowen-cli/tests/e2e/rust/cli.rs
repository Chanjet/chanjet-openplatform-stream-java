use assert_cmd::Command;

struct TestDaemonGuard {
    home: String,
    child: std::process::Child,
}

impl Drop for TestDaemonGuard {
    fn drop(&mut self) {
        let child_pid = self.child.id();
        let pid_file = std::path::Path::new(&self.home).join("master_daemon.pid");
        eprintln!("DEBUG_TEST: TestDaemonGuard dropping. home={}, child_pid={}, pid_file={:?}, exists={}", self.home, child_pid, pid_file, pid_file.exists());

        // 1. Send SIGTERM to the CLI child process so it can shut down its child daemon gracefully
        #[cfg(unix)]
        {
            let _ = std::process::Command::new("kill").arg("-15").arg(child_pid.to_string()).status();
        }
        std::thread::sleep(std::time::Duration::from_millis(200));

        // 2. Force kill the CLI process just in case
        let _ = self.child.kill();
        let _ = self.child.wait();

        // 3. Fallback: Force kill the daemon process directly if it still exists
        if let Ok(content) = std::fs::read_to_string(&pid_file) {
            if let Some(pid_str) = content.lines().next() {
                if let Ok(pid) = pid_str.trim().parse::<i32>() {
                    eprintln!("DEBUG_TEST: TestDaemonGuard force-killing master daemon pid {}", pid);
                    let _ = std::process::Command::new("kill").arg("-9").arg(pid.to_string()).status();
                }
            }
        }
        let _ = std::fs::remove_file(pid_file);
    }
}

fn start_daemon_for_test(home: &str, profile: &str) -> TestDaemonGuard {
    let app_config_path = std::path::Path::new(home).join("app.yaml");
    let app_config = serde_json::json!({
        "openapi_url": "http://localhost:12345",
        "stream_url": "http://localhost:12345",
        "telemetry_enabled": false,
        "log": { "level": "debug" }
    });
    std::fs::write(&app_config_path, serde_yaml::to_string(&app_config).unwrap()).unwrap();

    
    
    
    let config_path = std::path::Path::new(home).join(format!("{}.yaml", profile));
    let config = serde_json::json!({
        "app_key": "test_key",
        "app_mode": "oauth2",
        "encrypt_key": "1234567890123456", 
        "webhook_target": "http://localhost:8080",
        "auto_start": false,
        "version": 1
    });
    std::fs::write(&config_path, serde_yaml::to_string(&config).unwrap()).unwrap();

    let bin_path = assert_cmd::cargo::cargo_bin("cowen");
    let child = std::process::Command::new(bin_path)
        .env("COWEN_HOME", home)
        .env("COWEN_SKIP_DAEMON_RECOVERY", "1")
        .arg("-p")
        .arg(profile)
        .arg("daemon")
        .arg("start")
        .arg("--foreground")
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .unwrap();
    
    std::thread::sleep(std::time::Duration::from_millis(1500));
    TestDaemonGuard {
        home: home.to_string(),
        child,
    }
}


#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("Usage:"));
}

#[test]
fn test_cli_version() {
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("cowen"));
}

#[test]
fn test_cli_invalid_command() {
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.arg("nonexistent_command");
    cmd.assert()
        .failure()
        .stderr(predicates::str::contains("unrecognized subcommand"));
}

#[test]
fn test_cli_config_set_global() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();
    let _daemon = start_daemon_for_test(&home, "default");
    
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "1");
    cmd.args(&["config", "set", "log.level", "debug", "--global"]);
    cmd.assert().success();
}

#[test]
fn test_cli_dlq_list_page_size_short_n() {
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "1");
    // 统一后的 DLQ 列表分页应该支持 -n 缩写（原来仅支持 -s，这在 TDD 中为“红灯”）
    cmd.args(&["dlq", "list", "-n", "5"]);
    cmd.assert().success();
}



#[test]
fn test_config_list_json_format() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();
    let _daemon = start_daemon_for_test(&home, "default");

    let mut cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "1");
    cmd.arg("-o").arg("json");
    cmd.arg("config").arg("list");
    
    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("STDERR: {}", stderr);
    println!("STDOUT: {}", stdout);
    
    assert!(stdout.contains("\"openapi_url\":"));
}

#[test]
fn test_reset_specific_profile_only() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();
    let _daemon = start_daemon_for_test(&home, "profile_reset");
    
    let app_config_path = dir.path().join("app.yaml");
    let profile_keep_config = dir.path().join("profile_keep.yaml");
    std::fs::write(&profile_keep_config, "app_key: \"keep_key\"").unwrap();
    let profile_keep_db = dir.path().join("profile_keep.db");
    std::fs::write(&profile_keep_db, "mock keep db").unwrap();

    let profile_reset_config = dir.path().join("profile_reset.yaml");
    let profile_reset_db = dir.path().join("profile_reset.db");
    std::fs::write(&profile_reset_db, "mock reset db").unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "1");
    cmd.args(&["-p", "profile_reset", "reset"]);
    cmd.assert().success();

    assert!(!profile_reset_config.exists(), "profile_reset.yaml should have been deleted");
    assert!(!profile_reset_db.exists(), "profile_reset.db should have been deleted");
    assert!(profile_keep_config.exists(), "profile_keep.yaml should NOT have been deleted");
    assert!(profile_keep_db.exists(), "profile_keep.db should NOT have been deleted");
    assert!(app_config_path.exists(), "app.yaml should NOT have been deleted");
}

#[test]
fn test_reset_all_profiles() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();
    let _daemon = start_daemon_for_test(&home, "profile_reset");
    
    let app_config_path = dir.path().join("app.yaml");
    let profile_keep_config = dir.path().join("profile_keep.yaml");
    std::fs::write(&profile_keep_config, "app_key: \"keep_key\"").unwrap();
    let profile_keep_db = dir.path().join("profile_keep.db");
    std::fs::write(&profile_keep_db, "mock keep db").unwrap();

    let profile_reset_config = dir.path().join("profile_reset.yaml");
    let profile_reset_db = dir.path().join("profile_reset.db");
    std::fs::write(&profile_reset_db, "mock reset db").unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "1");
    cmd.args(&["reset", "-a"]);
    cmd.assert().success();

    assert!(!profile_reset_config.exists(), "profile_reset.yaml should have been deleted");
    assert!(!profile_reset_db.exists(), "profile_reset.db should have been deleted");
    assert!(!profile_keep_config.exists(), "profile_keep.yaml should have been deleted");
    assert!(!profile_keep_db.exists(), "profile_keep.db should have been deleted");
    assert!(!app_config_path.exists(), "app.yaml should have been deleted");
}

#[test]
fn test_reset_active_profile_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();
    let _daemon = start_daemon_for_test(&home, "profile_reset");
    
    let current_profile_path = dir.path().join("current_profile");
    std::fs::write(&current_profile_path, "profile_reset").unwrap();

    let app_config_path = dir.path().join("app.yaml");
    let profile_keep_config = dir.path().join("profile_keep.yaml");
    std::fs::write(&profile_keep_config, "app_key: \"keep_key\"").unwrap();
    let profile_keep_db = dir.path().join("profile_keep.db");
    std::fs::write(&profile_keep_db, "mock keep db").unwrap();

    let profile_reset_config = dir.path().join("profile_reset.yaml");
    let profile_reset_db = dir.path().join("profile_reset.db");
    std::fs::write(&profile_reset_db, "mock reset db").unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "1");
    cmd.args(&["reset"]);
    cmd.assert().success();

    assert!(!profile_reset_config.exists(), "profile_reset.yaml should have been deleted");
    assert!(!profile_reset_db.exists(), "profile_reset.db should have been deleted");
    assert!(profile_keep_config.exists(), "profile_keep.yaml should NOT have been deleted");
    assert!(profile_keep_db.exists(), "profile_keep.db should NOT have been deleted");
    assert!(app_config_path.exists(), "app.yaml should NOT have been deleted");
}



