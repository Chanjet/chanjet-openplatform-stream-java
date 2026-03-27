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
    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    println!("🛡️  Local Proxy Server listening on http://127.0.0.1:{}", port);
    
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_proxy(
    State(state): State<ProxyState>,
    req: Request,
) -> impl IntoResponse {
    let path = req.uri().path_and_query().map(|x| x.as_str()).unwrap_or("");
    let target_url = format!("{}{}", state.config.openapi_url, path);

    // 1. Resolve Auth & Spec
    let fingerprint = match crate::core::security::get_machine_fingerprint() {
        Ok(f) => f,
        Err(_) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Fingerprint failed").into_response()
    };

    let home = directories::UserDirs::new().unwrap().home_dir().to_path_buf();
    let seal_path = home.join(".cjtc").join(".seal");
    let vault = match crate::core::vault::MultiVault::new(seal_path, &fingerprint) {
        Ok(v) => v,
        Err(_) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Vault unlock failed").into_response()
    };

    let pool = crate::auth::VaultTokenPool::new(&vault);
    let auth_cli = crate::auth::AuthClient::new(&pool);
    use crate::auth::client::Client as AuthTrait; // This import was inside the old match, moved here for scope.

    let spec = match auth_cli.get_openapi_spec(&state.profile, &state.config).await {
        Ok(s) => s,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Spec error: {}", e)).into_response()
    };

    let req_path = req.uri().path();
    if !crate::auth::client::is_path_in_whitelist(req_path, &spec) {
        return (
                axum::http::StatusCode::FORBIDDEN,
                format!("Proxy Rejected: Target path {} is not in the OpenAPI whitelist.", req_path),
        ).into_response();
    }

    let token = match auth_cli.get_app_access_token(&state.profile, &state.config).await {
        Ok(t) => t,
        Err(e) => return (
            axum::http::StatusCode::UNAUTHORIZED,
            format!("Failed to get token: {}", e),
        ).into_response()
    };

    let method_str = req.method().as_str();
    let op = crate::auth::client::get_operation(&spec, req_path, method_str);

    // 2. Extract Parts
    let (mut parts, body) = req.into_parts();
    
    // Inject auth
    if let Ok(auth_header) = format!("Bearer {}", token.value).parse() {
        parts.headers.insert("Authorization", auth_header);
    }

    if let Some(operation) = op {
        if let Some(params) = operation.get("parameters").and_then(|p| p.as_array()) {
            for param in params.iter() {
                let name_str = param.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let p_in = param.get("in").and_then(|i| i.as_str()).unwrap_or("");
                if p_in == "header" {
                    match name_str {
                        "appKey" => {
                             if let Ok(hv) = axum::http::HeaderValue::from_str(&state.config.app_key) {
                                if let Ok(hn) = axum::http::HeaderName::from_bytes(b"appKey") {
                                    parts.headers.insert(hn, hv);
                                }
                             }
                        }
                        "appSecret" => {
                             if let Ok(hv) = axum::http::HeaderValue::from_str(&state.config.app_secret) {
                                if let Ok(hn) = axum::http::HeaderName::from_bytes(b"appSecret") {
                                    parts.headers.insert(hn, hv);
                                }
                             }
                        }
                        "openToken" => {
                             if let Ok(hv) = axum::http::HeaderValue::from_str(&token.value) {
                                if let Ok(hn) = axum::http::HeaderName::from_bytes(b"openToken") {
                                    parts.headers.insert(hn, hv);
                                }
                             }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    
    // Some headers like Host might confuse the upstream server if we copy them directly
    parts.headers.remove("host");

    // Extract body bytes
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(e) => return (
            axum::http::StatusCode::BAD_REQUEST,
            format!("Failed to read body: {}", e)
        ).into_response(),
    };

    // Convert axum headers to reqwest headers manually due to crate version mismatch (http v0.2 vs v1.0)
    let mut reqwest_headers = reqwest::header::HeaderMap::new();
    for (k, v) in parts.headers.iter() {
        if let Ok(name) = reqwest::header::HeaderName::from_bytes(k.as_str().as_bytes()) {
            if let Ok(val) = reqwest::header::HeaderValue::from_bytes(v.as_bytes()) {
                reqwest_headers.insert(name, val);
            }
        }
    }

    let method = reqwest::Method::from_bytes(parts.method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET);

    // 3. Forward
    let fwd_req = state.client.request(method, target_url)
        .headers(reqwest_headers)
        .body(reqwest::Body::from(bytes));

    let res = fwd_req.send().await;

    // 4. Respond
    match res {
        Ok(r) => {
            let status = axum::http::StatusCode::from_u16(r.status().as_u16()).unwrap_or(axum::http::StatusCode::OK);
            let mut builder = axum::response::Response::builder().status(status);
            for (k, v) in r.headers() {
                if let Ok(name) = axum::http::HeaderName::from_bytes(k.as_str().as_bytes()) {
                    if let Ok(val) = axum::http::HeaderValue::from_bytes(v.as_bytes()) {
                        builder = builder.header(name, val);
                    }
                }
            }
            let out_bytes = r.bytes().await.unwrap_or_default();
            builder.body(axum::body::Body::from(out_bytes)).unwrap_or_else(|_| (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to construct response"
            ).into_response())
        }
        Err(e) => (
            axum::http::StatusCode::BAD_GATEWAY,
            format!("Proxy upstream error: {}", e),
        ).into_response()
    }
}
