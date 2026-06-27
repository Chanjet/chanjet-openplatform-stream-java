use assert_cmd::Command;
use std::fs;

#[tokio::test(flavor = "multi_thread")]
async fn test_profile_management() {
    let (mock_port, _state) = crate::e2e::rust::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);

    // Given: A clean environment with mock API server running
    let dir = tempfile::tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    fs::create_dir_all(&cowen_home).unwrap();
    let home_str = cowen_home.to_str().unwrap().to_string();

    // When: Initialize multiple profiles (prof_a and prof_b)
    let mut cmd_init_a = Command::cargo_bin("cowen").unwrap();
    cmd_init_a.env("COWEN_HOME", &home_str);
    cmd_init_a.env("HOME", &home_str);
    cmd_init_a.args([
        "init",
        "--profile",
        "prof_a",
        "--app-mode",
        "self-built",
        "--app-key",
        "KEY_A",
        "--app-secret",
        "SEC_A",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "CERT_A",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
    ]);
    cmd_init_a.assert().success();

    let mut cmd_init_b = Command::cargo_bin("cowen").unwrap();
    cmd_init_b.env("COWEN_HOME", &home_str);
    cmd_init_b.env("HOME", &home_str);
    cmd_init_b.args([
        "init",
        "--profile",
        "prof_b",
        "--app-mode",
        "self-built",
        "--app-key",
        "KEY_B",
        "--app-secret",
        "SEC_B",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "CERT_B",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
    ]);
    cmd_init_b.assert().success();

    // 2. Check current profile
    let mut cmd_current = Command::cargo_bin("cowen").unwrap();
    cmd_current.env("COWEN_HOME", &home_str);
    cmd_current.env("HOME", &home_str);
    cmd_current.args(["profile", "current"]);
    let current_out = cmd_current.output().unwrap();
    let current_str = String::from_utf8_lossy(&current_out.stdout);
    println!("Current profile: {}", current_str);

    // Then: Profile list shows both profiles
    let mut cmd_list = Command::cargo_bin("cowen").unwrap();
    cmd_list.env("COWEN_HOME", &home_str);
    cmd_list.env("HOME", &home_str);
    cmd_list.args(["profile", "list"]);
    let list_out = String::from_utf8_lossy(&cmd_list.output().unwrap().stdout).to_string();
    assert!(list_out.contains("prof_a"), "List doesn't contain prof_a");
    assert!(list_out.contains("prof_b"), "List doesn't contain prof_b");

    // When: We use prof_a
    let mut cmd_use_a = Command::cargo_bin("cowen").unwrap();
    cmd_use_a.env("COWEN_HOME", &home_str);
    cmd_use_a.env("HOME", &home_str);
    cmd_use_a.args(["profile", "use", "prof_a"]);
    cmd_use_a.assert().success();

    let mut cmd_cur2 = Command::cargo_bin("cowen").unwrap();
    cmd_cur2.env("COWEN_HOME", &home_str);
    cmd_cur2.env("HOME", &home_str);
    cmd_cur2.args(["profile", "current"]);
    let cur2_out = String::from_utf8_lossy(&cmd_cur2.output().unwrap().stdout).to_string();
    assert!(
        cur2_out.contains("prof_a"),
        "Current profile is not prof_a: {}",
        cur2_out
    );

    // When: We try to switch to an invalid profile
    let mut cmd_use_invalid = Command::cargo_bin("cowen").unwrap();
    cmd_use_invalid.env("COWEN_HOME", &home_str);
    cmd_use_invalid.env("HOME", &home_str);
    cmd_use_invalid.args(["profile", "use", "nonexistent"]);
    cmd_use_invalid.assert().failure(); // should block

    let mut cmd_cur3 = Command::cargo_bin("cowen").unwrap();
    cmd_cur3.env("COWEN_HOME", &home_str);
    cmd_cur3.env("HOME", &home_str);
    cmd_cur3.args(["profile", "current"]);
    let cur3_out = String::from_utf8_lossy(&cmd_cur3.output().unwrap().stdout).to_string();
    assert!(
        cur3_out.contains("prof_a"),
        "Profile was incorrectly switched: {}",
        cur3_out
    );

    // Then: Ensure the previous config matches prof_a config
    let mut cmd_cfg_a = Command::cargo_bin("cowen").unwrap();
    cmd_cfg_a.env("COWEN_HOME", &home_str);
    cmd_cfg_a.env("HOME", &home_str);
    cmd_cfg_a.args(["config"]); // relies on current profile which is prof_a
    let cfg_a_out = String::from_utf8_lossy(&cmd_cfg_a.output().unwrap().stdout).to_string();
    assert!(cfg_a_out.contains("KEY_A"), "Config doesn't contain KEY_A");

    // When: We rename prof_b to prof_c
    let mut cmd_ren = Command::cargo_bin("cowen").unwrap();
    cmd_ren.env("COWEN_HOME", &home_str);
    cmd_ren.env("HOME", &home_str);
    cmd_ren.args(["profile", "rename", "prof_b", "prof_c"]);
    cmd_ren.assert().success();

    let mut cmd_list2 = Command::cargo_bin("cowen").unwrap();
    cmd_list2.env("COWEN_HOME", &home_str);
    cmd_list2.env("HOME", &home_str);
    cmd_list2.args(["profile", "list"]);
    let list2_out = String::from_utf8_lossy(&cmd_list2.output().unwrap().stdout).to_string();
    assert!(list2_out.contains("prof_c"), "List doesn't contain prof_c");
    assert!(!list2_out.contains("prof_b"), "List still contains prof_b");

    // Then: We can use prof_c and its migrated config is intact
    let mut cmd_use_c = Command::cargo_bin("cowen").unwrap();
    cmd_use_c.env("COWEN_HOME", &home_str);
    cmd_use_c.env("HOME", &home_str);
    cmd_use_c.args(["profile", "use", "prof_c"]);
    cmd_use_c.assert().success();

    let mut cmd_cfg_c = Command::cargo_bin("cowen").unwrap();
    cmd_cfg_c.env("COWEN_HOME", &home_str);
    cmd_cfg_c.env("HOME", &home_str);
    cmd_cfg_c.args(["config"]); // current is now prof_c
    let cfg_c_out = String::from_utf8_lossy(&cmd_cfg_c.output().unwrap().stdout).to_string();
    assert!(
        cfg_c_out.contains("KEY_B"),
        "Config doesn't contain migrated KEY_B"
    );

    // Verify token was also migrated by checking status
    let mut cmd_status_c = Command::cargo_bin("cowen").unwrap();
    cmd_status_c.env("COWEN_HOME", &home_str);
    cmd_status_c.env("HOME", &home_str);
    cmd_status_c.args(["status"]);
    let status_c_out = String::from_utf8_lossy(&cmd_status_c.output().unwrap().stdout).to_string();
    assert!(
        !status_c_out.contains("Not logged in"),
        "Authentication lost after rename! token migration failed: {}",
        status_c_out
    );

    // When: We switch back to prof_a and issue reset
    let mut cmd_use_a_again = Command::cargo_bin("cowen").unwrap();
    cmd_use_a_again.env("COWEN_HOME", &home_str);
    cmd_use_a_again.env("HOME", &home_str);
    cmd_use_a_again.args(["profile", "use", "prof_a"]);
    cmd_use_a_again.assert().success();

    let mut cmd_reset = Command::cargo_bin("cowen").unwrap();
    cmd_reset.env("COWEN_HOME", &home_str);
    cmd_reset.env("HOME", &home_str);
    cmd_reset.args(["reset"]);
    cmd_reset.assert().success();

    // Then: Profile a config is reset and its app-key is gone
    let mut cmd_cfg_reset = Command::cargo_bin("cowen").unwrap();
    cmd_cfg_reset.env("COWEN_HOME", &home_str);
    cmd_cfg_reset.env("HOME", &home_str);
    cmd_cfg_reset.args(["config"]);
    let cfg_reset_out =
        String::from_utf8_lossy(&cmd_cfg_reset.output().unwrap().stdout).to_string();
    assert!(
        !cfg_reset_out.contains("KEY_A"),
        "Config should not contain KEY_A after reset"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reset_profile_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let home_str = dir.path().to_str().unwrap().to_string();

    let init = |profile: &str, app_key: &str| {
        let mut cmd = Command::cargo_bin("cowen").unwrap();
        cmd.env("COWEN_HOME", &home_str);
        cmd.env("HOME", &home_str);
        cmd.args([
            "init",
            "-p",
            profile,
            "--app-mode",
            "self-built",
            "--app-key",
            app_key,
            "--app-secret",
            "sec",
            "--certificate",
            "cert",
            "--encrypt-key",
            "1234567890123456",
        ])
        .assert()
        .success();
    };

    init("profile_a", "key_a");
    init("profile_b", "key_b");

    // Switch to profile_a
    Command::cargo_bin("cowen")
        .unwrap()
        .env("COWEN_HOME", &home_str)
        .env("HOME", &home_str)
        .args(["profile", "use", "profile_a"])
        .assert()
        .success();

    // Reset current profile_a
    Command::cargo_bin("cowen")
        .unwrap()
        .env("COWEN_HOME", &home_str)
        .env("HOME", &home_str)
        .args(["reset", "-p", "profile_a"])
        .assert()
        .success();

    // Verify it disappeared from list
    let list_out = String::from_utf8_lossy(
        &Command::cargo_bin("cowen")
            .unwrap()
            .env("COWEN_HOME", &home_str)
            .env("HOME", &home_str)
            .args(["profile", "list"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert!(!list_out.contains("profile_a"));
    assert!(list_out.contains("profile_b"));

    // Verify fallback to another profile
    let current = String::from_utf8_lossy(
        &Command::cargo_bin("cowen")
            .unwrap()
            .env("COWEN_HOME", &home_str)
            .env("HOME", &home_str)
            .args(["profile", "current"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert!(
        current.trim() == "profile_b" || current.trim() == "default",
        "Fell back to: {}",
        current
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reset_behaviors() {
    let dir = tempfile::tempdir().unwrap();
    let home_str = dir.path().to_str().unwrap().to_string();

    let init = |profile: &str| {
        let mut cmd = Command::cargo_bin("cowen").unwrap();
        cmd.env("COWEN_HOME", &home_str);
        cmd.env("HOME", &home_str);
        cmd.args([
            "init",
            "-p",
            profile,
            "--app-mode",
            "self-built",
            "--app-key",
            "ak",
            "--app-secret",
            "as",
            "--certificate",
            "cert",
            "--encrypt-key",
            "1234567890123456",
        ])
        .assert()
        .success();
    };

    // Test missing keys after reset
    init("p2");
    Command::cargo_bin("cowen")
        .unwrap()
        .env("COWEN_HOME", &home_str)
        .env("HOME", &home_str)
        .args(["reset", "-p", "p2"])
        .assert()
        .success();

    // Init without keys should fail
    Command::cargo_bin("cowen")
        .unwrap()
        .env("COWEN_HOME", &home_str)
        .env("HOME", &home_str)
        .args(["init", "-p", "p2", "--app-mode", "self-built"])
        .assert()
        .failure();

    // Test Full Reset
    init("p3");
    Command::cargo_bin("cowen")
        .unwrap()
        .env("COWEN_HOME", &home_str)
        .env("HOME", &home_str)
        .args(["reset"])
        .assert()
        .success();

    let list_out = String::from_utf8_lossy(
        &Command::cargo_bin("cowen")
            .unwrap()
            .env("COWEN_HOME", &home_str)
            .env("HOME", &home_str)
            .args(["profile", "list"])
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();
    assert!(!list_out.contains("p3"));
}
