use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

fn setup_test_env(
    ai_enabled: bool,
) -> (
    tempfile::TempDir,
    String,
    crate::e2e::rust::common::DaemonKiller,
) {
    std::env::set_var("COWEN_HTTP_TIMEOUT", "2");
    let dir = tempdir().unwrap();
    let profile = "test_api";
    let cowen_home = dir.path().to_str().unwrap().to_string();

    // 1. Create app directory
    fs::create_dir_all(dir.path()).unwrap();

    // 2. Setup a dummy profile config (yaml)
    let config_path = dir.path().join(format!("{}.yaml", profile));
    let config = json!({
        "app_key": "test_key",
        "openapi_url": "http://localhost:8080",
        "stream_url": "http://localhost:8081",
        "webhook_target": "http://localhost:8082",
        "app_mode": "oauth2",
        "ai_enabled": ai_enabled,
        "version": 1,
        "log": {
            "level": "info",
            "rotation": "daily",
            "max_size_mb": 100,
            "max_files": 7
        }
    });
    fs::write(config_path, serde_yaml::to_string(&config).unwrap()).unwrap();

    // 3. Setup mock token so API calls don't require network refresh
    let vault_dir = dir.path().join("test_api").join("tok_v2");
    fs::create_dir_all(&vault_dir).unwrap();
    let token_path = vault_dir.join("access");
    let token = cowen_common::models::Token {
        value: "mock_token".to_string(),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        created_at: chrono::Utc::now(),
    };
    // Save to the file directly as the Store would
    fs::write(token_path, serde_json::to_string(&token).unwrap()).unwrap();

    // 4. Setup mock OpenAPI spec
    let spec_path = dir.path().join(format!("{}_openapi.yaml", profile));
    let spec = json!({
        "openapi": "3.0.0",
        "info": { "title": "Test API", "version": "1.0.0" },
        "paths": {
            "/v1/users": {
                "get": {
                    "summary": "List users",
                    "description": "Returns a list of users",
                    "parameters": [
                        { "name": "page", "in": "query", "required": false, "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": { "description": "OK" }
                    }
                },
                "post": {
                    "summary": "Create user",
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "name": { "type": "string", "example": "John Doe" }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "201": { "description": "Created" }
                    }
                }
            },
            "/v1/users/{id}": {
                "get": {
                    "summary": "Get user",
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }
                    ],
                    "responses": {
                        "200": {
                            "description": "OK",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "id": { "type": "string", "description": "User ID" },
                                            "name": { "type": "string", "description": "User Name" }
                                        }
                                    },
                                    "examples": {
                                        "success": {
                                            "summary": "success example",
                                            "value": {
                                                "id": "123",
                                                "name": "Alice"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/v2/items": {
                "get": { "summary": "List items", "responses": { "200": { "description": "OK" } } }
            },
            "/v2/items/{id}": {
                "get": { "summary": "Get item", "responses": { "200": { "description": "OK" } } }
            }
        }
    });
    fs::write(spec_path, serde_yaml::to_string(&spec).unwrap()).unwrap();

    (
        dir,
        cowen_home.clone(),
        crate::e2e::rust::common::DaemonKiller { home: cowen_home },
    )
}

#[test]
fn test_api_list() {
    let (dir, home, _killer) = setup_test_env(false);
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("api").arg("list");

    cmd.assert()
        .success()
        .stdout(
            predicates::str::contains("GET")
                .and(predicates::str::contains("/v1/users"))
                .and(predicates::str::contains("List users")),
        )
        .stdout(
            predicates::str::contains("POST")
                .and(predicates::str::contains("/v1/users"))
                .and(predicates::str::contains("Create user")),
        )
        .stdout(
            predicates::str::contains("GET")
                .and(predicates::str::contains("/v1/users/{id}"))
                .and(predicates::str::contains("Get user")),
        )
        .stdout(
            predicates::str::contains("GET")
                .and(predicates::str::contains("/v2/items"))
                .and(predicates::str::contains("List items")),
        )
        .stdout(
            predicates::str::contains("GET")
                .and(predicates::str::contains("/v2/items/{id}"))
                .and(predicates::str::contains("Get item")),
        );

    let _ = dir;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_openapi_whitelist() {
    let (addr, _server_handle) = crate::e2e::rust::mock_server::spawn_mock_server().await;
    let (dir, home, _killer) = setup_test_env(false);

    // Override the profile to use the mock server for actual requests
    let profile = "test_api";
    let config_path = std::path::Path::new(&home).join(format!("{}.yaml", profile));
    let config = json!({
        "app_key": "test_key",
        "openapi_url": format!("http://127.0.0.1:{}", addr),
        "stream_url": format!("http://127.0.0.1:{}", addr),
        "webhook_target": "http://localhost:8082",
        "app_mode": "oauth2",
        "ai_enabled": false,
        "version": 1,
        "log": {
            "level": "info"
        }
    });
    fs::write(config_path, serde_yaml::to_string(&config).unwrap()).unwrap();

    // 1. Test Whitelisted Path (should be forwarded)
    let mut cmd1 = Command::cargo_bin("cowen").unwrap();
    cmd1.env("COWEN_HOME", &home);
    cmd1.arg("--profile").arg("test_api");
    cmd1.arg("api")
        .arg("POST")
        .arg("/v1/users")
        .arg("--data")
        .arg("{\"name\":\"Alice\"}");

    // Note: The mock server does not actually have /v1/users, but it will be forwarded
    // by cowen-cli because it's in the OpenAPI spec. The mock server will return 404,
    // but what we care about is that it IS NOT blocked locally by cowen-cli.
    let output1 = cmd1.output().unwrap();
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    let stderr1 = String::from_utf8_lossy(&output1.stderr);

    // Check it wasn't blocked locally
    assert!(!stderr1.to_lowercase().contains("not in whitelist"));
    assert!(!stdout1.to_lowercase().contains("not in whitelist"));

    // 2. Test Non-Whitelisted Path (should be blocked)
    let mut cmd2 = Command::cargo_bin("cowen").unwrap();
    cmd2.env("COWEN_HOME", &home);
    cmd2.arg("--profile").arg("test_api");
    cmd2.arg("api")
        .arg("POST")
        .arg("/v1/evil/hacker/path")
        .arg("--data")
        .arg("{\"cmd\":\"rm -rf\"}");

    let output2 = cmd2.output().unwrap();
    let stderr2 = String::from_utf8_lossy(&output2.stderr);
    let stdout2 = String::from_utf8_lossy(&output2.stdout);

    // Check it was blocked
    let blocked = stderr2.to_lowercase().contains("not in whitelist")
        || stdout2.to_lowercase().contains("not in whitelist")
        || stderr2.to_lowercase().contains("not found in openapi spec")
        || stdout2.to_lowercase().contains("not found in openapi spec")
        || stderr2.to_lowercase().contains("forbidden")
        || stdout2.to_lowercase().contains("forbidden");
    assert!(
        blocked,
        "Expected request to be blocked locally. Output: {}\n{}",
        stdout2, stderr2
    );

    let _ = dir;
}

#[test]
fn test_api_list_pagination() {
    let (dir, home, _killer) = setup_test_env(false);

    // Page 1: GET /v1/users, POST /v1/users
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("api")
        .arg("list")
        .arg("--page-size")
        .arg("2")
        .arg("--page")
        .arg("1");

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8_lossy(&output);

    assert!(stdout.contains("/v1/users"));
    assert!(stdout.contains("POST"));
    assert!(!stdout.contains("/v1/users/{id}"));

    // Page 2: GET /v1/users/{id}, GET /v2/items
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("api")
        .arg("list")
        .arg("--page-size")
        .arg("2")
        .arg("--page")
        .arg("2");

    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8_lossy(&output);

    assert!(stdout.contains("/v1/users/{id}"));
    assert!(stdout.contains("/v2/items"));
    assert!(!stdout.contains("/v2/items/{id}"));

    let _ = dir;
}

#[test]
fn test_api_list_search() {
    let (dir, home, _killer) = setup_test_env(false); // Normal search
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("api").arg("list").arg("--search").arg("items");

    let output = cmd.assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    assert!(stdout.contains("/v2/items"));
    assert!(stdout.contains("/v2/items/{id}"));
    assert!(!stdout.contains("/v1/users"));

    let _ = dir;
}

#[test]
fn test_api_spec_details() {
    let (dir, home, _killer) = setup_test_env(false);

    // Test GET with path params
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("api").arg("spec").arg("GET").arg("/v1/users/{id}");

    cmd.assert()
        .success()
        .stdout(predicates::str::contains("📌 Summary:     Get user"))
        .stdout(predicates::str::contains("id").and(predicates::str::contains("(path    )")))
        .stdout(predicates::str::contains("📤 Responses:"))
        .stdout(predicates::str::contains("200 (OK):"))
        .stdout(predicates::str::contains("id: <string> - User ID"))
        .stdout(predicates::str::contains("name: <string> - User Name"))
        .stdout(predicates::str::contains("Example Response:"))
        .stdout(predicates::str::contains("\"id\": \"123\""))
        .stdout(predicates::str::contains("\"name\": \"Alice\""))
        .stdout(predicates::str::contains("💡 Usage Example:"))
        .stdout(predicates::str::contains(
            "cowen api GET \"/v1/users/<id>\"",
        ));

    // Test POST with request body
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("api").arg("spec").arg("POST").arg("/v1/users");

    cmd.assert()
        .success()
        .stdout(predicates::str::contains("📌 Summary:     Create user"))
        .stdout(predicates::str::contains("📥 Request Body:"))
        .stdout(predicates::str::contains("name").and(predicates::str::contains("<string>")))
        .stdout(predicates::str::contains("💡 Usage Example:"))
        .stdout(predicates::str::contains(
            "cowen api POST \"/v1/users\" -d '{\"name\":\"John Doe\"}'",
        ));

    let _ = dir;
}

#[test]
fn test_api_call_force_bypass() {
    let (dir, home, _killer) = setup_test_env(false);

    // Without --force, calling a path not in spec should be rejected
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("api").arg("GET").arg("/v1/ghost");

    cmd.assert()
        .failure()
        .stderr(predicates::str::contains("Validation error"));

    // With --force, it should bypass the check and fail later at network layer
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("api").arg("--force").arg("GET").arg("/v1/ghost");

    let output = cmd.assert().failure().get_output().stderr.clone();
    let stderr = String::from_utf8_lossy(&output);
    assert!(!stderr.contains("Validation error"));

    let _ = dir;
}

#[test]
fn test_api_error_json_format() {
    let (dir, home, _killer) = setup_test_env(false);

    // Global format JSON should wrap the error
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("-o").arg("json");
    cmd.arg("api").arg("GET").arg("/v1/ghost");

    // Fails with CLI Rejected, but output should be a JSON in stdout
    let output = cmd.assert().failure().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("\"status\": \"failed\""));
    assert!(stdout.contains("\"error\":"));
    assert!(stdout.contains("Validation error"));

    let _ = dir;
}

#[test]
fn test_api_call_ssrf_block() {
    let (dir, home, _killer) = setup_test_env(false);

    // Attempting to call an absolute external URL should be blocked even with --force
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("api")
        .arg("--force")
        .arg("GET")
        .arg("http://evil.com/v1/users");

    let output = cmd.assert().failure().get_output().stderr.clone();
    let stderr = String::from_utf8_lossy(&output);
    println!("STDERR WAS: {}", stderr);
    assert!(
        stderr.contains("Security Block")
            || stderr.contains("Absolute external URLs are not allowed")
    );

    let _ = dir;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_api_list_self_built_refresh_timeout() {
    let (dir, home, _killer) = setup_test_env(false);

    // Create self-built profile config without seeding token
    let profile = "test_sb_timeout";
    let config_path = dir.path().join(format!("{}.yaml", profile));
    let config = json!({
        "app_key": "test_key",
        "app_mode": "self-built",
        "webhook_target": "http://localhost:59999",
        "auto_start": false,
        "version": 1
    });
    fs::write(config_path, serde_yaml::to_string(&config).unwrap()).unwrap();

    let app_cfg_path = dir.path().join("app.yaml");
    let app_cfg_json = json!({
        "openapi_url": "http://localhost:59999", // dead port
        "stream_url": "http://localhost:59999",
        "webhook_target": "http://localhost:59999"
    });
    fs::write(app_cfg_path, serde_yaml::to_string(&app_cfg_json).unwrap()).unwrap();

    // Seed vault
    let app_cfg = cowen_common::config::AppConfig {
        openapi_url: "http://localhost:59999".to_string(),
        stream_url: "http://localhost:59999".to_string(),
        ..Default::default()
    };
    let vault =
        cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint")
            .await
            .unwrap();
    vault
        .set_config(profile, "app_key", "test_key")
        .await
        .unwrap();
    vault
        .set_secret(profile, "app_secret", "test_secret")
        .await
        .unwrap();
    vault
        .set_secret(profile, "encrypt_key", "1234567890123456")
        .await
        .unwrap();

    // Start daemon
    let mut cmd_start = Command::cargo_bin("cowen").unwrap();
    cmd_start.env("COWEN_HOME", &home);
    cmd_start.env("HOME", &home);
    cmd_start.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_start.env("COWEN_SKIP_DAEMON_RECOVERY", "true");
    cmd_start.env("COWEN_HTTP_TIMEOUT", "1");

    let daemon_bin = assert_cmd::cargo::cargo_bin("cowen-daemon");
    cmd_start.env("COWEN_DAEMON_PATH", daemon_bin.to_str().unwrap());

    cmd_start
        .arg("--profile")
        .arg(profile)
        .arg("daemon")
        .arg("start");
    let _ = cmd_start.output();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Now run api list
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd.env("COWEN_DAEMON_PATH", daemon_bin.to_str().unwrap());
    cmd.env("COWEN_HTTP_TIMEOUT", "1");
    cmd.arg("--profile").arg(profile);
    cmd.arg("api").arg("list");

    // We expect it to exit with code 0 (CLI prints to stderr but doesn't exit 1 for API failures sometimes)
    // or exit with code 1. Let's just get output.
    let output = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Stop daemon
    let mut cmd_stop = Command::cargo_bin("cowen").unwrap();
    cmd_stop.env("COWEN_HOME", &home);
    cmd_stop.env("HOME", &home);
    cmd_stop.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_stop
        .arg("--profile")
        .arg(profile)
        .arg("daemon")
        .arg("stop");
    let _ = cmd_stop.output();
    // Should fail with Missing appTicket instead of Timeout expired
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should fail with Missing appTicket instead of Timeout expired
    assert!(!stderr.contains("Timeout expired") && !stderr.contains("transport error"), "CLI timed out instead of getting the proper error from daemon. Output: stderr={}, stdout={}", stderr, stdout);
    assert!(
        stderr.contains("Missing appTicket"),
        "CLI did not receive the expected Missing appTicket error. Output: stderr={}, stdout={}",
        stderr,
        stdout
    );

    let _ = dir;
}

#[test]
fn test_api_list_and_spec_formats() {
    let (dir, home, _killer) = setup_test_env(false);

    let profile = "test_api";

    // test api list json format
    let mut cmd_json = Command::cargo_bin("cowen").unwrap();
    cmd_json.env("COWEN_HOME", &home);
    cmd_json.arg("--profile").arg(profile);
    cmd_json.arg("api").arg("list").arg("--format").arg("json");

    let output_json = cmd_json.assert().success().get_output().clone();
    let stdout_json = String::from_utf8_lossy(&output_json.stdout);

    // Validate JSON output
    assert!(
        stdout_json.contains("\"/v1/users\""),
        "Output: {}",
        stdout_json
    );
    assert!(stdout_json.contains("\"List users\""));

    // test api list yaml format
    let mut cmd_yaml = Command::cargo_bin("cowen").unwrap();
    cmd_yaml.env("COWEN_HOME", &home);
    cmd_yaml.arg("--profile").arg(profile);
    cmd_yaml.arg("api").arg("list").arg("--format").arg("yaml");

    let output_yaml = cmd_yaml.assert().success().get_output().clone();
    let stdout_yaml = String::from_utf8_lossy(&output_yaml.stdout);

    // Validate YAML output
    assert!(stdout_yaml.contains("path: /v1/users"));
    assert!(stdout_yaml.contains("summary: List users"));

    // test api spec raw format
    let mut cmd_spec_raw = Command::cargo_bin("cowen").unwrap();
    cmd_spec_raw.env("COWEN_HOME", &home);
    cmd_spec_raw.arg("--profile").arg(profile);
    cmd_spec_raw
        .arg("api")
        .arg("spec")
        .arg("GET")
        .arg("/v1/users")
        .arg("--raw");

    let output_spec_raw = cmd_spec_raw.assert().success().get_output().clone();
    let stdout_spec_raw = String::from_utf8_lossy(&output_spec_raw.stdout);

    // Validate raw spec output (should just be raw JSON/YAML part of the openapi spec)
    assert!(stdout_spec_raw.contains("summary"));
    assert!(stdout_spec_raw.contains("List users"));

    let _ = dir;
}
