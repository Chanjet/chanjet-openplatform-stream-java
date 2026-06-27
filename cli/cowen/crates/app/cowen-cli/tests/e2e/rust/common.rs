use serde_json::json;
use std::fs;
use tempfile::tempdir;

pub struct DaemonKiller {
    pub home: String,
}

impl Drop for DaemonKiller {
    fn drop(&mut self) {
        let pid_file = std::path::PathBuf::from(&self.home).join("master_daemon.pid");
        eprintln!(
            "DEBUG_TEST: DaemonKiller dropping. home={}, pid_file={:?}, exists={}",
            self.home,
            pid_file,
            pid_file.exists()
        );
        if let Ok(content) = std::fs::read_to_string(&pid_file) {
            if let Some(pid_str) = content.lines().next() {
                if let Ok(pid) = pid_str.trim().parse::<i32>() {
                    eprintln!("DEBUG_TEST: DaemonKiller killing master daemon pid {}", pid);
                    let kill_status = std::process::Command::new("kill")
                        .arg("-15")
                        .arg(pid.to_string())
                        .status();
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    eprintln!("DEBUG_TEST: kill -9 status: {:?}", kill_status);
                }
            }
        } else {
            eprintln!("DEBUG_TEST: DaemonKiller failed to read pid file");
        }

        // Also stop workers gracefully if possible
        let bin_path = assert_cmd::cargo::cargo_bin("cowen");
        let _ = std::process::Command::new(bin_path)
            .env("COWEN_HOME", &self.home)
            .arg("daemon")
            .arg("stop")
            .output();

        let _ = std::fs::remove_file(pid_file);
    }
}

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

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
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code": "50001", "message": "appKey/appSecret missing"})),
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

pub async fn start_mock_platform() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let state = Arc::new(Mutex::new(MockState {
        last_generate_token_body: None,
        last_refresh_token_body: None,
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

pub fn setup_test_env(
    profile: &str,
    mode: &str,
    openapi_url: &str,
) -> (
    tempfile::TempDir,
    String,
    crate::e2e::rust::common::DaemonKiller,
) {
    let dir = tempdir().unwrap();
    let home = dir.path().to_str().unwrap().to_string();
    let killer = setup_test_env_in_dir(profile, mode, openapi_url, &home);
    let cowen_home = std::path::PathBuf::from(home).join(".cowen");
    (dir, cowen_home.to_str().unwrap().to_string(), killer)
}

pub fn setup_test_env_in_dir(
    profile: &str,
    mode: &str,
    openapi_url: &str,
    home: &str,
) -> crate::e2e::rust::common::DaemonKiller {
    let cowen_home = std::path::PathBuf::from(home).join(".cowen");
    fs::create_dir_all(&cowen_home).unwrap();

    let profiles_dir = cowen_home.join("profiles");
    fs::create_dir_all(&profiles_dir).unwrap();
    let config_path = profiles_dir.join(format!("{}.yaml", profile));
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
    let bin_dir = std::path::PathBuf::from(home).join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let current_dir = std::env::current_dir().unwrap();

    // Potential target directories to search for binaries
    let mut search_paths = vec![
        current_dir.join("target").join("debug"),
        current_dir.join("target").join("release"),
        current_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("target")
            .join("debug"),
        current_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("target")
            .join("release"),
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

    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let mut cli_found = false;
    let mut daemon_found = false;

    for path in search_paths {
        let cli_src = path.join(format!("cowen{}", exe_suffix));
        let daemon_src = path.join(format!("cowen-daemon{}", exe_suffix));

        if !cli_found && cli_src.exists() {
            let dest = bin_dir.join(format!("cowen{}", exe_suffix));
            #[cfg(unix)]
            {
                let _ = fs::remove_file(&dest);
                std::os::unix::fs::symlink(&cli_src, &dest).unwrap();
            }
            #[cfg(not(unix))]
            {
                fs::copy(&cli_src, &dest).unwrap();
            }
            cli_found = true;
        }
        if !daemon_found && daemon_src.exists() {
            let dest = bin_dir.join(format!("cowen-daemon{}", exe_suffix));
            #[cfg(unix)]
            {
                let _ = fs::remove_file(&dest);
                std::os::unix::fs::symlink(&daemon_src, &dest).unwrap();
            }
            #[cfg(not(unix))]
            {
                fs::copy(&daemon_src, &dest).unwrap();
            }
            daemon_found = true;
        }

        if cli_found && daemon_found {
            break;
        }
    }

    if !cli_found {
        panic!(
            "Could not find 'cowen' binary in any expected target directory. Search paths: {:?}",
            current_dir
        ); // Simplification for error reporting
    }

    crate::e2e::rust::common::DaemonKiller {
        home: cowen_home.to_str().unwrap().to_string(),
    }
}
