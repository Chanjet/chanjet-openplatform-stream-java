use assert_cmd::Command;

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
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "1");
    // 动态修改全局配置的局部值应该支持 --global 选项，目前代码中未实现，所以这在 TDD 流程中将是“红灯”（失败测试）
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
    
    // Create basic app config
    let config_path = dir.path().join("app.yaml");
    std::fs::write(&config_path, "openapi_url: \"http://localhost:8080\"").unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("-o").arg("json");
    cmd.arg("config").arg("list");
    
    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    assert!(stdout.contains("\"openapi_url\":"));
    
    let _ = dir;
}

#[test]
fn test_reset_specific_profile_only() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();
    
    // Create basic app config
    let app_config_path = dir.path().join("app.yaml");
    std::fs::write(&app_config_path, "openapi_url: \"http://localhost:8080\"").unwrap();

    // Create profile keep
    let profiles_dir = dir.path().join("profiles");
    std::fs::create_dir_all(&profiles_dir).unwrap();
    
    let profile_keep_config = profiles_dir.join("profile_keep.yaml");
    std::fs::write(&profile_keep_config, "app_key: \"keep_key\"").unwrap();
    let profile_keep_db = dir.path().join("profile_keep.db");
    std::fs::write(&profile_keep_db, "mock keep db").unwrap();

    // Create profile reset
    let profile_reset_config = profiles_dir.join("profile_reset.yaml");
    std::fs::write(&profile_reset_config, "app_key: \"reset_key\"").unwrap();
    let profile_reset_db = dir.path().join("profile_reset.db");
    std::fs::write(&profile_reset_db, "mock reset db").unwrap();

    // Run reset command for profile_reset
    let mut cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "1");
    cmd.args(&["-p", "profile_reset", "reset"]);
    cmd.assert().success();

    // Assert profile_reset is deleted
    assert!(!profile_reset_config.exists(), "profile_reset.yaml should have been deleted");
    assert!(!profile_reset_db.exists(), "profile_reset.db should have been deleted");

    // Assert profile_keep and app.yaml are kept intact
    assert!(profile_keep_config.exists(), "profile_keep.yaml should NOT have been deleted");
    assert!(profile_keep_db.exists(), "profile_keep.db should NOT have been deleted");
    assert!(app_config_path.exists(), "app.yaml should NOT have been deleted");
}

#[test]
fn test_reset_all_profiles() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();
    
    // Create basic app config
    let app_config_path = dir.path().join("app.yaml");
    std::fs::write(&app_config_path, "openapi_url: \"http://localhost:8080\"").unwrap();

    // Create profile keep
    let profiles_dir = dir.path().join("profiles");
    std::fs::create_dir_all(&profiles_dir).unwrap();
    
    let profile_keep_config = profiles_dir.join("profile_keep.yaml");
    std::fs::write(&profile_keep_config, "app_key: \"keep_key\"").unwrap();
    let profile_keep_db = dir.path().join("profile_keep.db");
    std::fs::write(&profile_keep_db, "mock keep db").unwrap();

    // Create profile reset
    let profile_reset_config = profiles_dir.join("profile_reset.yaml");
    std::fs::write(&profile_reset_config, "app_key: \"reset_key\"").unwrap();
    let profile_reset_db = dir.path().join("profile_reset.db");
    std::fs::write(&profile_reset_db, "mock reset db").unwrap();

    // Run reset command with -a
    let mut cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "1");
    cmd.args(&["reset", "-a"]);
    cmd.assert().success();

    // Assert ALL profiles and configurations are deleted
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
    
    // Create basic app config
    let app_config_path = dir.path().join("app.yaml");
    std::fs::write(&app_config_path, "openapi_url: \"http://localhost:8080\"").unwrap();

    // Create current_profile specifying "profile_reset"
    let current_profile_path = dir.path().join("current_profile");
    std::fs::write(&current_profile_path, "profile_reset").unwrap();

    // Create profile keep
    let profiles_dir = dir.path().join("profiles");
    std::fs::create_dir_all(&profiles_dir).unwrap();
    
    let profile_keep_config = profiles_dir.join("profile_keep.yaml");
    std::fs::write(&profile_keep_config, "app_key: \"keep_key\"").unwrap();
    let profile_keep_db = dir.path().join("profile_keep.db");
    std::fs::write(&profile_keep_db, "mock keep db").unwrap();

    // Create profile reset
    let profile_reset_config = profiles_dir.join("profile_reset.yaml");
    std::fs::write(&profile_reset_config, "app_key: \"reset_key\"").unwrap();
    let profile_reset_db = dir.path().join("profile_reset.db");
    std::fs::write(&profile_reset_db, "mock reset db").unwrap();

    // Run reset command without -p or -a
    let mut cmd = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "1");
    cmd.args(&["reset"]);
    cmd.assert().success();

    // Assert profile_reset is deleted
    assert!(!profile_reset_config.exists(), "profile_reset.yaml should have been deleted");
    assert!(!profile_reset_db.exists(), "profile_reset.db should have been deleted");

    // Assert profile_keep and app.yaml are kept intact
    assert!(profile_keep_config.exists(), "profile_keep.yaml should NOT have been deleted");
    assert!(profile_keep_db.exists(), "profile_keep.db should NOT have been deleted");
    assert!(app_config_path.exists(), "app.yaml should NOT have been deleted");
}



