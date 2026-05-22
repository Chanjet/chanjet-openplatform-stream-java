use assert_cmd::Command;
use tempfile::tempdir;
use std::fs;
use serde_json::json;
use std::net::SocketAddr;
use axum::{
    routing::post,
    extract::State,
    Json,
    Router,
    http::{HeaderMap, StatusCode},
};
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};

struct MockState {
    pub last_generate_token_body: Option<serde_json::Value>,
    pub last_refresh_token_body: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, Debug)]
struct OAuth2Form {
    grant_type: String,
    client_id: String,
    refresh_token: Option<String>,
    code: Option<String>,
}

async fn handle_generate_token(
    State(state): State<Arc<Mutex<MockState>>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut s = state.lock().await;
    s.last_generate_token_body = Some(body);

    // Verify headers
    if headers.get("appKey").is_none() || headers.get("appSecret").is_none() {
        return (StatusCode::UNAUTHORIZED, Json(json!({"code": "50001", "message": "appKey/appSecret missing"})));
    }

    (StatusCode::OK, Json(json!({
        "result": true,
        "value": {
            "accessToken": "mock_at_sb_12345",
            "expiresIn": 7200
        }
    })))
}

async fn handle_oauth2_token(
    State(state): State<Arc<Mutex<MockState>>>,
    _headers: HeaderMap,
    axum::extract::Form(payload): axum::extract::Form<OAuth2Form>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut s = state.lock().await;
    s.last_refresh_token_body = Some(json!(payload));

    (StatusCode::OK, Json(json!({
        "access_token": "mock_at_oa2_new",
        "refresh_token": "mock_rt_oa2_new",
        "expires_in": 7200,
        "refresh_token_expires_in": 604800
    })))
}

async fn start_mock_platform() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let state = Arc::new(Mutex::new(MockState {
        last_generate_token_body: None,
        last_refresh_token_body: None,
    }));

    let app = Router::new()
        .route("/v1/common/auth/selfBuiltApp/generateToken", post(handle_generate_token))
        .route("/oauth2/token", post(handle_oauth2_token))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (addr, handle)
}

fn setup_auth_env(profile: &str, mode: &str, openapi_url: &str) -> (tempfile::TempDir, String) {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    let home = dir.path().join("home");
    fs::create_dir_all(&cowen_home).unwrap();
    fs::create_dir_all(&home).unwrap();

    let cowen_home_str = cowen_home.to_str().unwrap().to_string();
    
    // 2. Setup a dummy profile config (yaml)
    let config_path = cowen_home.join(format!("{}.yaml", profile));
    let config = json!({
        "app_key": "test_key",
        "app_mode": mode,
        "encrypt_key": "1234567890123456", // 16-byte dummy key to pass validation rules
        "webhook_target": "http://localhost:8080",
        "auto_start": false,
        "version": 1
    });
    fs::write(config_path, serde_yaml::to_string(&config).unwrap()).unwrap();

    let app_config_path = cowen_home.join("app.yaml");
    let app_config = json!({
        "openapi_url": openapi_url,
        "stream_url": openapi_url,
        "telemetry_enabled": false,
        "log": {
            "level": "info",
            "rotation": "daily",
            "max_size_mb": 100,
            "max_files": 7
        }
    });
    fs::write(app_config_path, serde_yaml::to_string(&app_config).unwrap()).unwrap();

    (dir, cowen_home_str)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_auth_login_self_built() {
    let (addr, _server_handle) = start_mock_platform().await;
    let openapi_url = format!("http://{}", addr);
    let (dir, home) = setup_auth_env("sb_profile", "self-built", &openapi_url);
    
    let app_cfg = cowen_common::config::AppConfig { openapi_url: openapi_url.clone(), stream_url: openapi_url.clone(), ..Default::default() };
    let vault = cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint").await.unwrap();
    
    vault.set_config("sb_profile", "app_key", "test_key").await.unwrap();
    vault.set_secret("sb_profile", "app_secret", "test_secret").await.unwrap();
    vault.set_secret("sb_profile", "encrypt_key", "test_encrypt_key").await.unwrap();
    
    vault.save_app_ticket("test_key", cowen_common::models::Ticket {
        value: "mock_ticket_abc".to_string(),
        created_at: chrono::Utc::now(),
    }).await.unwrap();

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home); 
    cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd.env("COWEN_SKIP_COMPLETION_INSTALL", "true");
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "true");
    cmd.arg("--profile").arg("sb_profile");
    cmd.arg("auth").arg("login").arg("--force");
    
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("Token forcefully refreshed"));
    
    let token = vault.get_app_access_token("test_key").await.unwrap();
    assert_eq!(token.value, "mock_at_sb_12345");

    let _ = dir;
}

