use assert_cmd::Command;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::Mutex;

struct MockState {
    pub last_generate_token_body: Option<serde_json::Value>,
    pub last_refresh_token_body: Option<serde_json::Value>,
    pub refresh_count: u32,
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
    s.last_generate_token_body = Some(body.clone());

    // Verify headers
    if headers.get("appKey").is_none() || headers.get("appSecret").is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code": "50001", "message": "appKey/appSecret missing"})),
        );
    }

    // 🚀 E2E Enhancement: Strictly validate appTicket presence in JSON body to mirror real platform behavior
    if body.get("appTicket").is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "result": false,
                "error": {
                    "code": "2002",
                    "msg": "Required request parameter 'appTicket' for method parameter type String is not present",
                    "hint": null
                },
                "value": null,
                "traceId": "mock-trace-id"
            })),
        );
    }

    (
        StatusCode::OK,
        Json(json!({
            "result": true,
            "value": {
                "accessToken": "mock_at_sb_12345",
                "expiresIn": 7200
            }
        })),
    )
}

async fn handle_oauth2_token(
    State(state): State<Arc<Mutex<MockState>>>,
    _headers: HeaderMap,
    axum::extract::Form(payload): axum::extract::Form<OAuth2Form>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut s = state.lock().await;
    s.last_refresh_token_body = Some(json!(payload));

    (
        StatusCode::OK,
        Json(json!({
            "access_token": "mock_at_oa2_new",
            "refresh_token": "mock_rt_oa2_new",
            "expires_in": 7200,
            "refresh_token_expires_in": 604800
        })),
    )
}

