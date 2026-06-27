use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_signer_verification() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().to_str().unwrap().to_string();

    fs::create_dir_all(&cowen_home).unwrap();

    // 1. GenerateRoot
    let mut gen_cmd = Command::cargo_bin("cowen-signer").unwrap();
    gen_cmd.current_dir(&cowen_home);
    gen_cmd.args([
        "generate-root",
        "--out-root-key",
        "root.pk8",
        "--out-root-pub",
        "root.pub",
    ]);
    gen_cmd.assert().success();

    let root_pk8 = dir.path().join("root.pk8");
    let root_pub = dir.path().join("root.pub");
    assert!(root_pk8.exists());
    assert!(root_pub.exists());

    // 2. IssueCert
    let mut issue_cmd = Command::cargo_bin("cowen-signer").unwrap();
    issue_cmd.current_dir(&cowen_home);
    issue_cmd.args([
        "issue-cert",
        "--root-key",
        "root.pk8",
        "--dev-id",
        "test-developer",
        "--out-dev-key",
        "dev.pk8",
        "--out-cert",
        "dev_cert.json",
        "--days",
        "30",
        "--org",
        "TestOrg",
        "--country",
        "CN",
    ]);
    issue_cmd.assert().success();

    let dev_pk8 = dir.path().join("dev.pk8");
    let dev_cert = dir.path().join("dev_cert.json");
    assert!(dev_pk8.exists());
    assert!(dev_cert.exists());

    let cert_content = fs::read_to_string(&dev_cert).unwrap();
    assert!(cert_content.contains("test-developer"));

    // 3. SignPlugin
    let dylib_path = dir.path().join("dummy.dylib");
    fs::write(&dylib_path, "mock_dylib_bytes").unwrap();

    let plugin_json_path = dir.path().join("plugin.json");
    fs::write(
        &plugin_json_path,
        r#"{
  "required_capabilities": {
    "auth": {}
  },
  "requested_permissions": {
    "storage": {}
  },
  "transport": "uds"
}"#,
    )
    .unwrap();

    let mut sign_cmd = Command::cargo_bin("cowen-signer").unwrap();
    sign_cmd.current_dir(&cowen_home);
    sign_cmd.args([
        "sign-plugin",
        "--dylib",
        "dummy.dylib",
        "--name",
        "mock-plugin",
        "--version",
        "1.0.0",
        "--dev-key",
        "dev.pk8",
        "--dev-cert",
        "dev_cert.json",
        "--out-bundle",
        "signature.bundle",
        "--manifest-file",
        "plugin.json",
    ]);
    sign_cmd.assert().success();

    let signature_bundle = dir.path().join("signature.bundle");
    assert!(signature_bundle.exists());

    let bundle_content = fs::read_to_string(&signature_bundle).unwrap();
    assert!(bundle_content.contains("mock-plugin"));
    assert!(bundle_content.contains("test-developer"));
}
