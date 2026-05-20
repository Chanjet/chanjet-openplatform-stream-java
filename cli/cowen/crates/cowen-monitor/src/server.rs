use axum::{routing::{get, post}, Router, response::IntoResponse, extract::{State, Query}};
use serde::Deserialize;
use std::sync::Arc;
use cowen_common::daemon::DaemonService;

use prometheus::{Encoder, TextEncoder};
use std::net::SocketAddr;
use tokio::sync::oneshot;

pub struct MonitorServer {
    port: u16,
    daemon_svc: Arc<dyn DaemonService>,
}

impl MonitorServer {
    pub fn new(port: u16, daemon_svc: Arc<dyn DaemonService>) -> Self {
        Self { port, daemon_svc }
    }

    pub async fn start(&self, port_tx: Option<oneshot::Sender<u16>>) -> anyhow::Result<()> {
        let app = Router::new()
            .route("/health", get(health_handler))
            .route("/metrics", get(metrics_handler))
            .route("/daemon/reload", post(reload_handler))
            .with_state(self.daemon_svc.clone());

        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        
        // 🚀 RELIABILITY: Retry binding if port is temporarily occupied
        let mut retry_count = 0;
        let listener = loop {
            match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => break l,
                Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && retry_count < 5 => {
                    tracing::warn!(target: "sys", "Monitor port {} in use, retrying in 500ms... ({} / 5)", self.port, retry_count + 1);
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    retry_count += 1;
                }
                Err(e) => return Err(anyhow::anyhow!("Failed to bind to monitor port {}: {}", self.port, e)),
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
    State(daemon_svc): State<Arc<dyn DaemonService>>,
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
