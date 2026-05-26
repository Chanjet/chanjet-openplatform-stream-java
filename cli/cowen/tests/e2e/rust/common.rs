use std::fs;
use serde_json::json;
use tempfile::tempdir;

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
use std::net::SocketAddr;

pub struct MockState {
    pub last_generate_token_body: Option<serde_json::Value>,
    pub last_refresh_token_body: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct OAuth2Form {
    pub grant_type: String,
    pub client_id: String,
    pub refresh_token: Option<String>,
    pub code: Option<String>,
}

async fn handle_generate_token(
    State(state): State<Arc<Mutex<MockState>>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut s = state.lock().await;
    s.last_generate_token_body = Some(body);

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

pub async fn start_mock_platform() -> (SocketAddr, tokio::task::JoinHandle<()>) {
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

pub fn setup_test_env(profile: &str, mode: &str, openapi_url: &str) -> (tempfile::TempDir, String) {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    fs::create_dir_all(&cowen_home).unwrap();

    // Create profile config
    let config_path = cowen_home.join(format!("{}.yaml", profile));
    let config = json!({
        "app_key": "test_key",
        "app_mode": mode,
        "encrypt_key": "1234567890123456",
        "webhook_target": "http://localhost:8080",
        "version": 1
    });
    fs::write(config_path, serde_yaml::to_string(&config).unwrap()).unwrap();

    // Create app config
    let app_config_path = cowen_home.join("app.yaml");
    let app_config = json!({
        "openapi_url": openapi_url,
        "stream_url": openapi_url,
        "telemetry_enabled": false,
        "log": { "level": "info" }
    });
    fs::write(app_config_path, serde_yaml::to_string(&app_config).unwrap()).unwrap();

    // Setup Binaries
    let bin_dir = dir.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let current_dir = std::env::current_dir().unwrap();
    
    // Potential target directories to search for binaries
    let mut search_paths = vec![
        current_dir.join("target").join("debug"),
        current_dir.join("target").join("release"),
    ];

    // If CARGO_TARGET_DIR is set, prioritize it
    if let Ok(target_env) = std::env::var("CARGO_TARGET_DIR") {
        let base = if target_env.starts_with('/') {
            std::path::PathBuf::from(target_env)
        } else {
            current_dir.join(target_env)
        };
        
        // Add common subdirectories within the custom target dir
        search_paths.insert(0, base.join("x86_64-unknown-linux-gnu").join("release"));
        search_paths.insert(1, base.join("x86_64-unknown-linux-gnu").join("debug"));
        search_paths.insert(2, base.join("release"));
        search_paths.insert(3, base.join("debug"));
    }

    let mut cli_found = false;
    let mut daemon_found = false;

    for path in search_paths {
        let cli_src = path.join("cowen");
        let daemon_src = path.join("cowen-daemon");

        if !cli_found && cli_src.exists() {
            fs::copy(&cli_src, bin_dir.join("cowen")).unwrap();
            cli_found = true;
        }
        if !daemon_found && daemon_src.exists() {
            fs::copy(&daemon_src, bin_dir.join("cowen-daemon")).unwrap();
            daemon_found = true;
        }
        
        if cli_found && daemon_found { break; }
    }

    if !cli_found {
        panic!("Could not find 'cowen' binary in any expected target directory. Search paths: {:?}", 
            current_dir); // Simplification for error reporting
    }

    (dir, cowen_home.to_str().unwrap().to_string())
}

