#![allow(unused_imports, unused_variables, dead_code)]

use tempfile::tempdir;

#[tokio::test]
async fn test_signer_verification() {
    let dir = tempdir().unwrap();
    let home = dir.path().join(".cowen");
    std::fs::create_dir_all(&home).unwrap();
    
    let signer_bin = assert_cmd::cargo::cargo_bin("cowen-signer");
    
    // 1. GenerateRoot
    let mut gen_cmd = std::process::Command::new(&signer_bin);
    gen_cmd.current_dir(dir.path()).args([
        "generate-root", "--out-root-key", "root.pk8", "--out-root-pub", "root.pub"
    ]);
    assert!(gen_cmd.status().unwrap().success());
    assert!(dir.path().join("root.pk8").exists());
    
    // 2. IssueCert
    let mut issue_cmd = std::process::Command::new(&signer_bin);
    issue_cmd.current_dir(dir.path()).args([
        "issue-cert", "--root-key", "root.pk8", "--dev-id", "test-developer",
        "--out-dev-key", "dev.pk8", "--out-cert", "dev_cert.json",
        "--days", "30", "--org", "TestOrg", "--country", "CN"
    ]);
    assert!(issue_cmd.status().unwrap().success());
    assert!(dir.path().join("dev.pk8").exists());
    
    let cert_content = std::fs::read_to_string(dir.path().join("dev_cert.json")).unwrap();
    assert!(cert_content.contains("test-developer"));
    
    // 3. SignPlugin
    std::fs::write(dir.path().join("dummy.dylib"), "mock_dylib_bytes").unwrap();
    std::fs::write(dir.path().join("plugin.json"), r#"
{
  "required_capabilities": {
    "auth": {}
  },
  "requested_permissions": {
    "storage": {}
  },
  "transport": "uds"
}
"#).unwrap();

    let mut sign_cmd = std::process::Command::new(&signer_bin);
    sign_cmd.current_dir(dir.path()).args([
        "sign-plugin", "--dylib", "dummy.dylib", "--name", "mock-plugin",
        "--version", "1.0.0", "--dev-key", "dev.pk8", "--dev-cert", "dev_cert.json",
        "--out-bundle", "signature.bundle", "--manifest-file", "plugin.json"
    ]);
    assert!(sign_cmd.status().unwrap().success());
    
    let bundle = std::fs::read_to_string(dir.path().join("signature.bundle")).unwrap();
    assert!(bundle.contains("mock-plugin"));
    assert!(bundle.contains("test-developer"));
}
