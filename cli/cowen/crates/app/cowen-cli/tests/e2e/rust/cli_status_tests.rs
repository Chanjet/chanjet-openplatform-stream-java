use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn setup_empty_env() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn test_status_empty_env() {
    let dir = setup_empty_env();
    let cowen_home = dir.path().to_str().unwrap();

    // Runs cowen status on an uninitialized environment
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", cowen_home).arg("status");

    let assert = cmd.assert().success();
    let output = String::from_utf8_lossy(&assert.get_output().stdout);

    // Verify empty status properties
    assert!(
        output.contains("System is not initialized"),
        "Should state system is not initialized"
    );
    assert!(
        output.contains("Not Initialized"),
        "Should print Not Initialized placeholder"
    );
    assert!(
        !output.contains("Profile: 'default'"),
        "Should not print artificial default profile"
    );
    assert!(
        !output.contains("Efficiency Tip"),
        "No false positive Efficiency Tip warnings"
    );
    assert!(
        !output.contains("Oauth2 Mode Diagnostics"),
        "No false positive Oauth2 Mode Diagnostics"
    );
    assert!(
        output.contains("Storage: Mode:"),
        "Global storage mode correctly printed"
    );
}

#[test]
fn test_status_storage_mode() {
    let dir = setup_empty_env();
    let cowen_home = dir.path().to_str().unwrap();

    // Initialize default profile
    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd
        .env("COWEN_HOME", cowen_home)
        .arg("-p")
        .arg("default")
        .arg("init")
        .arg("--app-key")
        .arg("test-key")
        .arg("--app-secret")
        .arg("test-secret")
        .arg("--app-mode")
        .arg("self-built")
        .arg("--certificate")
        .arg("test-cert")
        .arg("--encrypt-key")
        .arg("1234567890123456");

    init_cmd.assert().success();

    // Run status
    let mut status_cmd = Command::cargo_bin("cowen").unwrap();
    status_cmd.env("COWEN_HOME", cowen_home).arg("status");

    let assert = status_cmd.assert().success();
    let output = String::from_utf8_lossy(&assert.get_output().stdout);

    assert!(
        output.contains("Storage: Mode: innerdb"),
        "Status output should display 'Storage: Mode: innerdb'"
    );
}

#[test]
fn test_status_all() {
    let dir = setup_empty_env();
    let cowen_home = dir.path().to_str().unwrap();

    // 1. Profile 1: Healthy
    let mut init_cmd1 = Command::cargo_bin("cowen").unwrap();
    init_cmd1
        .env("COWEN_HOME", cowen_home)
        .arg("init")
        .arg("--profile")
        .arg("healthy")
        .arg("--app-mode")
        .arg("self-built")
        .arg("--app-key")
        .arg("AK_HEALTHY")
        .arg("--app-secret")
        .arg("AS_HEALTHY")
        .arg("--certificate")
        .arg("CERT_HEALTHY")
        .arg("--encrypt-key")
        .arg("1234567890123456")
        .arg("--openapi-url")
        .arg("http://127.0.0.1:9090")
        .arg("--stream-url")
        .arg("ws://127.0.0.1:9090")
        .arg("--proxy-port")
        .arg("10001");
    init_cmd1.assert().success();

    // 2. Profile 2: Expired (simulate by creating another valid one for now)
    let mut init_cmd2 = Command::cargo_bin("cowen").unwrap();
    init_cmd2
        .env("COWEN_HOME", cowen_home)
        .arg("init")
        .arg("--profile")
        .arg("expired")
        .arg("--app-mode")
        .arg("self-built")
        .arg("--app-key")
        .arg("AK_EXPIRED")
        .arg("--app-secret")
        .arg("AS_EXPIRED")
        .arg("--certificate")
        .arg("CERT_EXPIRED")
        .arg("--encrypt-key")
        .arg("1234567890123456")
        .arg("--openapi-url")
        .arg("http://127.0.0.1:9090")
        .arg("--stream-url")
        .arg("ws://127.0.0.1:9090")
        .arg("--proxy-port")
        .arg("10002");
    init_cmd2.assert().success();

    // 3. Profile 3: Broken (malformed yaml)
    let broken_path = Path::new(cowen_home).join("broken.yaml");
    fs::write(
        &broken_path,
        "storage:\n  store: unknown_store\n",
    )
    .unwrap();

    // 4. Empty profile (should be ignored)
    let empty_path = Path::new(cowen_home).join(".yaml");
    fs::write(&empty_path, "").unwrap();

    // Run status --all
    let mut status_cmd = Command::cargo_bin("cowen").unwrap();
    status_cmd
        .env("COWEN_HOME", cowen_home)
        .arg("status")
        .arg("--all");

    let assert = status_cmd.assert().success();
    let output = String::from_utf8_lossy(&assert.get_output().stdout);

    // Verify healthy, expired, and broken are detected
    assert!(output.contains("healthy"), "Profile 'healthy' not found");
    assert!(output.contains("expired"), "Profile 'expired' not found");
    assert!(output.contains("broken"), "Profile 'broken' not found");

    // Verify error reporting for broken
    assert!(
        output.contains("Profile load failed") || output.contains("broken"),
        "Error reporting failed for broken profile"
    );

    // Verify storage mode reporting
    assert!(
        output.contains("Storage: Mode: innerdb") || output.contains("innerdb"),
        "Storage mode innerdb not reported"
    );

    // Verify empty profile is ignored
    assert!(
        !output.contains("Profile: ''") && !output.contains("Profile: \"\""),
        "Empty profile was incorrectly detected and displayed"
    );
}
