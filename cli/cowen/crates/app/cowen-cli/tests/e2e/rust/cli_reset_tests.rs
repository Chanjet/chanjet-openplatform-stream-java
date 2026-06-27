use assert_cmd::Command;

#[tokio::test(flavor = "multi_thread")]
async fn test_modular_reset() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();

    // 1. Init
    let mut cmd_init = Command::cargo_bin("cowen").unwrap();
    cmd_init.env("COWEN_HOME", &home);
    cmd_init.env("HOME", &home);
    cmd_init.args([
        "init",
        "--profile",
        "case_55_reset",
        "--app-key",
        "dummy-reset-key",
        "--app-secret",
        "dummy-secret",
        "--certificate",
        "dummy-cert",
        "--app-mode",
        "self-built",
        "--encrypt-key",
        "dummy-encrypt-key",
    ]);
    cmd_init.assert().success();

    // 2. Generate dummy telemetry/logs
    let cowen_home = std::path::Path::new(&home);
    std::fs::write(cowen_home.join("telemetry.db"), "").unwrap();
    std::fs::create_dir_all(cowen_home.join("logs")).unwrap();
    std::fs::write(cowen_home.join("logs").join("test.log"), "").unwrap();

    // 3. Dry run reset
    let mut cmd_dry = Command::cargo_bin("cowen").unwrap();
    cmd_dry.env("COWEN_HOME", &home);
    cmd_dry.env("HOME", &home);
    cmd_dry.args(["reset", "--dry-run"]);
    let out_dry = String::from_utf8_lossy(&cmd_dry.output().unwrap().stdout).to_string();

    assert!(out_dry.contains("[DRY RUN]"), "Missing DRY RUN header");
    assert!(
        out_dry.contains("telemetry.db") || out_dry.contains("logs"),
        "Missing plan items: {}",
        out_dry
    );
    assert!(
        cowen_home.join("telemetry.db").exists(),
        "Dry run deleted telemetry.db!"
    );

    // 4. Actual reset
    let mut cmd_reset = Command::cargo_bin("cowen").unwrap();
    cmd_reset.env("COWEN_HOME", &home);
    cmd_reset.env("HOME", &home);
    cmd_reset.args(["reset"]);

    // We have to pipe 'y' to stdin since reset is interactive?
    use std::io::Write;
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home)
        .env("HOME", &home)
        .args(["reset"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(b"y\n").unwrap();
    }
    let status = child.wait().unwrap();
    assert!(status.success());

    assert!(
        !cowen_home.join("telemetry.db").exists(),
        "telemetry.db not deleted"
    );
    assert!(!cowen_home.join("logs").exists(), "logs dir not deleted");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reset_reinit_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();

    // 1. Init
    let mut cmd_init = Command::cargo_bin("cowen").unwrap();
    cmd_init.env("COWEN_HOME", &home);
    cmd_init.env("HOME", &home);
    cmd_init.args([
        "init",
        "--profile",
        "case_58_reset",
        "--app-key",
        "dummy-reset-key",
        "--app-secret",
        "dummy-secret",
        "--certificate",
        "dummy-cert",
        "--app-mode",
        "self-built",
        "--encrypt-key",
        "dummy-encrypt-key",
    ]);
    cmd_init.assert().success();

    let cowen_home = std::path::Path::new(&home);
    assert!(cowen_home.join("cowen.db").exists());
    assert!(cowen_home.join("case_58_reset.yaml").exists());

    // Create dummy WAL and SHM
    std::fs::write(cowen_home.join("cowen.db-wal"), "").unwrap();
    std::fs::write(cowen_home.join("cowen.db-shm"), "").unwrap();
    std::fs::write(cowen_home.join("cowen.ddl.lock"), "").unwrap();
    std::fs::write(cowen_home.join("telemetry.db"), "").unwrap();
    std::fs::write(cowen_home.join("telemetry.db-wal"), "").unwrap();
    std::fs::write(cowen_home.join("telemetry.db-shm"), "").unwrap();

    // 2. Reset --all
    let mut cmd_reset = Command::cargo_bin("cowen").unwrap();
    cmd_reset.env("COWEN_HOME", &home);
    cmd_reset.env("HOME", &home);
    cmd_reset.args(["reset", "--all"]);

    // Pipe 'y'
    use std::io::Write;
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home)
        .env("HOME", &home)
        .args(["reset", "--all"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(b"y\n").unwrap();
    }
    let status = child.wait().unwrap();
    assert!(status.success());

    // Verify all deleted
    assert!(!cowen_home.join("cowen.db").exists());
    assert!(!cowen_home.join("cowen.db-wal").exists());
    assert!(!cowen_home.join("cowen.db-shm").exists());
    assert!(!cowen_home.join("cowen.ddl.lock").exists());
    assert!(!cowen_home.join("telemetry.db").exists());
    assert!(!cowen_home.join("telemetry.db-wal").exists());
    assert!(!cowen_home.join("telemetry.db-shm").exists());

    // 3. Re-init to verify no disk I/O errors
    let mut cmd_reinit = Command::cargo_bin("cowen").unwrap();
    cmd_reinit.env("COWEN_HOME", &home);
    cmd_reinit.env("HOME", &home);
    cmd_reinit.args([
        "init",
        "--profile",
        "case_58_reset",
        "--app-key",
        "new-dummy-reset-key",
        "--app-secret",
        "new-dummy-secret",
        "--certificate",
        "new-dummy-cert",
        "--app-mode",
        "self-built",
        "--encrypt-key",
        "new-dummy-encrypt-key",
    ]);
    cmd_reinit.assert().success();
}
