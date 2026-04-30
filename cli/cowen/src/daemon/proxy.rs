use axum::{
    extract::{Request, State},
    response::IntoResponse,
    routing::any,
    Router,
};
use reqwest::Client;
use std::net::SocketAddr;
use crate::core::config::Config;
use anyhow::Result;



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
) -> Result<()> {
    let state = ProxyState {
        client: Client::new(),
        config: config.clone(),
        profile: profile.to_string(),
    };

    let app = Router::new()
        .route("/{*path}", any(handle_proxy))
        .route("/", any(handle_proxy))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    crate::core::network::validate_loopback_addr(&addr)?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    tracing::info!(target: "sys", "Local Proxy Server listening on http://127.0.0.1:{}", port);
    
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_proxy(
    State(state): State<ProxyState>,
    req: Request,
) -> impl IntoResponse {
    // 2. Extract Parts
    let (parts, body) = req.into_parts();
    let target_url = format!("{}{}", state.config.openapi_url, parts.uri.path_and_query().map(|x| x.as_str()).unwrap_or(""));

    // 1. Resolve Auth & Spec
    let fingerprint = match crate::core::security::get_machine_fingerprint() {
        Ok(f) => f,
        Err(_) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Fingerprint failed").into_response()
    };

    let app_dir = crate::core::config::get_app_dir();
    let cfg_mgr = match crate::core::config::ConfigManager::new() {
        Ok(m) => m,
        Err(_) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Config manager failed").into_response()
    };
    let app_config = match cfg_mgr.load_app_config().await {
        Ok(c) => c,
        Err(_) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Config load failed").into_response()
    };
    let vault = match crate::core::vault::create_vault(&app_config, &app_dir, &fingerprint).await {
        Ok(v) => v,
        Err(_) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Vault unlock failed").into_response()
    };

    let auth_cli = crate::auth::create_auth_client_with_vault(vault.clone());
    use crate::auth::client::Client as AuthTrait; 

    let spec = match auth_cli.get_openapi_spec(&state.profile, &state.config, false).await {
        Ok(s) => s,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Spec error: {}", crate::core::utils::mask_sensitive_json(&e.to_string()))).into_response()
    };

    let req_path = parts.uri.path().to_string();
    let method_str = parts.method.to_string();

    if !crate::auth::client::is_path_in_whitelist(&req_path, &spec) {
        tracing::error!(target: "audit", profile = %state.profile, method = %method_str, path = %req_path, "Proxy Rejected: Path not in whitelist");
        return (
                axum::http::StatusCode::FORBIDDEN,
                format!("Proxy Rejected: Target path {} is not in the OpenAPI whitelist.", req_path),
        ).into_response();
    }

    // Convert axum headers to reqwest headers for provider compatibility
    let mut headers = reqwest::header::HeaderMap::new();
    for (k, v) in parts.headers.iter() {
        if let Ok(name) = reqwest::header::HeaderName::from_bytes(k.as_str().as_bytes()) {
            if let Ok(val) = reqwest::header::HeaderValue::from_bytes(v.as_bytes()) {
                headers.insert(name, val);
            }
        }
    }

    // 1.5. Intercept Logic (Encapsulated in Provider)
    // Read body early to allow interception and re-use for forwarding
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(e) => return (
            axum::http::StatusCode::BAD_REQUEST,
            format!("Failed to read body: {}", e)
        ).into_response(),
    };

    let provider = auth_cli.provider(&state.config.app_mode);
    
    // 1. Pre-flight Interceptor (Token injection, header decoration, or short-circuit)
    let intercept_result = match provider.intercept_request(&state.profile, &state.config, &req_path, &method_str, headers, &bytes, &spec).await {
        Ok(res) => res,
        Err(e) => {
            let masked_err = crate::core::utils::mask_sensitive_json(&e.to_string());
            tracing::error!(target: "audit", profile = %state.profile, method = %method_str, path = %req_path, error = %masked_err, "Proxy pre-flight intercept failed");
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                format!("Pre-flight error: {}", masked_err),
            ).into_response();
        }
    };

    let reqwest_headers = match intercept_result {
        crate::auth::provider::ProxyRequestAction::Respond(json_resp) => {
            return (axum::http::StatusCode::OK, axum::Json(json_resp)).into_response();
        }
        crate::auth::provider::ProxyRequestAction::Forward { mut headers } => {
            // Some headers like Host might confuse the upstream server if we copy them directly
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

            builder.body(axum::body::Body::from(out_bytes)).unwrap_or_else(|_| (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to construct response"
            ).into_response())
        }
        Err(e) => {
            tracing::error!(target: "audit", profile = %state.profile, method = %method_str, path = %req_path, error = %e, "Proxy upstream request failed");
            (
                axum::http::StatusCode::BAD_GATEWAY,
                format!("Proxy upstream error: {}", e),
            ).into_response()
        }
    }
}
