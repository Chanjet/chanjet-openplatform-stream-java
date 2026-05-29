use axum::{
    extract::{Request, State},
    response::IntoResponse,
    routing::any,
    Router,
};
use reqwest::Client;
use std::net::SocketAddr;
use std::sync::Arc;
use cowen_common::config::Config;
use cowen_common::vault::Vault;
use cowen_common::{CowenResult, CowenError};

#[derive(Clone)]
pub struct ProxyState {
    pub client: Client,
    pub config: Config,
    pub profile: String,
    pub vault: Arc<dyn Vault>,
}

pub async fn start_proxy(
    profile: &str,
    config: &Config,
    vault: Arc<dyn Vault>,
    port: u16,
    port_tx: Option<tokio::sync::oneshot::Sender<u16>>,
) -> CowenResult<()> {
    let state = ProxyState {
        client: Client::new(),
        config: config.clone(),
        profile: profile.to_string(),
        vault,
    };

    let app = Router::new()
        .route("/*path", any(handle_proxy))
        .route("/", any(handle_proxy))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    cowen_infra::validate_loopback_addr(&addr).map_err(cowen_common::CowenError::Security)?;
    
    // Retry logic for binding to handle port release delay during reloads
    let mut retry_count = 0;
    let listener = loop {
        let socket = match addr {
            SocketAddr::V4(_) => tokio::net::TcpSocket::new_v4().map_err(|e| CowenError::Store(format!("Failed to create socket: {}", e)))?,
            SocketAddr::V6(_) => tokio::net::TcpSocket::new_v6().map_err(|e| CowenError::Store(format!("Failed to create socket: {}", e)))?,
        };
        let _ = cowen_sys::configure_socket_reuse(&socket);

        match socket.bind(addr) {
            Ok(_) => {
                match socket.listen(1024) {
                    Ok(l) => break l,
                    Err(e) => return Err(CowenError::Store(format!("Failed to listen on proxy port {}: {}", port, e))),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && retry_count < 10 => {
                tracing::warn!(target: "sys", "Proxy port {} in use, retrying in 500ms... ({} / 10)", port, retry_count + 1);
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                retry_count += 1;
            }
            Err(e) => return Err(CowenError::Store(format!("Failed to bind to proxy port {}: {}", port, e))),
        }
    };
    
    let local_addr = listener.local_addr().map_err(|e| CowenError::Store(format!("Failed to get local addr: {}", e)))?;
    let actual_port = local_addr.port();
    tracing::info!(target: "sys", "Local Proxy Server listening on http://{}", local_addr);
    
    if let Some(tx) = port_tx {
        let _ = tx.send(actual_port);
    }
    
    axum::serve(listener, app).await.map_err(CowenError::from)?;
    Ok(())
}

async fn handle_proxy(
    State(state): State<ProxyState>,
    req: Request,
) -> axum::response::Response {
    cowen_common::events::event_bus().publish(cowen_common::events::GlobalEvent::ProxyRequestReceived);
    let method_str = req.method().as_str().to_uppercase();

    let origin_header = req.headers().get("origin").and_then(|v| v.to_str().ok()).unwrap_or("");
    let allow_origin = if origin_header.contains("localhost") || origin_header.contains("127.0.0.1") {
        origin_header.to_string()
    } else {
        "http://127.0.0.1".to_string()
    };

    if method_str == "OPTIONS" {
        tracing::info!(target: "sys", "Intercepted OPTIONS pre-flight request, returning CORS headers immediately.");
        return axum::response::Response::builder()
            .status(axum::http::StatusCode::NO_CONTENT)
            .header("Access-Control-Allow-Origin", &allow_origin)
            .header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, PATCH, OPTIONS")
            .header("Access-Control-Allow-Headers", "*")
            .header("Access-Control-Max-Age", "86400")
            .body(axum::body::Body::empty())
            .unwrap();
    }

    // 0. Extract Parts
    let (parts, body) = req.into_parts();
    
    // Helper for CORS-enabled error responses
    let allow_origin_clone = allow_origin.clone();
    let cors_error = |status: axum::http::StatusCode, msg: String| {
        axum::response::Response::builder()
            .status(status)
            .header("Access-Control-Allow-Origin", &allow_origin_clone)
            .body(axum::body::Body::from(msg))
            .unwrap()
    };

    let app_cfg = match cowen_config::ConfigManager::new() {
        Ok(mgr) => match mgr.load_app_config().await {
            Ok(cfg) => cfg,
            Err(e) => return cors_error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        },
        Err(e) => return cors_error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    let base_url = app_cfg.openapi_url.trim_end_matches('/');
    let req_path_and_query = parts.uri.path_and_query().map(|x| x.as_str()).unwrap_or("/");
    let target_url = format!("{}{}", base_url, if req_path_and_query.starts_with('/') { req_path_and_query.to_string() } else { format!("/{}", req_path_and_query) });
    
    tracing::info!(target: "audit", profile = %state.profile, "Proxying {} request to: {}", parts.method, target_url);

    let req_path = parts.uri.path().to_string();


    // 1. Resolve Auth directly reusing the shared Vault O(1)
    let auth_cli = cowen_auth::create_auth_client_with_vault(state.vault.clone());

    // 1.1. Read body early
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(e) => return cors_error(axum::http::StatusCode::BAD_REQUEST, format!("Failed to read body: {}", e)),
    };

    let provider = auth_cli.provider(&state.config.app_mode);

    // Convert axum headers to reqwest headers
    let mut headers = reqwest::header::HeaderMap::new();
    for (k, v) in parts.headers.iter() {
        if let Ok(name) = reqwest::header::HeaderName::from_bytes(k.as_str().as_bytes()) {
            if let Ok(val) = reqwest::header::HeaderValue::from_bytes(v.as_bytes()) {
                headers.insert(name, val);
            }
        }
    }

    // 🚀 Intelligent Header Stripping: Some upstream servers (e.g. Tomcat 9) strictly reject
    // GET/HEAD/DELETE requests with a Content-Type if there's no actual body payload.
    if bytes.is_empty() && (parts.method == axum::http::Method::GET || parts.method == axum::http::Method::HEAD || parts.method == axum::http::Method::DELETE) {
        headers.remove(reqwest::header::CONTENT_TYPE);
    }

    // 1.2. Pre-flight Interceptor
    let fallback_spec = serde_json::Value::Null;
    let intercept_result = match provider.intercept_request(&state.profile, &state.config, &req_path, &method_str, headers, &bytes, &fallback_spec).await {
        Ok(res) => res,
        Err(e) => {
            let masked_err = cowen_common::utils::mask_sensitive_json(&e.to_string());
            tracing::error!(target: "audit", profile = %state.profile, method = %method_str, path = %req_path, error = %masked_err, "Proxy pre-flight intercept failed");
            return cors_error(axum::http::StatusCode::UNAUTHORIZED, format!("Pre-flight error: {}", masked_err));
        }
    };

    // 1.3. Dispatch
    let reqwest_headers = match intercept_result {
        cowen_auth::provider::ProxyRequestAction::Respond(json_resp) => {
            let body_bytes = serde_json::to_vec(&json_resp).unwrap_or_default();
            return axum::response::Response::builder()
                .status(axum::http::StatusCode::OK)
                .header("Access-Control-Allow-Origin", &allow_origin)
                .header("Content-Type", "application/json")
                .body(axum::body::Body::from(body_bytes))
                .unwrap();
        }
        cowen_auth::provider::ProxyRequestAction::Forward { mut headers } => {
            headers.remove(reqwest::header::HOST);
            headers
        }
    };

    let method = reqwest::Method::from_bytes(parts.method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET);

    // 2. Forward
    let fwd_req = state.client.request(method, target_url)
        .headers(reqwest_headers)
        .body(reqwest::Body::from(bytes));

    let res = fwd_req.send().await;

    // 3. Post-flight Interceptor & Respond
    match res {
        Ok(r) => {
            let status = r.status().as_u16();
            let mut builder = axum::response::Response::builder().status(status);
            
            let mut response_headers = reqwest::header::HeaderMap::new();
            for (k, v) in r.headers() {
                if let Ok(name) = axum::http::HeaderName::from_bytes(k.as_str().as_bytes()) {
                    if let Ok(val) = axum::http::HeaderValue::from_bytes(v.as_bytes()) {
                        builder = builder.header(name, val);
                        response_headers.insert(k.clone(), v.clone());
                    }
                }
            }
            let out_headers = r.headers().clone();
            let out_bytes = r.bytes().await.unwrap_or_default();
            
            // Execute Post-flight Interceptor
            if let Err(e) = provider.intercept_response(&state.profile, &state.config, &req_path, &method_str, status, &out_headers, &out_bytes).await {
                tracing::warn!(target: "audit", profile = %state.profile, error = %e, "Proxy post-flight intercept failed (non-fatal)");
            }
            
            tracing::info!(target: "audit", profile = %state.profile, method = %method_str, path = %req_path, status = %status, "Request successfully proxied");

            builder
                .body(axum::body::Body::from(out_bytes)).unwrap_or_else(|_| (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to construct response"
            ).into_response())
        }
        Err(e) => {
            tracing::error!(target: "audit", profile = %state.profile, method = %method_str, path = %req_path, error = %e, "Proxy upstream request failed");
            cors_error(axum::http::StatusCode::BAD_GATEWAY, format!("Proxy upstream error: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cowen_common::config::AppConfig;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_proxy_concurrency_no_deadlock() {
        // 1. Setup mock upstream server
        let mock_app = Router::new().route("/hello", any(|| async {
            axum::response::Response::builder()
                .status(axum::http::StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(axum::body::Body::from(r#"{"status":"ok"}"#))
                .unwrap()
        }));
        let mock_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mock_addr = mock_listener.local_addr().unwrap();
        let mock_server_handle = tokio::spawn(async move {
            axum::serve(mock_listener, mock_app).await.unwrap();
        });

        // 2. Setup temp directory for local Vault using UUID to avoid dependency on tempfile
        let temp_dir = std::env::temp_dir().join(format!("cowen_proxy_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let app_cfg = AppConfig::default();
        let vault = cowen_store::create_vault(&app_cfg, &temp_dir, "test_fingerprint").await.unwrap();

        // Seed some configs
        vault.set_config("test_profile", "app_key", "test_key").await.unwrap();
        vault.set_secret("test_profile", "app_secret", "test_secret").await.unwrap();
        vault.save_app_access_token("test_key", cowen_common::models::Token {
            value: "mock_at_sb_12345".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            created_at: chrono::Utc::now(),
        }).await.unwrap();

        // 3. Start proxy
        let mut config = Config::default_with_profile("test_profile");
        config.app_key = "test_key".to_string();
        config.app_mode = cowen_common::models::AuthMode::SelfBuilt;
        std::env::set_var("COWEN_OPENAPI_URL", format!("http://{}", mock_addr));
        std::env::set_var("COWEN_STREAM_URL", format!("http://{}", mock_addr));
        config.webhook_target = "http://localhost:8080".to_string();

        let (port_tx, port_rx) = tokio::sync::oneshot::channel();
        let p_vault = vault.clone();
        let p_config = config.clone();

        let proxy_task = tokio::spawn(async move {
            start_proxy("test_profile", &p_config, p_vault, 0, Some(port_tx)).await.unwrap();
        });

        let proxy_port = port_rx.await.unwrap();
        let client = reqwest::Client::new();

        // 4. Send concurrent requests to the proxy
        let mut futures = vec![];
        for _ in 0..10 {
            let client_clone = client.clone();
            let url = format!("http://127.0.0.1:{}/hello", proxy_port);
            futures.push(tokio::spawn(async move {
                let resp = client_clone.get(&url).send().await.unwrap();
                assert_eq!(resp.status(), axum::http::StatusCode::OK);
                let text = resp.text().await.unwrap();
                assert!(text.contains("ok"));
            }));
        }

        // Wait for all requests to complete
        for fut in futures {
            fut.await.unwrap();
        }

        // Cleanup and shutdown mock server/proxy
        mock_server_handle.abort();
        proxy_task.abort();
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
