use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread")]
async fn test_status_all() {
    let dir = tempdir().unwrap();
    let home = dir.path();
    let home_str = home.to_str().unwrap();

    let (mock_port, _state) = crate::e2e::rust::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}/connect", mock_port);

    // Profile 1: healthy
    let mut cmd1 = Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    cmd1.env("COWEN_HOME", home_str);
    cmd1.args([
        "init",
        "--profile",
        "healthy",
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_HEALTHY",
        "--app-secret",
        "AS_HEALTHY",
        "--certificate",
        "CERT_HEALTHY",
        "--encrypt-key",
        "1234567890123456",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--proxy-port",
        "0",
    ]);
    assert!(cmd1.status().unwrap().success());

    // Profile 2: expired
    let mut cmd2 = Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    cmd2.env("COWEN_HOME", home_str);
    cmd2.args([
        "init",
        "--profile",
        "expired",
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_EXPIRED",
        "--app-secret",
        "AS_EXPIRED",
        "--certificate",
        "CERT_EXPIRED",
        "--encrypt-key",
        "1234567890123456",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--proxy-port",
        "0",
    ]);
    assert!(cmd2.status().unwrap().success());

    // Profile 3: broken
    fs::create_dir_all(home.join("broken")).unwrap();
    fs::write(
        home.join("broken.yaml"),
        "storage:\n  store: unknown_store\n",
    )
    .unwrap();

    // Profile 4: Empty name (should be ignored)
    fs::write(home.join(".yaml"), "").unwrap();

    // Run status --all
    let mut status_cmd = Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    status_cmd.env("COWEN_HOME", home_str);
    status_cmd.args(["status", "--all"]);

    let output = status_cmd.output().unwrap();
    let out_str = String::from_utf8_lossy(&output.stdout).to_string();
    let err_str = String::from_utf8_lossy(&output.stderr).to_string();
    let full_out = format!("{}\n{}", out_str, err_str);

    assert!(
        full_out.contains("healthy"),
        "Missing 'healthy' profile in output"
    );
    assert!(
        full_out.contains("expired"),
        "Missing 'expired' profile in output"
    );
    assert!(
        full_out.contains("broken") || full_out.contains("Profile load failed"),
        "Missing error for 'broken' profile"
    );
    assert!(
        full_out.contains("Storage: Mode: innerdb"),
        "Storage mode 'innerdb' missing"
    );

    // The bash script does:
    // if echo "$OUT" | grep -q "Profile: ''" || echo "$OUT" | grep -q "Profile: \"\""; then fail
    assert!(
        !full_out.contains("Profile: ''") && !full_out.contains("Profile: \"\""),
        "Empty profile was incorrectly detected"
    );
}
