use axum::{
    extract::{Request, State},
    response::IntoResponse,
    routing::any,
    Router,
};
use reqwest::Client;
use std::net::SocketAddr;
use cowen_common::config::Config;
use cowen_common::{CowenResult, CowenError};

#[derive(Clone)]
pub struct ProxyState {
    pub client: Client,
    pub config: Config,
    pub profile: String,
}

pub async fn start_proxy(
    profile: &str,
    config: &Config,
    port: u16,
    port_tx: Option<tokio::sync::oneshot::Sender<u16>>,
) -> CowenResult<()> {
    let state = ProxyState {
        client: Client::new(),
        config: config.clone(),
        profile: profile.to_string(),
    };

    let app = Router::new()
        .route("/*path", any(handle_proxy))
        .route("/", any(handle_proxy))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    cowen_infra::validate_loopback_addr(&addr).map_err(|e| cowen_common::CowenError::Security(e))?;
    
    // Retry logic for binding to handle port release delay during reloads
    let mut retry_count = 0;
    let listener = loop {
        match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => break l,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && retry_count < 5 => {
                tracing::warn!(target: "sys", "Proxy port {} in use, retrying in 500ms... ({} / 5)", port, retry_count + 1);
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
    let method_str = req.method().as_str().to_uppercase();

    if method_str == "OPTIONS" {
        tracing::info!(target: "sys", "Intercepted OPTIONS pre-flight request, returning CORS headers immediately.");
        return axum::response::Response::builder()
            .status(axum::http::StatusCode::NO_CONTENT)
            .header("Access-Control-Allow-Origin", "*")
            .header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, PATCH, OPTIONS")
            .header("Access-Control-Allow-Headers", "*")
            .header("Access-Control-Max-Age", "86400")
            .body(axum::body::Body::empty())
            .unwrap();
    }

    // 0. Extract Parts
    let (parts, body) = req.into_parts();
    
    let base_url = state.config.openapi_url.trim_end_matches('/');
    let req_path_and_query = parts.uri.path_and_query().map(|x| x.as_str()).unwrap_or("/");
    let target_url = format!("{}{}", base_url, if req_path_and_query.starts_with('/') { req_path_and_query.to_string() } else { format!("/{}", req_path_and_query) });
    
    tracing::info!(target: "audit", profile = %state.profile, "Proxying {} request to: {}", parts.method, target_url);

    let req_path = parts.uri.path().to_string();

    // Helper for CORS-enabled error responses
    let cors_error = |status: axum::http::StatusCode, msg: String| {
        axum::response::Response::builder()
            .status(status)
            .header("Access-Control-Allow-Origin", "*")
            .body(axum::body::Body::from(msg))
            .unwrap()
    };

    // 1. Resolve Auth
    let fingerprint = match cowen_common::security::get_machine_fingerprint() {
        Ok(f) => f,
        Err(_) => return cors_error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Fingerprint failed".to_string())
    };

    let app_dir = cowen_common::config::get_app_dir();
    let cfg_mgr: cowen_config::ConfigManager = match cowen_config::ConfigManager::new() {
        Ok(m) => m,
        Err(_) => return cors_error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Config manager failed".to_string())
    };
    let app_config = match cfg_mgr.load_app_config().await {
        Ok(c) => c,
        Err(_) => return cors_error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Config load failed".to_string())
    };
    let vault: std::sync::Arc<dyn cowen_common::vault::Vault> = match cowen_store::create_vault(&app_config, &app_dir, &fingerprint).await {
        Ok(v) => v,
        Err(_) => return cors_error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Vault unlock failed".to_string())
    };

    let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());

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
                .header("Access-Control-Allow-Origin", "*")
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