async fn start_mock_platform() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let state = Arc::new(Mutex::new(MockState {
        last_generate_token_body: None,
        last_refresh_token_body: None,
        refresh_count: 0,
    }));

    let app = Router::new()
        .route(
            "/v1/common/auth/selfBuiltApp/generateToken",
            post(handle_generate_token),
        )
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

    let app_cfg = cowen_common::config::AppConfig {
        openapi_url: openapi_url.clone(),
        stream_url: openapi_url.clone(),
        ..Default::default()
    };
    let vault =
        cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint")
            .await
            .unwrap();

    vault
        .set_config("sb_profile", "app_key", "test_key")
        .await
        .unwrap();
    vault
        .set_secret("sb_profile", "app_secret", "test_secret")
        .await
        .unwrap();
    vault
        .set_secret("sb_profile", "encrypt_key", "test_encrypt_key")
        .await
        .unwrap();

    vault
        .save_app_ticket(
            "test_key",
            cowen_common::models::Ticket {
                value: "mock_ticket_abc".to_string(),
                created_at: chrono::Utc::now(),
            },
        )
        .await
        .unwrap();

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd.env("COWEN_SKIP_COMPLETION_INSTALL", "true");
    cmd.env("COWEN_SKIP_BROWSER", "true");
    cmd.arg("--profile").arg("sb_profile");
    cmd.arg("auth").arg("login").arg("--force");

    cmd.assert()
        .success()
        .stdout(predicates::str::contains("Token is active and ready"));

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

    (
        StatusCode::OK,
        Json(json!({
            "result": false,
            "code": "401",
            "message": {
                "error_code": "ticket_expired",
                "reason": "The provided app ticket is no longer valid"
            }
        })),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn test_auth_login_complex_error_serialization() {
    let state = Arc::new(Mutex::new(MockState {
        last_generate_token_body: None,
        last_refresh_token_body: None,
        refresh_count: 0,
    }));

    let app = Router::new()
        .route(
            "/v1/common/auth/selfBuiltApp/generateToken",
            post(handle_generate_token_error),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let _handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let openapi_url = format!("http://{}", addr);
    let (dir, home) = setup_auth_env("error_profile", "self-built", &openapi_url);

    let app_cfg = cowen_common::config::AppConfig {
        openapi_url: openapi_url.clone(),
        stream_url: openapi_url.clone(),
        ..Default::default()
    };
    let vault =
        cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint")
            .await
            .unwrap();

    vault
        .set_config("error_profile", "app_key", "test_key")
        .await
        .unwrap();
    vault
        .set_secret("error_profile", "app_secret", "test_secret")
        .await
        .unwrap();
    vault
        .set_secret("error_profile", "encrypt_key", "test_encrypt_key")
        .await
        .unwrap();
    vault
        .save_app_ticket(
            "test_key",
            cowen_common::models::Ticket {
                value: "test_ticket".to_string(),
                created_at: chrono::Utc::now(),
            },
        )
        .await
        .unwrap();

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd.env("COWEN_SKIP_COMPLETION_INSTALL", "true");
    cmd.env("COWEN_SKIP_BROWSER", "true");
    cmd.arg("--profile").arg("error_profile");
    cmd.arg("auth").arg("login");

    // Should NOT fail with serialization error, but with the reported platform error
    cmd.assert()
        .failure()
        .stderr(predicates::str::contains("Platform error:"));

    // Verify it doesn't contain "Serialization error"
    let output = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Serialization error"),
        "Should not have serialization error, got: {}",
        stderr
    );
    assert!(
        stderr.contains("ticket_expired"),
        "Should contain the complex error content"
    );

    let _ = dir;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_auth_logout_login_flow() {
    // [Given] A self-built app initialized with active token and ticket
    let (addr, _server_handle) = start_mock_platform().await;
    let openapi_url = format!("http://{}", addr);
    let (dir, home) = setup_auth_env("seq_profile", "self-built", &openapi_url);

    let app_cfg = cowen_common::config::AppConfig {
        openapi_url: openapi_url.clone(),
        stream_url: openapi_url.clone(),
        ..Default::default()
    };
    let vault =
        cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint")
            .await
            .unwrap();

    vault
        .set_config("seq_profile", "app_key", "test_key")
        .await
        .unwrap();
    vault
        .set_secret("seq_profile", "app_secret", "test_secret")
        .await
        .unwrap();
    vault
        .set_secret("seq_profile", "encrypt_key", "test_encrypt_key")
        .await
        .unwrap();
    vault
        .save_app_ticket(
            "test_key",
            cowen_common::models::Ticket {
                value: "test_ticket".to_string(),
                created_at: chrono::Utc::now(),
            },
        )
        .await
        .unwrap();

    // [When] The user performs a logout
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd.env("COWEN_SKIP_COMPLETION_INSTALL", "true");
    cmd.env("COWEN_SKIP_BROWSER", "true");
    cmd.arg("--profile").arg("seq_profile");
    cmd.arg("auth").arg("logout");
    cmd.assert().success();

    // Re-seed ticket because logout cleared it (in a real scenario, the platform would push it back or daemon would be running)
    vault
        .save_app_ticket(
            "test_key",
            cowen_common::models::Ticket {
                value: "test_ticket".to_string(),
                created_at: chrono::Utc::now(),
            },
        )
        .await
        .unwrap();

    // [When] The user attempts to login again without providing any new parameters
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd.env("COWEN_SKIP_COMPLETION_INSTALL", "true");
    cmd.env("COWEN_SKIP_BROWSER", "true");
    cmd.arg("--profile").arg("seq_profile");
    cmd.arg("auth").arg("login");

    cmd.assert()
        .success()
        .stdout(predicates::str::contains("Token is active and ready"));

    // [Then] A new valid network token should be fetched and the profile should be active
    let token = vault.get_app_access_token("test_key").await.unwrap();
    assert_eq!(token.value, "mock_at_sb_12345");

    let _ = dir;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_oauth2_refresh_via_token_cmd() {
    let (addr, _server_handle) = start_mock_platform().await;
    let openapi_url = format!("http://{}", addr);
    let (dir, home) = setup_auth_env("oa2_refresh", "oauth2", &openapi_url);

    let app_cfg = cowen_common::config::AppConfig {
        openapi_url: openapi_url.clone(),
        stream_url: openapi_url.clone(),
        ..Default::default()
    };
    let vault =
        cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint")
            .await
            .unwrap();

    vault
        .set_config("oa2_refresh", "app_key", "test_key")
        .await
        .unwrap();
    vault
        .set_secret("oa2_refresh", "app_secret", "test_secret")
        .await
        .unwrap();

    // 1. Set EXPIRED access token and VALID refresh token
    let expired_at = cowen_common::models::Token {
        value: "expired_access_token".to_string(),
        expires_at: chrono::Utc::now() - chrono::Duration::seconds(10), // Expired!
        created_at: chrono::Utc::now() - chrono::Duration::minutes(30),
    };
    vault
        .save_access_token("oa2_refresh", expired_at)
        .await
        .unwrap();

    let valid_rt = cowen_common::models::Token {
        value: "valid_refresh_token".to_string(),
        expires_at: chrono::Utc::now() + chrono::Duration::days(1),
        created_at: chrono::Utc::now() - chrono::Duration::minutes(30),
    };
    vault
        .save_refresh_token("oa2_refresh", valid_rt)
        .await
        .unwrap();

    // 2. Run cowen auth token
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd.arg("auth")
        .arg("token")
        .arg("--profile")
        .arg("oa2_refresh");

    // It should succeed and get a new token via refresh flow
    let output = cmd.assert().success().get_output().stdout.clone();
    let new_token_str = String::from_utf8(output).unwrap();
    assert!(!new_token_str.trim().is_empty());
    assert!(!new_token_str.contains("expired_access_token"));

    // Verify DB was updated
    let new_at = vault.get_access_token("oa2_refresh").await.unwrap();
    assert_eq!(new_at.value, "mock_at_oa2_new"); // from the mock server

    let _ = dir;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_auth_login_oauth2() {
    let (addr, _server_handle) = start_mock_platform().await;
    let openapi_url = format!("http://{}", addr);
    let (dir, home) = setup_auth_env("oa2_profile", "oauth2", &openapi_url);

    let app_cfg = cowen_common::config::AppConfig {
        openapi_url: openapi_url.clone(),
        stream_url: openapi_url.clone(),
        ..Default::default()
    };
    let vault =
        cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint")
            .await
            .unwrap();

    vault
        .set_config("oa2_profile", "app_key", "test_key")
        .await
        .unwrap();
    vault
        .set_secret("oa2_profile", "app_secret", "test_secret")
        .await
        .unwrap();

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
    cmd.env("COWEN_SKIP_BROWSER", "true");
    cmd.arg("--profile").arg("oa2_profile");
    cmd.arg("auth").arg("login");

    cmd.assert().success().stdout(predicates::str::contains(
        "OAuth2 Token Pair has been rotated",
    ));

    let at = vault.get_access_token("oa2_profile").await.unwrap();
    let rt = vault.get_refresh_token("oa2_profile").await.unwrap();
    assert_eq!(at.value, "mock_at_oa2_new");
    assert_eq!(rt.value, "mock_rt_oa2_new");

    // Legacy JSON blob should be purged
    assert!(vault
        .get_config("oa2_profile", "oauth2_token_pair")
        .await
        .is_err());

    let _ = dir;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_auth_logout() {
    let (addr, _server_handle) = start_mock_platform().await;
    let openapi_url = format!("http://{}", addr);
    let (dir, home) = setup_auth_env("logout_profile", "self-built", &openapi_url);

    let app_cfg = cowen_common::config::AppConfig {
        openapi_url: openapi_url.clone(),
        stream_url: openapi_url.clone(),
        ..Default::default()
    };
    let vault =
        cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint")
            .await
            .unwrap();

    // 1. Setup active token and ticket
    vault
        .set_config("logout_profile", "app_key", "test_key")
        .await
        .unwrap();
    vault
        .set_secret("logout_profile", "encrypt_key", "1234567890123456")
        .await
        .unwrap();
    vault
        .save_app_access_token(
            "test_key",
            cowen_common::models::Token {
                value: "active_token".to_string(),
                expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
                created_at: chrono::Utc::now(),
            },
        )
        .await
        .unwrap();
    vault
        .save_app_ticket(
            "test_key",
            cowen_common::models::Ticket {
                value: "active_ticket".to_string(),
                created_at: chrono::Utc::now(),
            },
        )
        .await
        .unwrap();

    // 2. Verify they exist
    assert!(vault.get_app_access_token("test_key").await.is_ok());
    assert!(vault.get_app_ticket("test_key").await.is_ok());

    // 3. Perform logout
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &home);
    cmd.env("HOME", &home);
    cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd.env("COWEN_SKIP_COMPLETION_INSTALL", "true");
    cmd.env("COWEN_SKIP_BROWSER", "true");
    cmd.arg("--profile").arg("logout_profile");
    cmd.arg("auth").arg("logout");

    cmd.assert().success();

    // 4. Verify they are cleared
    assert!(vault.get_app_access_token("test_key").await.is_err());
    assert!(vault.get_app_ticket("test_key").await.is_err());

    let _ = dir;
}

async fn handle_oauth2_refresh_bug_tracking(
    State(state): State<Arc<Mutex<MockState>>>,
    _headers: HeaderMap,
    axum::extract::Form(payload): axum::extract::Form<OAuth2Form>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut s = state.lock().await;
    s.refresh_count += 1;
    s.last_refresh_token_body = Some(json!(payload));
    println!("MOCK SERVER HIT! refresh_count: {}", s.refresh_count);

    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "code": "4007",
            "message": "refresh_token不正确",
            "result": serde_json::Value::Null
        })),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn test_oauth2_refresh_continuous_request_bug() {
    let state = Arc::new(Mutex::new(MockState {
        last_generate_token_body: None,
        last_refresh_token_body: None,
        refresh_count: 0,
    }));

    let app = Router::new()
        .route("/oauth2/token", post(handle_oauth2_refresh_bug_tracking))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let openapi_url = format!("http://{}", addr);

    let _handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let (dir, home) = setup_auth_env("bug_profile", "oauth2", &openapi_url);

    let app_cfg = cowen_common::config::AppConfig {
        openapi_url: openapi_url.clone(),
        stream_url: openapi_url.clone(),
        ..Default::default()
    };
    let vault =
        cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "test_fingerprint")
            .await
            .unwrap();

    vault
        .set_config("bug_profile", "app_key", "test_key")
        .await
        .unwrap();
    vault
        .set_secret("bug_profile", "app_secret", "test_secret")
        .await
        .unwrap();

    let at = cowen_common::models::Token {
        value: "expired_at".to_string(),
        expires_at: chrono::Utc::now() - chrono::Duration::hours(1),
        created_at: chrono::Utc::now() - chrono::Duration::hours(2),
    };
    vault.save_access_token("bug_profile", at).await.unwrap();

    let rt = cowen_common::models::Token {
        value: "active_rt".to_string(),
        expires_at: chrono::Utc::now() + chrono::Duration::days(1),
        created_at: chrono::Utc::now() - chrono::Duration::hours(2),
    };
    vault.save_refresh_token("bug_profile", rt).await.unwrap();

    let current_exe = std::env::current_exe().unwrap();
    let target_dir = current_exe.parent().unwrap().parent().unwrap();
    let daemon_bin = target_dir.join("cowen-daemon");

    let run_cli = |args: &[&str]| {
        let mut cmd = Command::cargo_bin("cowen").unwrap();
        cmd.env("COWEN_HOME", &home);
        cmd.env("HOME", &home);
        cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
        cmd.env("COWEN_SKIP_COMPLETION_INSTALL", "true");
        cmd.env("COWEN_SKIP_BROWSER", "true");
        cmd.env("COWEN_DAEMON_BIN", &daemon_bin);
        cmd.arg("--profile").arg("bug_profile");
        for arg in args {
            cmd.arg(arg);
        }
        cmd.assert()
    };

    // 1. Start daemon
    run_cli(&["daemon", "start"]).success();

    // Give daemon a tiny bit of time to initialize
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    // 2. Trigger token fetch (forces refresh)
    run_cli(&["auth", "token"]);
    run_cli(&["auth", "token"]);
    run_cli(&["auth", "token"]);

    // 3. Stop daemon
    run_cli(&["daemon", "stop"]);

    let final_count = state.lock().await.refresh_count;

    let daemon_stdout =
        std::fs::read_to_string(std::path::Path::new(&home).join("logs/daemon.stdout.log"))
            .unwrap_or_default();
    println!("DAEMON STDOUT:\n{}", daemon_stdout);
    let daemon_stderr =
        std::fs::read_to_string(std::path::Path::new(&home).join("logs/daemon.stderr.log"))
            .unwrap_or_default();
    println!("DAEMON STDERR:\n{}", daemon_stderr);

    assert_eq!(
        final_count, 1,
        "Mock server should only be hit ONCE before oauth2_revoked short-circuits further attempts"
    );

    let _ = dir;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_auth_ipc_sync() {
    let (addr, _server_handle) = start_mock_platform().await;
    let openapi_url = format!("http://{}", addr);
    let (dir, home) =
        crate::e2e::rust::auth_tests::setup_auth_env("case_51", "oauth2", &openapi_url);

    // 1. Start daemon
    let mut cmd_daemon = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    cmd_daemon.env("COWEN_HOME", &home);
    cmd_daemon.env("HOME", &home);
    cmd_daemon.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_daemon.args(["daemon", "start", "--foreground"]);

    // We spawn it so it runs in background of test
    let mut child = cmd_daemon.spawn().unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // 2. Trigger OAuth2 Init Flow
    let mut cmd_init = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_init.env("COWEN_HOME", &home);
    cmd_init.env("HOME", &home);
    cmd_init.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_init.args([
        "init",
        "--profile",
        "case_51",
        "--app-mode",
        "oauth2",
        "--openapi-url",
        &openapi_url,
        "--stream-url",
        &openapi_url,
        "--webhook-target",
        &format!("{}/webhook_sink", openapi_url),
        "--no-telemetry",
    ]);

    // Use a background process for init to capture redirect URI
    let mut init_child = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"))
        .env("COWEN_HOME", &home)
        .env("HOME", &home)
        .env("COWEN_FS_FINGERPRINT", "test_fingerprint")
        .args([
            "init",
            "--profile",
            "case_51",
            "--app-mode",
            "oauth2",
            "--openapi-url",
            &openapi_url,
            "--stream-url",
            &openapi_url,
            "--webhook-target",
            &format!("{}/webhook_sink", openapi_url),
            "--no-telemetry",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let mut port = None;
    let mut state_val = None;

    // We need to read from init stdout without blocking indefinitely.
    if let Some(stdout) = init_child.stdout.take() {
        let mut reader = std::io::BufReader::new(stdout);
        use std::io::BufRead;
        for line_res in (&mut reader).lines() {
            let line = line_res.unwrap_or_default();
            if line.contains("redirect_uri=") {
                // e.g. redirect_uri=http%3A%2F%2F127.0.0.1%3A50529%2Fcallback
                // Extract port
                let parts: Vec<&str> = line.split("127.0.0.1%3A").collect();
                if parts.len() > 1 {
                    let port_part: String = parts[1]
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    port = Some(port_part);
                }

                let state_parts: Vec<&str> = line.split("state=").collect();
                if state_parts.len() > 1 {
                    let st: String = state_parts[1].chars().take_while(|c| *c != '&').collect();
                    state_val = Some(st);
                }
                break;
            }
        }
        // Consume the rest of stdout in a background thread to prevent SIGPIPE
        std::thread::spawn(move || {
            let mut buf = String::new();
            use std::io::Read;
            let mut reader = reader;
            let _ = reader.read_to_string(&mut buf);
        });
    }

    assert!(port.is_some(), "Could not find redirect port");
    let port = port.unwrap();
    let state_val = state_val.unwrap_or_else(|| "123".to_string());

    // 3. Simulate Browser Callback
    let client = reqwest::Client::new();
    let cb_url = format!(
        "http://127.0.0.1:{}/callback?code=mock_auth_code_case_51&state={}",
        port, state_val
    );
    let res = client.get(&cb_url).send().await;
    assert!(res.is_ok());

    let status = init_child.wait().unwrap();
    if !status.success() {
        eprintln!("Init exited with {:?}", status);
        assert!(
            status.success(),
            "Init should complete successfully after callback"
        );
    }
    // 5. Verify Token in Vault
    let mut cmd_status = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_status.env("COWEN_HOME", &home);
    cmd_status.env("HOME", &home);
    cmd_status.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_status.args(["status", "--profile", "case_51"]);

    let output = String::from_utf8_lossy(&cmd_status.output().unwrap().stdout).to_string();
    assert!(
        output.contains("AccessToken"),
        "Token should be present in status"
    );

    // Clean up
    let _ = child.kill();
    let _ = child.wait();
    let _ = dir;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_profile_rename_auth() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let home_str = home.to_str().unwrap();

    let (mock_addr, _jh) = start_mock_platform().await;
    let mock_url = format!("http://{}", mock_addr);
    let mock_ws = format!("ws://{}/connect", mock_addr);
    let proxy_port = mock_addr.port() + 110;

    let profile = "p1";

    let mut init_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd.env("COWEN_HOME", home_str);
    init_cmd.env("HOME", home_str);
    init_cmd.args([
        "init",
        "--profile",
        profile,
        "--app-mode",
        "oauth2",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--proxy-port",
        &proxy_port.to_string(),
    ]);

    let mut init_child = init_cmd.spawn().expect("failed to spawn init");

    // Wait for auth session
    let db_path = home.join("cowen.db");
    let mut session_json = String::new();
    for _ in 0..40 {
        if db_path.exists() {
            let out = std::process::Command::new("sqlite3")
                .arg(&db_path)
                .arg("SELECT item_value FROM cowen_token WHERE profile='global' AND item_key LIKE 'session:%' LIMIT 1;")
                .output()
                .unwrap();
            let res = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !res.is_empty() {
                session_json = res;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    assert!(!session_json.is_empty(), "Timeout waiting for auth session");

    let session: serde_json::Value = serde_json::from_str(&session_json).unwrap();
    let redirect_port = session.get("redirect_port").unwrap().as_u64().unwrap();
    let state = session.get("state").unwrap().as_str().unwrap();

    // Simulate browser callback
    let client = reqwest::Client::new();
    let callback_url = format!(
        "http://127.0.0.1:{}/callback?code=mock_code&state={}",
        redirect_port, state
    );

    let mut success = false;
    for _ in 0..40 {
        if let Ok(resp) = client.get(&callback_url).send().await {
            if resp.status().is_success() {
                success = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    assert!(success, "Redirect port unreachable");

    let _ = init_child.wait().unwrap();

    // Verify initial login status
    let mut status_cmd = Command::cargo_bin("cowen").unwrap();
    status_cmd.env("COWEN_HOME", home_str);
    status_cmd.env("HOME", home_str);
    status_cmd.args(["status", "--profile", profile]);
    let status_out = String::from_utf8_lossy(&status_cmd.output().unwrap().stdout).to_string();
    assert!(
        !status_out.contains("Not logged in or session expired"),
        "Initial login failed"
    );

    // Rename profile
    let mut rename_cmd = Command::cargo_bin("cowen").unwrap();
    rename_cmd.env("COWEN_HOME", home_str);
    rename_cmd.env("HOME", home_str);
    rename_cmd.args(["profile", "rename", profile, "p2"]);
    rename_cmd.assert().success();

    // Verify file system residuals
    let entries = std::fs::read_dir(&home).unwrap();
    for entry in entries.flatten() {
        let name = entry.file_name().into_string().unwrap();
        assert!(!name.starts_with(profile), "Residual file found: {}", name);
    }

    // Verify status after rename
    let mut status2_cmd = Command::cargo_bin("cowen").unwrap();
    status2_cmd.env("COWEN_HOME", home_str);
    status2_cmd.env("HOME", home_str);
    status2_cmd.args(["status", "--profile", "p2"]);
    let status2_out = String::from_utf8_lossy(&status2_cmd.output().unwrap().stdout).to_string();
    assert!(
        !status2_out.contains("Not logged in or session expired"),
        "Authentication lost after rename"
    );
}
