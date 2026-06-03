use assert_cmd::Command;

fn start_daemon_for_test(home: &str, profile: &str) -> std::process::Child {
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
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    
    std::thread::sleep(std::time::Duration::from_millis(1500));
    child
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
    let mut daemon = start_daemon_for_test(&home, "default");
    
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "1");
    cmd.args(&["config", "set", "log.level", "debug", "--global"]);
    cmd.assert().success();
    let _ = daemon.kill();
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
    let mut daemon = start_daemon_for_test(&home, "default");

    let mut cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "1");
    cmd.arg("-o").arg("json");
    cmd.arg("config").arg("list");
    
    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("STDOUT: {}", stdout);
    
    assert!(stdout.contains("\"openapi_url\":"));
    let _ = daemon.kill();
}

#[test]
fn test_reset_specific_profile_only() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();
    let mut daemon = start_daemon_for_test(&home, "profile_reset");
    
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
    let _ = daemon.kill();
}

#[test]
fn test_reset_all_profiles() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();
    let mut daemon = start_daemon_for_test(&home, "profile_reset");
    
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
    let _ = daemon.kill();
}

#[test]
fn test_reset_active_profile_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();
    let mut daemon = start_daemon_for_test(&home, "profile_reset");
    
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
    let _ = daemon.kill();
}