async fn handle_generate_token_error(
    State(state): State<Arc<Mutex<MockState>>>,
    _headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut s = state.lock().await;
    s.last_generate_token_body = Some(body);

    (StatusCode::OK, Json(json!({
        "result": false,
        "code": "401",
        "message": {
            "error_code": "ticket_expired",
            "reason": "The provided app ticket is no longer valid"
        }
    })))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_auth_login_complex_error_serialization() {
    let state = Arc::new(Mutex::new(MockState {
        last_generate_token_body: None,
        last_refresh_token_body: None,
    }));

    let app = Router::new()
        .route("/v1/common/auth/selfBuiltApp/generateToken", post(handle_generate_token_error))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let _handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let openapi_url = format!("http://{}", addr);
    let (dir, home) = setup_auth_env("error_profile", "self-built", &openapi_url);
    
    let app_cfg = cowen_common::config::AppConfig { openapi_url: openapi_url.clone(), stream_url: openapi_url.clone(), ..Default::default() };
    let vault = cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint").await.unwrap();
    
    vault.set_config("error_profile", "app_key", "test_key").await.unwrap();
    vault.set_secret("error_profile", "app_secret", "test_secret").await.unwrap();
    vault.set_secret("error_profile", "encrypt_key", "test_encrypt_key").await.unwrap();
    vault.save_app_ticket("test_key", cowen_common::models::Ticket {
        value: "test_ticket".to_string(),
        created_at: chrono::Utc::now(),
    }).await.unwrap();

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd.env("COWEN_SKIP_COMPLETION_INSTALL", "true");
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "true");
    cmd.arg("--profile").arg("error_profile");
    cmd.arg("auth").arg("login");
    
    // Should NOT fail with serialization error, but with the reported platform error
    cmd.assert()
        .failure()
        .stderr(predicates::str::contains("Platform error:"));
    
    // Verify it doesn't contain "Serialization error"
    let output = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("Serialization error"), "Should not have serialization error, got: {}", stderr);
    assert!(stderr.contains("ticket_expired"), "Should contain the complex error content");

    let _ = dir;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_auth_logout_login_sequence() {
    let (addr, _server_handle) = start_mock_platform().await;
    let openapi_url = format!("http://{}", addr);
    let (dir, home) = setup_auth_env("seq_profile", "self-built", &openapi_url);
    
    let app_cfg = cowen_common::config::AppConfig { openapi_url: openapi_url.clone(), stream_url: openapi_url.clone(), ..Default::default() };
    let vault = cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint").await.unwrap();
    
    vault.set_config("seq_profile", "app_key", "test_key").await.unwrap();
    vault.set_secret("seq_profile", "app_secret", "test_secret").await.unwrap();
    vault.set_secret("seq_profile", "encrypt_key", "test_encrypt_key").await.unwrap();
    vault.save_app_ticket("test_key", cowen_common::models::Ticket {
        value: "test_ticket".to_string(),
        created_at: chrono::Utc::now(),
    }).await.unwrap();

    // 1. Logout first (Ensure clean state)
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd.env("COWEN_SKIP_COMPLETION_INSTALL", "true");
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "true");
    cmd.arg("--profile").arg("seq_profile");
    cmd.arg("auth").arg("logout");
    cmd.assert().success();

    // Re-seed ticket because logout cleared it (in a real scenario, the platform would push it back or daemon would be running)
    vault.save_app_ticket("test_key", cowen_common::models::Ticket {
        value: "test_ticket".to_string(),
        created_at: chrono::Utc::now(),
    }).await.unwrap();

    // 2. Login (Should trigger network refresh because logout cleared the token)
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd.env("COWEN_SKIP_COMPLETION_INSTALL", "true");
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "true");
    cmd.arg("--profile").arg("seq_profile");
    cmd.arg("auth").arg("login");
    
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("Token is active and ready"));
    
    let token = vault.get_app_access_token("test_key").await.unwrap();
    assert_eq!(token.value, "mock_at_sb_12345");

    let _ = dir;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_auth_login_oauth2() {
    let (addr, _server_handle) = start_mock_platform().await;
    let openapi_url = format!("http://{}", addr);
    let (dir, home) = setup_auth_env("oa2_profile", "oauth2", &openapi_url);
    
    let app_cfg = cowen_common::config::AppConfig { openapi_url: openapi_url.clone(), stream_url: openapi_url.clone(), ..Default::default() };
    let vault = cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint").await.unwrap();
    
    vault.set_config("oa2_profile", "app_key", "test_key").await.unwrap();
    vault.set_secret("oa2_profile", "app_secret", "test_secret").await.unwrap();

    let rt = cowen_common::models::Token {
        value: "old_rt".to_string(),
        expires_at: chrono::Utc::now() + chrono::Duration::days(1),
        created_at: chrono::Utc::now() - chrono::Duration::hours(2),
    };
    vault.save_refresh_token("oa2_profile", rt).await.unwrap();

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd.env("COWEN_SKIP_COMPLETION_INSTALL", "true");
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "true");
    cmd.arg("--profile").arg("oa2_profile");
    cmd.arg("auth").arg("login");
    
    cmd.assert()
        .success()
        .stdout(predicates::str::contains("OAuth2 Token Pair has been rotated"));
    
    let at = vault.get_access_token("oa2_profile").await.unwrap();
    let rt = vault.get_refresh_token("oa2_profile").await.unwrap();
    assert_eq!(at.value, "mock_at_oa2_new");
    assert_eq!(rt.value, "mock_rt_oa2_new");

    // Legacy JSON blob should be purged
    assert!(vault.get_config("oa2_profile", "oauth2_token_pair").await.is_err());

    let _ = dir;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_auth_logout() {
    let (addr, _server_handle) = start_mock_platform().await;
    let openapi_url = format!("http://{}", addr);
    let (dir, home) = setup_auth_env("logout_profile", "self-built", &openapi_url);
    
    let app_cfg = cowen_common::config::AppConfig { openapi_url: openapi_url.clone(), stream_url: openapi_url.clone(), ..Default::default() };
    let vault = cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint").await.unwrap();
    
    // 1. Setup active token and ticket
    vault.set_config("logout_profile", "app_key", "test_key").await.unwrap();
    vault.set_secret("logout_profile", "encrypt_key", "1234567890123456").await.unwrap();
    vault.save_app_access_token("test_key", cowen_common::models::Token {
        value: "active_token".to_string(),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        created_at: chrono::Utc::now(),
    }).await.unwrap();
    vault.save_app_ticket("test_key", cowen_common::models::Ticket {
        value: "active_ticket".to_string(),
        created_at: chrono::Utc::now(),
    }).await.unwrap();

    // 2. Verify they exist
    assert!(vault.get_app_access_token("test_key").await.is_ok());
    assert!(vault.get_app_ticket("test_key").await.is_ok());

    // 3. Perform logout
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd.env("COWEN_SKIP_COMPLETION_INSTALL", "true");
    cmd.env("COWEN_SKIP_DAEMON_RECOVERY", "true");
    cmd.arg("--profile").arg("logout_profile");
    cmd.arg("auth").arg("logout");
    
    cmd.assert().success();

    // 4. Verify they are cleared
    assert!(vault.get_app_access_token("test_key").await.is_err());
    assert!(vault.get_app_ticket("test_key").await.is_err());

    let _ = dir;
}
