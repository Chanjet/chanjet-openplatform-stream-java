use axum::{routing::{get, post}, Router, response::IntoResponse, extract::{State, Query}};
use serde::Deserialize;
use std::sync::Arc;
use cowen_common::daemon::DaemonService;

use prometheus::{Encoder, TextEncoder};
use std::net::SocketAddr;
use tokio::sync::oneshot;
use crate::mgmt::{AuthManager, finalize_auth_handler, progress_handler};

pub struct MonitorServer {
    port: u16,
    daemon_svc: Arc<dyn DaemonService>,
    auth_mgr: Arc<AuthManager>,
}

impl MonitorServer {
    pub fn new(port: u16, daemon_svc: Arc<dyn DaemonService>) -> Self {
        Self { 
            port, 
            daemon_svc,
            auth_mgr: Arc::new(AuthManager::new()),
        }
    }

    pub async fn start(&self, port_tx: Option<oneshot::Sender<u16>>, allow_fallback: bool) -> anyhow::Result<()> {
        let app = Router::new()
            .route("/health", get(health_handler))
            .route("/metrics", get(metrics_handler))
            .route("/daemon/reload", post(reload_handler))
            .route("/v1/mgmt/auth/finalize", post(finalize_auth_handler))
            .route("/v1/mgmt/auth/progress", get(progress_handler))
            .with_state((self.daemon_svc.clone(), self.auth_mgr.clone()));

        // 🚀 RELIABILITY: Retry binding if port is temporarily occupied, then fallback to random port
        let mut current_port = self.port;
        let mut addr = SocketAddr::from(([127, 0, 0, 1], current_port));
        let mut retry_count = 0;
        let listener = loop {
            match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => break l,
                Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                    if current_port != 0 && retry_count < 2 {
                        tracing::warn!(target: "sys", "Monitor port {} in use, retrying in 500ms... ({} / 2)", current_port, retry_count + 1);
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        retry_count += 1;
                    } else if allow_fallback {
                        tracing::warn!(target: "sys", "Monitor port {} in use after retries, falling back to random port.", current_port);
                        current_port = 0;
                        addr.set_port(0);
                    } else {
                        return Err(anyhow::anyhow!("Monitor port {} is in use. Please manually configure a new monitor_port in app.yaml or use `cowen config set monitor_port <PORT>`.", current_port));
                    }
                }
                Err(e) => return Err(anyhow::anyhow!("Failed to bind to monitor port {}: {}", current_port, e)),
            }
        };

        let actual_port = listener.local_addr()?.port();
        tracing::info!(target: "sys", "Monitor server listening on http://127.0.0.1:{}", actual_port);
        
        if let Some(tx) = port_tx {
            let _ = tx.send(actual_port);
        }

        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn health_handler() -> impl IntoResponse {
    "UP"
}

async fn metrics_handler() -> impl IntoResponse {
    let _gauge = crate::gauge!("cowen_active_connections", "Number of active streaming connections");

    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

#[derive(Deserialize)]
struct ReloadQuery {
    profile: Option<String>,
}

async fn reload_handler(
    State((daemon_svc, _)): State<(Arc<dyn DaemonService>, Arc<AuthManager>)>,
    Query(query): Query<ReloadQuery>,
) -> impl IntoResponse {
    if let Some(profile) = query.profile {
        match daemon_svc.reload_daemon(&profile).await {
            Ok(_) => "OK".into_response(),
            Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to reload: {}", e)).into_response(),
        }
    } else {
        (axum::http::StatusCode::BAD_REQUEST, "Missing profile query parameter").into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_contains_active_connections() {
        let response = metrics_handler().await;
        use axum::response::IntoResponse;
        let response = response.into_response();
        let body_bytes = axum::body::to_bytes(response.into_body(), 100000).await.unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert!(body_str.contains("cowen_active_connections"));
    }
}

