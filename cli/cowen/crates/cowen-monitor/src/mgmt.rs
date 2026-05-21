use axum::{
    extract::{State, Query},
    Json,
    response::IntoResponse,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use cowen_common::daemon::DaemonService;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProgressInfo {
    pub profile: String,
    pub status: AuthStatus,
    pub message: String,
    pub percent: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum AuthStatus {
    Idle,
    Starting,
    Exchanging,
    Saving,
    Completed,
    Failed,
}

pub struct AuthManager {
    progress: Mutex<HashMap<String, AuthProgressInfo>>,
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            progress: Mutex::new(HashMap::new()),
        }
    }

    pub async fn update_progress(&self, profile: &str, status: AuthStatus, message: &str, percent: u32, error: Option<String>) {
        let mut progress = self.progress.lock().await;
        progress.insert(profile.to_string(), AuthProgressInfo {
            profile: profile.to_string(),
            status,
            message: message.to_string(),
            percent,
            error,
        });
    }

    pub async fn get_progress(&self, profile: &str) -> Option<AuthProgressInfo> {
        let progress = self.progress.lock().await;
        progress.get(profile).cloned()
    }
}

#[derive(Serialize, Deserialize)]
pub struct FinalizeRequest {
    pub profile: String,
    pub code: String,
    pub state: Option<String>,
    pub session_id: String,
}

pub async fn finalize_auth_handler(
    State((daemon_svc, auth_mgr)): State<(Arc<dyn DaemonService>, Arc<AuthManager>)>,
    Json(payload): Json<FinalizeRequest>,
) -> impl IntoResponse {
    let profile = payload.profile.clone();
    let auth_mgr_clone = auth_mgr.clone();
    let daemon_svc_clone = daemon_svc.clone();
    
    // Initial status
    auth_mgr.update_progress(&profile, AuthStatus::Starting, "Received authorization code", 10, None).await;
    
    tokio::spawn(async move {
        auth_mgr_clone.update_progress(&profile, AuthStatus::Exchanging, "Exchanging token via daemon...", 30, None).await;
        
        match daemon_svc_clone.finalize_auth(&payload.profile, &payload.code, payload.state.as_deref(), &payload.session_id).await {
            Ok(_) => {
                auth_mgr_clone.update_progress(&profile, AuthStatus::Completed, "Authorization successful", 100, None).await;
            }
            Err(e) => {
                auth_mgr_clone.update_progress(&profile, AuthStatus::Failed, &format!("Authorization failed: {}", e), 0, Some(e.to_string())).await;
            }
        }
    });

    StatusCode::ACCEPTED
}

#[derive(Deserialize)]
pub struct ProgressQuery {
    pub profile: String,
}

pub async fn progress_handler(
    State((_, auth_mgr)): State<(Arc<dyn DaemonService>, Arc<AuthManager>)>,
    Query(query): Query<ProgressQuery>,
) -> impl IntoResponse {
    if let Some(info) = auth_mgr.get_progress(&query.profile).await {
        Json(info).into_response()
    } else {
        (StatusCode::NOT_FOUND, "No progress found for profile").into_response()
    }
}
