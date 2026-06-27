use assert_cmd::Command;

fn setup_empty_env() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn test_doctor_diagnostics() {
    let dir = setup_empty_env();
    let cowen_home = dir.path().to_str().unwrap();
    let test_profile = "doctor_test_prof";

    // 1. Run doctor on uninitialized profile (defaults to oauth2 mode)
    // OAuth2 mode does NOT require app_secret — it uses built-in client ID with PKCE.
    // Credentials check should NOT report "App Secret 缺失".
    let mut doctor_cmd = Command::cargo_bin("cowen").unwrap();
    doctor_cmd
        .env("COWEN_HOME", cowen_home)
        .arg("doctor")
        .arg("--profile")
        .arg(test_profile);

    // Depending on the implementation, doctor might return non-zero if issues are found,
    // so we just capture output
    let output = doctor_cmd.output().expect("Failed to execute doctor");
    let out_str = String::from_utf8_lossy(&output.stdout);
    let err_str = String::from_utf8_lossy(&output.stderr);
    let full_output = format!("{}\n{}", out_str, err_str);

    assert!(
        !full_output.contains("App Secret 缺失"),
        "OAuth2 mode should NOT report 'App Secret 缺失'. Output: {}",
        full_output
    );

    // 2. Initialize self-built with valid 16-byte key and run doctor
    let selfbuilt_profile = "doctor_selfbuilt_prof";
    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd
        .env("COWEN_HOME", cowen_home)
        .arg("init")
        .arg("--profile")
        .arg(selfbuilt_profile)
        .arg("--app-mode")
        .arg("self-built")
        .arg("--app-key")
        .arg("k")
        .arg("--app-secret")
        .arg("1234567890123456")
        .arg("--certificate")
        .arg("c")
        .arg("--encrypt-key")
        .arg("1234567890123456")
        .arg("--stream-url")
        .arg("http://localhost:8080");
    init_cmd.assert().success();

    let mut doctor_sb_cmd = Command::cargo_bin("cowen").unwrap();
    doctor_sb_cmd
        .env("COWEN_HOME", cowen_home)
        .arg("doctor")
        .arg("--profile")
        .arg(selfbuilt_profile);

    let output_sb = doctor_sb_cmd.output().expect("Failed to execute doctor");
    let out_sb_str = String::from_utf8_lossy(&output_sb.stdout);
    let err_sb_str = String::from_utf8_lossy(&output_sb.stderr);
    let full_sb_output = format!("{}\n{}", out_sb_str, err_sb_str);

    assert!(
        !full_sb_output.contains("缺少解密密钥") && !full_sb_output.contains("解密密钥不合规"),
        "Doctor reported decrypt key error for self-built profile with valid key. Output: {}",
        full_sb_output
    );

    // 3. Verify network checks are present
    assert!(
        full_sb_output.contains("OpenAPI"),
        "Doctor output missing network check for OpenAPI. Output: {}",
        full_sb_output
    );
}
