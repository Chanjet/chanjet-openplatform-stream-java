use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use tempfile::tempdir;
use std::fs;
use serde_json::json;

fn setup_test_env(ai_enabled: bool) -> (tempfile::TempDir, String) {
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
    
    // 3. Setup mock OpenAPI spec
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
                        "200": { "description": "OK" }
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
    
    (dir, cowen_home)
}

#[test]
fn test_api_list() {
    let (dir, home) = setup_test_env(false);
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("api").arg("list");
    
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("GET").and(predicates::str::contains("/v1/users")).and(predicates::str::contains("List users")))
        .stdout(predicates::str::contains("POST").and(predicates::str::contains("/v1/users")).and(predicates::str::contains("Create user")))
        .stdout(predicates::str::contains("GET").and(predicates::str::contains("/v1/users/{id}")).and(predicates::str::contains("Get user")))
        .stdout(predicates::str::contains("GET").and(predicates::str::contains("/v2/items")).and(predicates::str::contains("List items")))
        .stdout(predicates::str::contains("GET").and(predicates::str::contains("/v2/items/{id}")).and(predicates::str::contains("Get item")));
    
    let _ = dir;
}

#[test]
fn test_api_list_pagination() {
    let (dir, home) = setup_test_env(false);
    
    // Page 1: GET /v1/users, POST /v1/users
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("api").arg("list").arg("--page-size").arg("2").arg("--page").arg("1");
    
    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8_lossy(&output);
    
    assert!(stdout.contains("/v1/users"));
    assert!(stdout.contains("POST"));
    assert!(!stdout.contains("/v1/users/{id}"));
    
    // Page 2: GET /v1/users/{id}, GET /v2/items
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("api").arg("list").arg("--page-size").arg("2").arg("--page").arg("2");
    
    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8_lossy(&output);
    
    assert!(stdout.contains("/v1/users/{id}"));
    assert!(stdout.contains("/v2/items"));
    assert!(!stdout.contains("/v2/items/{id}"));
    
    let _ = dir;
}

#[test]
fn test_api_list_search() {
    let (dir, home) = setup_test_env(false); // Normal search
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("api").arg("list").arg("--search").arg("items");
    
    let output = cmd.assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8_lossy(&output);

    assert!(stdout.contains("/v2/items"));
    assert!(stdout.contains("/v2/items/{id}"));
    assert!(!stdout.contains("/v1/users"));
    
    let _ = dir;
}

#[test]
fn test_api_spec_details() {
    let (dir, home) = setup_test_env(false);
    
    // Test GET with path params
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.arg("--profile").arg("test_api");
    cmd.arg("api").arg("spec").arg("GET").arg("/v1/users/{id}");
    
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("📌 Summary:     Get user"))
        .stdout(predicates::str::contains("id").and(predicates::str::contains("(path    )")))
        .stdout(predicates::str::contains("💡 Usage Example:"))
        .stdout(predicates::str::contains("cowen api GET \"/v1/users/<id>\""));
    
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
        .stdout(predicates::str::contains("cowen api POST \"/v1/users\" -d '{\"name\":\"John Doe\"}'"));
    
    let _ = dir;
}
