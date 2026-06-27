#![allow(unused_imports, unused_variables, dead_code)]

use tempfile::tempdir;

#[tokio::test]
async fn test_sealed_storage() {
    let profile = "case_85_sealed";
    let dir = tempdir().unwrap();
    let home = dir.path().join(".cowen");
    std::fs::create_dir_all(&home).unwrap();
    
    std::fs::write(home.join("app.yaml"), r#"
storage:
  store: local
log:
  level: debug
"#).unwrap();
    
    std::fs::write(home.join(".seal"), "").unwrap();
    
    let mut init_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "init", "--profile", profile, "--app-key", "dummykey",
        "--app-secret", "dummysecret", "--app-mode", "self-built",
        "--certificate", "dummy_cert", "--encrypt-key", "supersecret"
    ]);
    assert!(init_cmd.status().unwrap().success());
    
    assert!(home.join("vault").exists());
    assert!(home.join(".seal").exists());
    
    let mut set_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    set_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "--profile", profile, "config", "set", "webhook_target", "http://localhost:9999"
    ]);
    assert!(set_cmd.status().unwrap().success());
    
    let get_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["--profile", profile, "config", "get", "webhook_target"])
        .output().unwrap();
    let val = String::from_utf8_lossy(&get_cmd.stdout).trim().to_string();
    assert_eq!(val, ""http://localhost:9999""); // get prints json usually, or plain? The bash script expects http://localhost:9999 directly. Wait! config get prints json string.
    
    let sec_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["--profile", profile, "config", "get", "encrypt_key"])
        .output().unwrap();
    let sec_val = String::from_utf8_lossy(&sec_cmd.stdout).trim().trim_matches('"').to_string();
    assert_eq!(sec_val, "supersecret");
    
    let list_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["--profile", profile, "config", "list"])
        .output().unwrap();
    let list_val = String::from_utf8_lossy(&list_cmd.stdout);
    assert!(list_val.contains("webhook_target"));
}

#[tokio::test]
async fn test_storage_migration() {
    let profile = "case_88_mig";
    let dir = tempdir().unwrap();
    let home = dir.path().join(".cowen");
    std::fs::create_dir_all(&home).unwrap();
    
    std::fs::write(home.join("app.yaml"), r#"
storage:
  store: local
log:
  level: debug
"#).unwrap();
    
    let vault = home.join("vault");
    let prof_dir = vault.join(profile);
    std::fs::create_dir_all(&prof_dir).unwrap();
    
    let json_v2 = format!(r#"
{{
  "config": {{
    "key1": {{
      "profile": "{}",
      "key": "key1",
      "value": "val1",
      "version": 1,
      "updated_at": 123456
    }}
  }}
}}
"#, profile);
    std::fs::write(vault.join(format!("{}.json", profile)), json_v2).unwrap();
    
    let tok_v2 = prof_dir.join("tok_v2");
    std::fs::create_dir_all(&tok_v2).unwrap();
    std::fs::write(tok_v2.join("access_token"), "dummy_token").unwrap();
    
    let dlq_test = prof_dir.join("dlq").join("test_topic");
    std::fs::create_dir_all(&dlq_test).unwrap();
    std::fs::write(dlq_test.join("msg1"), "dlq_msg").unwrap();
    
    let mut init_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "init", "--profile", profile, "--app-key", "dummykey",
        "--app-secret", "dummysecret", "--app-mode", "self-built",
        "--certificate", "dummy_cert", "--encrypt-key", "1234567890123456"
    ]);
    assert!(init_cmd.status().unwrap().success());
    
    assert!(vault.join(format!("{}.json.v2_bak", profile)).exists());
    
    let key1 = prof_dir.join("cfg").join("key1");
    assert!(key1.exists());
    assert!(std::fs::read_to_string(key1).unwrap().contains(""value":"val1""));
    
    assert!(prof_dir.join("tokens").exists());
    assert!(prof_dir.join("tokens").join("access_token").exists());
    
    assert!(prof_dir.join("dlq").join("msg1").exists());
}

#[tokio::test]
async fn test_store_migration_cli() {
    let profile = "case_89_mig";
    let dir = tempdir().unwrap();
    let home = dir.path().join(".cowen");
    std::fs::create_dir_all(&home).unwrap();
    
    std::fs::write(home.join("app.yaml"), r#"
storage:
  store: local
log:
  level: debug
"#).unwrap();
    
    let mut init_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "init", "--profile", profile, "--app-key", "dummykey",
        "--app-secret", "dummysecret", "--app-mode", "self-built",
        "--certificate", "dummy_cert", "--encrypt-key", "1234567890123456"
    ]);
    assert!(init_cmd.status().unwrap().success());
    
    let mut set_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    set_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "config", "set", "--profile", profile, "webhook_target", "http://localhost:9999"
    ]);
    assert!(set_cmd.status().unwrap().success());
    
    let mut daemon_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "daemon", "start", "--profile", profile
    ]);
    assert!(daemon_cmd.status().unwrap().success());
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    let mut mig_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    mig_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "store", "migrate", "--to", "sqlite://test_migrate_89.db", "--mode", "clone"
    ]);
    assert!(mig_cmd.status().unwrap().success());
    
    let app_yaml = std::fs::read_to_string(home.join("app.yaml")).unwrap();
    assert!(app_yaml.contains("store: innerdb") || app_yaml.contains("store: "innerdb""));
    assert!(app_yaml.contains("db_url: sqlite://test_migrate_89.db") || app_yaml.contains("db_url: "sqlite://test_migrate_89.db""));
    
    let mut stop_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    stop_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "daemon", "stop", "--profile", profile
    ]);
    stop_cmd.status().unwrap();
    
    let mut daemon_cmd2 = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd2.env("COWEN_HOME", &home).env("HOME", &home).args([
        "daemon", "start", "--profile", profile
    ]);
    assert!(daemon_cmd2.status().unwrap().success());
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    let get_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home).env("HOME", &home)
        .args(["config", "get", "webhook_target", "--profile", profile])
        .output().unwrap();
    let val = String::from_utf8_lossy(&get_cmd.stdout).trim().to_string();
    assert_eq!(val, ""http://localhost:9999"");
    
    let mut stop_cmd2 = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    stop_cmd2.env("COWEN_HOME", &home).env("HOME", &home).args([
        "daemon", "stop", "--profile", profile
    ]);
    stop_cmd2.status().unwrap();
}
