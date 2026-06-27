use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread")]
async fn test_store_migration_v2_to_v3() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    let home = dir.path().join("home");
    fs::create_dir_all(&cowen_home).unwrap();
    fs::create_dir_all(&home).unwrap();

    let cowen_home_str = cowen_home.to_str().unwrap().to_string();
    let home_str = home.to_str().unwrap().to_string();

    let app_config_path = cowen_home.join("app.yaml");
    let app_config = serde_json::json!({
        "storage": {
            "store": "local"
        },
        "log": {
            "level": "debug"
        }
    });
    fs::write(app_config_path, serde_yaml::to_string(&app_config).unwrap()).unwrap();

    let profile = "case_88_mig";
    let vault_dir = cowen_home.join("vault").join(profile);
    fs::create_dir_all(&vault_dir).unwrap();

    let v2_json = serde_json::json!({
        "config": {
            "key1": {
                "profile": profile,
                "key": "key1",
                "value": "val1",
                "version": 1,
                "updated_at": 123456
            }
        }
    });
    fs::write(
        cowen_home.join("vault").join(format!("{}.json", profile)),
        serde_json::to_string(&v2_json).unwrap(),
    )
    .unwrap();

    let tok_v2_dir = vault_dir.join("tok_v2");
    fs::create_dir_all(&tok_v2_dir).unwrap();
    fs::write(tok_v2_dir.join("access_token"), "dummy_token").unwrap();

    let dlq_topic_dir = vault_dir.join("dlq").join("test_topic");
    fs::create_dir_all(&dlq_topic_dir).unwrap();
    fs::write(dlq_topic_dir.join("msg1"), "dlq_msg").unwrap();

    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &cowen_home_str);
    init_cmd.env("HOME", &home_str);
    init_cmd
        .arg("init")
        .arg("--profile")
        .arg(profile)
        .arg("--app-key")
        .arg("dummykey")
        .arg("--app-secret")
        .arg("dummysecret")
        .arg("--app-mode")
        .arg("self-built")
        .arg("--certificate")
        .arg("dummy_cert")
        .arg("--encrypt-key")
        .arg("1234567890123456");

    init_cmd.assert().success();

    assert!(cowen_home
        .join("vault")
        .join(format!("{}.json.v2_bak", profile))
        .exists());
    assert!(vault_dir.join("cfg").join("key1").exists());
    let cfg_val = fs::read_to_string(vault_dir.join("cfg").join("key1")).unwrap();
    assert!(cfg_val.contains(r#""value":"val1""#));

    assert!(vault_dir.join("tokens").exists());
    assert!(vault_dir.join("tokens").join("access_token").exists());

    assert!(vault_dir.join("dlq").join("msg1").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_store_migration_cli() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    let home = dir.path().join("home");
    fs::create_dir_all(&cowen_home).unwrap();
    fs::create_dir_all(&home).unwrap();

    let cowen_home_str = cowen_home.to_str().unwrap().to_string();
    let home_str = home.to_str().unwrap().to_string();

    let app_config_path = cowen_home.join("app.yaml");
    let app_config = serde_json::json!({
        "storage": {
            "store": "local"
        },
        "log": {
            "level": "debug"
        }
    });
    fs::write(app_config_path, serde_yaml::to_string(&app_config).unwrap()).unwrap();

    let profile = "case_89_mig";

    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &cowen_home_str);
    init_cmd.env("HOME", &home_str);
    init_cmd
        .arg("init")
        .arg("--profile")
        .arg(profile)
        .arg("--app-key")
        .arg("dummykey")
        .arg("--app-secret")
        .arg("dummysecret")
        .arg("--app-mode")
        .arg("self-built")
        .arg("--certificate")
        .arg("dummy_cert")
        .arg("--encrypt-key")
        .arg("1234567890123456");
    init_cmd.assert().success();

    let mut cfg_cmd = Command::cargo_bin("cowen").unwrap();
    cfg_cmd.env("COWEN_HOME", &cowen_home_str);
    cfg_cmd.env("HOME", &home_str);
    cfg_cmd
        .arg("config")
        .arg("set")
        .arg("--profile")
        .arg(profile)
        .arg("webhook_target")
        .arg("http://localhost:9999");
    cfg_cmd.assert().success();

    let target_db = "sqlite://test_migrate_89.db";

    let mut migrate_cmd = Command::cargo_bin("cowen").unwrap();
    migrate_cmd.env("COWEN_HOME", &cowen_home_str);
    migrate_cmd.env("HOME", &home_str);
    migrate_cmd
        .arg("store")
        .arg("migrate")
        .arg("--to")
        .arg(target_db)
        .arg("--mode")
        .arg("clone");
    migrate_cmd.assert().success();

    let updated_app_yaml = fs::read_to_string(cowen_home.join("app.yaml")).unwrap();
    assert!(updated_app_yaml.contains("store: innerdb"));
    assert!(updated_app_yaml.contains("db_url: sqlite://test_migrate_89.db"));

    // We can't easily start the daemon here, but we can use CLI to read back the config
    // The CLI should load the new innerdb store based on app.yaml
    let mut cfg_get_cmd = Command::cargo_bin("cowen").unwrap();
    cfg_get_cmd.env("COWEN_HOME", &cowen_home_str);
    cfg_get_cmd.env("HOME", &home_str);
    cfg_get_cmd
        .arg("config")
        .arg("get")
        .arg("webhook_target")
        .arg("--profile")
        .arg(profile);

    let get_out = String::from_utf8_lossy(&cfg_get_cmd.output().unwrap().stdout).to_string();
    assert!(
        get_out.contains("http://localhost:9999"),
        "Migrated value mismatch: {}",
        get_out
    );
}
