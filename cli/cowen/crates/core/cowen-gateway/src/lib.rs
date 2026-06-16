// PRD v0.5.0 — Identity-Aware Gateway Server (Ingress Reverse Proxy)
//
// Implements the core gateway functionality:
// - OAuth code interception and exchange
// - Session validation and sliding window renewal
// - Auth routing enforcement
// - Reverse proxy to ISV backend with identity header injection

pub mod jwks;
pub mod routing;
pub mod session;

use crate::jwks::JwksManager;
use axum::extract::ConnectInfo;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, StatusCode, Uri},
    response::Response,
    routing::any,
    Router,
};
use cowen_auth::client::Client;
use cowen_common::vault::Vault;
use cowen_common::{
    config::{AppConfig, Config, GatewayConfig},
    CowenError, CowenResult,
};
use moka::sync::Cache;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::CorsLayer;

use self::{
    routing::{RequestType, RouteDecision, RouteMatcher},
    session::{build_set_cookie_header, extract_session_cookie, GatewayClaims, SessionManager},
};

/// Shared state for the gateway server.
#[derive(Clone)]
pub struct GatewayState {
    pub profile: String,
    pub config: Config,
    pub gateway_config: GatewayConfig,
    pub app_config: cowen_common::config::AppConfig,
    pub session_manager: Arc<SessionManager>,
    pub route_matcher: RouteMatcher,
    pub auth_client: Arc<dyn Client>,
    pub http_client: reqwest::Client,
    pub session_cache: Arc<Cache<String, GatewayClaims>>,
    pub vault: Arc<dyn Vault>,
}

/// Start the Identity-Aware Gateway server.
///
/// The gateway listens on the configured `bind_address` and:
/// 1. Intercepts OAuth `code` parameters → exchanges for token → creates session → 302 redirect
/// 2. Validates `cowen_sess_id` cookie → decrypts JWE → checks expiry
/// 3. Enforces auth routing rules (STRICT/PERMISSIVE)
/// 4. Injects identity headers (`x-org-id`, `x-user-id`, `x-app-id`)
/// 5. Reverse-proxies to `upstream_url`
/// 6. Applies sliding window renewal on response
pub async fn start_gateway(
    profile: &str,
    config: &Config,
    gateway_config: &GatewayConfig,
    app_config: &AppConfig,
    auth_client: Arc<dyn Client>,
    vault: Arc<dyn Vault>,
    port_tx: Option<tokio::sync::oneshot::Sender<u16>>,
) -> CowenResult<()> {
    let state = build_gateway_state(
        profile,
        config,
        gateway_config,
        app_config,
        auth_client,
        vault,
    )
    .await?;
    let addr = parse_bind_address(&gateway_config.bind_address)?;
    let listener = bind_listener_with_retry(addr, profile).await?;

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::mirror_request())
        .allow_credentials(true)
        .allow_methods(tower_http::cors::AllowMethods::mirror_request())
        .allow_headers(tower_http::cors::AllowHeaders::mirror_request());

    let app = Router::new()
        .fallback(any(handle_gateway))
        .layer(cors)
        .with_state(state);

    let actual_port = listener
        .local_addr()
        .map_err(|e| CowenError::Store(format!("Failed to get local addr: {}", e)))?
        .port();
    tracing::info!(
        target: "sys", profile = %profile,
        "Identity-Aware Gateway listening on http://{} → upstream: {}",
        listener.local_addr().unwrap(),
        gateway_config.upstream_url
    );

    if let Some(tx) = port_tx {
        let _ = tx.send(actual_port);
    }

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .map_err(|e| CowenError::Store(format!("Gateway server error: {}", e)))?;
    Ok(())
}

/// Build the shared gateway state from configuration.
async fn build_gateway_state(
    profile: &str,
    config: &Config,
    gateway_config: &GatewayConfig,
    app_config: &AppConfig,
    auth_client: Arc<dyn Client>,
    vault: Arc<dyn Vault>,
) -> CowenResult<GatewayState> {
    let jwks_manager = JwksManager::new(vault.clone(), profile).await?;

    let session_manager = SessionManager::new(std::sync::Arc::new(jwks_manager))
        .map_err(|e| CowenError::Internal(format!("Failed to create SessionManager: {}", e)))?;
    let route_matcher = RouteMatcher::new(gateway_config.auth_routing.clone());
    let http_client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .map_err(|e| CowenError::Internal(format!("Failed to create HTTP client: {}", e)))?;

    let session_cache = Cache::builder()
        .time_to_idle(Duration::from_secs(1800))
        .max_capacity(10_000)
        .build();

    Ok(GatewayState {
        profile: profile.to_string(),
        config: config.clone(),
        gateway_config: gateway_config.clone(),
        app_config: app_config.clone(),
        session_manager: Arc::new(session_manager),
        route_matcher,
        auth_client,
        http_client,
        session_cache: Arc::new(session_cache),
        vault,
    })
}

/// Parse the bind address string into a SocketAddr.
fn parse_bind_address(bind_address: &str) -> CowenResult<std::net::SocketAddr> {
    bind_address
        .parse::<std::net::SocketAddr>()
        .map_err(|e| CowenError::Config(format!("Invalid gateway bind address: {}", e)))
}

/// Bind a TCP listener with retry logic for port conflicts.
async fn bind_listener_with_retry(
    addr: std::net::SocketAddr,
    _profile: &str,
) -> CowenResult<tokio::net::TcpListener> {
    let mut retry_count = 0;
    loop {
        let socket = match addr {
            std::net::SocketAddr::V4(_) => tokio::net::TcpSocket::new_v4()
                .map_err(|e| CowenError::Store(format!("Failed to create socket: {}", e)))?,
            std::net::SocketAddr::V6(_) => tokio::net::TcpSocket::new_v6()
                .map_err(|e| CowenError::Store(format!("Failed to create socket: {}", e)))?,
        };
        let _ = cowen_sys::configure_socket_reuse(&socket);

        match socket.bind(addr) {
            Ok(_) => {
                return socket.listen(1024).map_err(|e| {
                    CowenError::Store(format!(
                        "Failed to listen on gateway port {}: {}",
                        addr.port(),
                        e
                    ))
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && retry_count < 10 => {
                tracing::warn!(target: "sys", "Gateway port {} in use, retrying... ({}/10)", addr.port(), retry_count + 1);
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                retry_count += 1;
            }
            Err(e) => {
                return Err(CowenError::Store(format!(
                    "Failed to bind gateway port {}: {}",
                    addr.port(),
                    e
                )))
            }
        }
    }
}

/// Main gateway request handler.
///
/// Flow:
/// 1. Check for OAuth `code` query parameter → exchange and redirect
/// 2. OPTIONS passthrough (CORS preflight)
/// 3. Validate session cookie
/// 4. Apply auth routing rules
/// 5. Inject identity headers and proxy to upstream

#[axum::debug_handler]
async fn handle_gateway(State(state): State<GatewayState>, req: Request) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path().to_string();

    tracing::debug!(
        target: "audit",
        profile = %state.profile,
        method = %method,
        path = %path,
        "Gateway received request"
    );

    // 1. Global code interception (highest priority)
    if let Some(code) = extract_code_param(&uri) {
        let fp = generate_fingerprint(&req);
        return handle_code_interception(&state, &uri, &code, &fp).await;
    }

    // 3. Session validation
    let fp = generate_fingerprint(&req);
    let cookie_value = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(extract_session_cookie);

    let session_claims = match extract_and_validate_session(&state, cookie_value, &fp).await {
        Ok(Some(claims)) => Some(claims),
        Ok(None) => None,
        Err(e) => {
            tracing::debug!(target: "audit", profile = %state.profile, "Session validation failed: {}", e);
            None
        }
    };

    // 4. Auth routing check
    let decision = state.route_matcher.classify(&path);
    let request_type = RouteMatcher::detect_request_type(req.headers());

    match (decision, &session_claims) {
        (RouteDecision::RequiresAuth, None) => {
            // No valid session for protected route
            return handle_unauthorized(&state, &request_type, &req);
        }
        (RouteDecision::BypassAuth, None) => {
            // Bypass route, no session — proxy without identity headers
            return proxy_to_upstream(&state, req, None).await;
        }
        (RouteDecision::RequiresAuth, Some(_)) | (RouteDecision::BypassAuth, Some(_)) => {
            // Valid session — inject headers and proxy
        }
    }

    // 5. Proxy with identity injection
    let response = proxy_to_upstream(&state, req, session_claims.as_ref()).await;

    // 6. Sliding window renewal
    if let Some(ref claims) = session_claims {
        if state.session_manager.needs_refresh(claims) {
            return inject_refresh_cookie(&state, response, claims).await;
        }
    }

    response
}

/// Generate fingerprint based on IP and User-Agent
pub fn generate_fingerprint(req: &Request<Body>) -> String {
    let ua = req
        .headers()
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let ip = req
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|c| c.0.ip().to_string())
        .unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(format!("{}|{}", ip, ua).as_bytes());
    hex::encode(hasher.finalize())
}

/// Extract the OAuth `code` query parameter from the URI.
fn extract_code_param(uri: &Uri) -> Option<String> {
    uri.query().and_then(|q| {
        q.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            if key == "code" && !value.is_empty() {
                Some(urlencoding::decode(value).ok()?.into_owned())
            } else {
                None
            }
        })
    })
}

fn parse_token_and_identity(
    json_val: &serde_json::Value,
) -> (
    cowen_common::models::Token,
    String,
    Option<String>,
    Option<String>,
) {
    let access_token = json_val
        .get("access_token")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let expires_in = json_val
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .unwrap_or(7200);

    let now = chrono::Utc::now();
    let token = cowen_common::models::Token {
        value: access_token,
        expires_at: now + chrono::Duration::seconds(expires_in),
        created_at: now,
    };

    let identity = token.extract_identity();
    let org_id = identity
        .as_ref()
        .map(|i| i.org_id.clone())
        .unwrap_or_default();
    let user_id = identity.as_ref().map(|i| i.user_id.clone());
    let app_id = identity.as_ref().map(|i| i.app_id.clone());

    (token, org_id, user_id, app_id)
}

/// Handle OAuth code interception: exchange code for token, create session, 302 redirect.
async fn handle_code_interception(
    state: &GatewayState,
    uri: &Uri,
    code: &str,
    fp: &str,
) -> Response {
    tracing::info!(
        target: "audit",
        profile = %state.profile,
        "Intercepted OAuth code, exchanging for token..."
    );

    // URL-encode the code and create the standard OAuth2 body
    let body = format!(
        "grant_type=authorization_code&code={}",
        urlencoding::encode(code)
    );

    // Exchange code for token using the auth provider's oauth2/token interception logic
    let exchange_result = state
        .auth_client
        .intercept_exchange(&state.profile, &state.config, body.as_bytes())
        .await;

    match exchange_result {
        Ok(json_val) => {
            // Reconstruct Token to extract identity
            let (token, org_id, user_id, app_id) = parse_token_and_identity(&json_val);

            // Create encrypted session
            let session_cookie = match state
                .session_manager
                .create_session(
                    org_id.clone(),
                    user_id.clone(),
                    app_id.clone(),
                    token.value.clone(),
                    fp.to_string(),
                )
                .await
            {
                Ok(cookie) => cookie,
                Err(e) => {
                    tracing::error!(target: "audit", profile = %state.profile, "Failed to create session: {}", e);
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to create session",
                    );
                }
            };

            // Store in LRU cache
            let uid_str = user_id.as_deref().unwrap_or("");
            let cache_key = hex::encode(Sha256::digest(format!("{}|{}", org_id, uid_str)));
            let claims = GatewayClaims::new(
                org_id.clone(),
                user_id.clone(),
                app_id.clone(),
                token.value.clone(),
                fp.to_string(),
                state.session_manager.idle_timeout_secs(),
                state.session_manager.absolute_timeout_secs(),
            );
            state.session_cache.insert(cache_key, claims);

            // Sync Hook
            let sync_hook_cookies =
                invoke_sync_hook(state, &org_id, user_id.as_deref(), app_id.as_deref()).await;

            // Build clean redirect URL (strip code parameter)
            let redirect_url = strip_code_param(uri);

            tracing::info!(
                target: "audit",
                profile = %state.profile,
                "Code exchange successful, redirecting to {}",
                redirect_url
            );

            let mut builder = Response::builder()
                .status(StatusCode::FOUND)
                .header(header::LOCATION, redirect_url)
                .header(header::SET_COOKIE, build_set_cookie_header(&session_cookie));

            for cookie in sync_hook_cookies {
                builder = builder.header(header::SET_COOKIE, cookie);
            }

            builder.body(Body::empty()).unwrap()
        }
        Err(e) => {
            tracing::error!(
                target: "audit",
                profile = %state.profile,
                "OAuth code exchange failed: {}",
                e
            );
            error_response(
                StatusCode::BAD_REQUEST,
                &format!("Code exchange failed: {}", e),
            )
        }
    }
}

async fn invoke_sync_hook(
    state: &GatewayState,
    org_id: &str,
    user_id: Option<&str>,
    app_id: Option<&str>,
) -> Vec<String> {
    let mut sync_hook_cookies = Vec::new();
    if let Some(ref hook_url) = state.gateway_config.auth_sync_hook {
        let mut retries = 0;
        let mut success = false;
        let payload = serde_json::json!({
            "org_id": org_id,
            "user_id": user_id.unwrap_or_default(),
            "app_id": app_id.unwrap_or_default()
        });
        while retries < 2 && !success {
            match state.http_client.post(hook_url).json(&payload).send().await {
                Ok(resp) if resp.status().is_success() => {
                    for cookie in resp.headers().get_all(reqwest::header::SET_COOKIE) {
                        if let Ok(s) = cookie.to_str() {
                            sync_hook_cookies.push(s.to_string());
                        }
                    }
                    success = true;
                }
                Ok(resp) => {
                    tracing::warn!(target: "audit", profile = %state.profile, "Sync hook returned non-200: {}", resp.status());
                }
                Err(e) => {
                    tracing::warn!(target: "audit", profile = %state.profile, "Sync hook failed: {}", e);
                }
            }
            if !success {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                retries += 1;
            }
        }
        if !success {
            tracing::warn!(target: "audit", profile = %state.profile, "Sync hook exhausted retries");
        }
    }
    sync_hook_cookies
}

/// Strip the `code` query parameter from a URI, preserving other parameters.
fn strip_code_param(uri: &Uri) -> String {
    let path = uri.path();
    if let Some(query) = uri.query() {
        let filtered: Vec<&str> = query
            .split('&')
            .filter(|pair| !pair.starts_with("code="))
            .collect();
        if filtered.is_empty() {
            path.to_string()
        } else {
            format!("{}?{}", path, filtered.join("&"))
        }
    } else {
        path.to_string()
    }
}

/// Extract and validate the session cookie from the request.
async fn extract_and_validate_session(
    state: &GatewayState,
    cookie_value: Option<String>,
    fp: &str,
) -> Result<Option<GatewayClaims>, String> {
    if let Some(cookie) = cookie_value {
        let claims = state.session_manager.validate_session(&cookie, fp).await?;
        return Ok(Some(claims));
    }

    Ok(None)
}

/// Handle unauthorized request based on request type.
///
/// - API request → 401 JSON with login_url
/// - Page request → 302 redirect to open platform login
fn handle_unauthorized(
    state: &GatewayState,
    request_type: &RequestType,
    req: &Request<Body>,
) -> Response {
    let uri = req.uri();
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");
    let scheme = req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("http");

    let full_requested_url = format!("{}://{}{}", scheme, host, uri);
    let encoded_redirect_uri = urlencoding::encode(&full_requested_url);

    let market_url = cowen_infra::obfs!(cowen_common::config::DEF_MARKET_URL);
    let oauth_authorize_url = format!("{}/user/v2/authorize", market_url.trim_end_matches('/'));

    match request_type {
        RequestType::Api => {
            let body = serde_json::json!({
                "error": "unauthorized",
                "message": "Valid session required",
                "login_url": format!("{}?client_id={}&response_type=code&redirect_uri={}", oauth_authorize_url, state.config.app_key, encoded_redirect_uri)
            });
            Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap()
        }
        RequestType::Page => {
            // Redirect to open platform login with state parameter
            let state_param = urlencoding::encode(&full_requested_url);
            let login_url = format!(
                "{}?client_id={}&response_type=code&redirect_uri={}&state={}",
                oauth_authorize_url, state.config.app_key, encoded_redirect_uri, state_param
            );
            Response::builder()
                .status(StatusCode::FOUND)
                .header(header::LOCATION, login_url)
                .body(Body::empty())
                .unwrap()
        }
    }
}

fn match_and_route(state: &GatewayState, path: &str) -> (bool, String, String) {
    let mut matched_route = None;
    for r in &state.gateway_config.routes {
        if routing::glob_match(&r.path, path) {
            matched_route = Some(r.clone());
            break;
        }
    }

    match matched_route {
        Some(ref route) => {
            let mut p = path.to_string();
            if let Some(ref prefix) = route.strip_prefix {
                if p.starts_with(prefix) {
                    p = p.replacen(prefix, "", 1);
                    if !p.starts_with('/') {
                        p = format!("/{}", p);
                    }
                }
            }
            if route.upstream == "openapi" {
                (true, state.app_config.openapi_url.clone(), p)
            } else {
                (false, route.upstream.clone(), p)
            }
        }
        None => {
            let is_direct = state.gateway_config.upstream_url == "openapi"
                || state.gateway_config.upstream_url == state.app_config.openapi_url;
            let target = if state.gateway_config.upstream_url == "openapi" {
                state.app_config.openapi_url.clone()
            } else {
                state.gateway_config.upstream_url.clone()
            };
            (is_direct, target, path.to_string())
        }
    }
}

fn create_request_builder(
    state: &GatewayState,
    method: &str,
    url: &str,
    headers: reqwest::header::HeaderMap,
    body: Vec<u8>,
) -> reqwest::RequestBuilder {
    let reqwest_method =
        reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET);
    let mut req_builder = state.http_client.request(reqwest_method, url);
    req_builder = req_builder.headers(headers);
    if !body.is_empty() {
        req_builder = req_builder.body(reqwest::Body::from(body));
    }
    req_builder
}

async fn send_and_build_response(
    state: &GatewayState,
    req_builder: reqwest::RequestBuilder,
    upstream_url: &str,
) -> Result<(StatusCode, reqwest::header::HeaderMap, axum::body::Bytes), Response> {
    match req_builder.send().await {
        Ok(upstream_resp) => {
            let status = StatusCode::from_u16(upstream_resp.status().as_u16())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let headers = upstream_resp.headers().clone();
            let body = upstream_resp.bytes().await.unwrap_or_default();
            Ok((status, headers, body))
        }
        Err(e) => {
            tracing::error!(
                target: "audit",
                profile = %state.profile,
                upstream = %upstream_url,
                "Upstream request failed: {}",
                e
            );
            Err(error_response(
                StatusCode::BAD_GATEWAY,
                &format!("Upstream error: {}", e),
            ))
        }
    }
}

fn build_axum_response(
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let mut response_builder = Response::builder().status(status);
    for (key, value) in headers.iter() {
        if let (Ok(name), Ok(val)) = (
            axum::http::HeaderName::from_bytes(key.as_str().as_bytes()),
            axum::http::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            response_builder = response_builder.header(name, val);
        }
    }
    response_builder.body(Body::from(body)).unwrap_or_else(|_| {
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "Response build failed")
    })
}

async fn handle_direct_openapi(
    state: &GatewayState,
    path: &str,
    method: &str,
    req_headers: reqwest::header::HeaderMap,
    body_bytes: &[u8],
    query: Option<&str>,
) -> Response {
    let auth_cli = cowen_auth::create_auth_client_with_vault(state.vault.clone());
    let provider = auth_cli.provider(&state.config.app_mode);

    let final_headers = match provider
        .intercept_request(
            &state.profile,
            &state.config,
            path,
            method,
            req_headers,
            body_bytes,
            &serde_json::Value::Null,
        )
        .await
    {
        Ok(cowen_auth::provider::ProxyRequestAction::Respond(json_resp)) => {
            let body_bytes = serde_json::to_vec(&json_resp).unwrap_or_default();
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body_bytes))
                .unwrap();
        }
        Ok(cowen_auth::provider::ProxyRequestAction::Forward { mut headers }) => {
            headers.remove(reqwest::header::HOST);
            headers
        }
        Err(e) => {
            let masked_err = cowen_common::utils::mask_sensitive_json(&e.to_string());
            tracing::error!(target: "audit", profile = %state.profile, method = %method, path = %path, error = %masked_err, "Direct OpenAPI intercept failed");
            return error_response(
                StatusCode::UNAUTHORIZED,
                &format!("Direct intercept error: {}", masked_err),
            );
        }
    };

    let upstream_url = format!(
        "{}{}{}",
        state.app_config.openapi_url.trim_end_matches('/'),
        path,
        query.map(|q| format!("?{}", q)).unwrap_or_default()
    );

    let req_builder = create_request_builder(
        state,
        method,
        &upstream_url,
        final_headers,
        body_bytes.to_vec(),
    );

    match send_and_build_response(state, req_builder, &upstream_url).await {
        Ok((status, out_headers, resp_body)) => {
            if let Err(e) = provider
                .intercept_response(
                    &state.profile,
                    &state.config,
                    path,
                    method,
                    status.as_u16(),
                    &out_headers,
                    &resp_body,
                )
                .await
            {
                tracing::warn!(target: "audit", profile = %state.profile, error = %e, "Direct post-flight intercept failed (non-fatal)");
            }
            build_axum_response(status, &out_headers, resp_body)
        }
        Err(error_resp) => error_resp,
    }
}

async fn handle_normal_upstream(
    state: &GatewayState,
    target_upstream: &str,
    path: &str,
    query: Option<&str>,
    req_headers: reqwest::header::HeaderMap,
    body_bytes: Vec<u8>,
    method: &str,
) -> Response {
    let upstream_url = format!(
        "{}{}{}",
        target_upstream.trim_end_matches('/'),
        path,
        query.map(|q| format!("?{}", q)).unwrap_or_default()
    );

    let req_builder = create_request_builder(state, method, &upstream_url, req_headers, body_bytes);

    match send_and_build_response(state, req_builder, &upstream_url).await {
        Ok((status, out_headers, resp_body)) => {
            build_axum_response(status, &out_headers, resp_body)
        }
        Err(error_resp) => error_resp,
    }
}

/// Reverse proxy the request to the upstream ISV backend.
async fn proxy_to_upstream(
    state: &GatewayState,
    req: Request,
    claims: Option<&GatewayClaims>,
) -> Response {
    let (parts, body) = req.into_parts();
    let path = parts.uri.path();
    let query = parts.uri.query();

    // 1. Evaluate custom route rules for upstream matching and prefix stripping
    let (is_direct_openapi, target_upstream, final_path) = match_and_route(state, path);

    // 2. Prepare headers (copy incoming request headers, except Host)
    let mut req_headers = reqwest::header::HeaderMap::new();
    for (key, value) in parts.headers.iter() {
        if key != header::HOST {
            if let Ok(name) = reqwest::header::HeaderName::from_bytes(key.as_str().as_bytes()) {
                if let Ok(val) = reqwest::header::HeaderValue::from_bytes(value.as_bytes()) {
                    req_headers.insert(name, val);
                }
            }
        }
    }

    // Inject identity headers if session is valid
    if let Some(c) = claims {
        if let Ok(v) = c.org_id.parse() {
            req_headers.insert("x-org-id", v);
        }
        if let Some(ref uid) = c.user_id {
            if let Ok(v) = uid.parse() {
                req_headers.insert("x-user-id", v);
            }
        }
        if let Some(ref aid) = c.app_id {
            if let Ok(v) = aid.parse() {
                req_headers.insert("x-app-id", v);
            }
        }
    }

    // Forward body
    let body_bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .unwrap_or_default();

    if is_direct_openapi {
        handle_direct_openapi(
            state,
            &final_path,
            parts.method.as_str(),
            req_headers,
            &body_bytes,
            query,
        )
        .await
    } else {
        handle_normal_upstream(
            state,
            &target_upstream,
            &final_path,
            query,
            req_headers,
            body_bytes.to_vec(),
            parts.method.as_str(),
        )
        .await
    }
}

/// Inject a refreshed session cookie into the response.
async fn inject_refresh_cookie(
    state: &GatewayState,
    mut response: Response,
    claims: &GatewayClaims,
) -> Response {
    match state.session_manager.refresh_session(claims).await {
        Ok(new_cookie) => {
            response.headers_mut().insert(
                header::SET_COOKIE,
                build_set_cookie_header(&new_cookie).parse().unwrap(),
            );

            // Also update LRU Cache
            let uid_str = claims.user_id.as_deref().unwrap_or("");
            let cache_key = hex::encode(Sha256::digest(format!("{}|{}", claims.org_id, uid_str)));
            let refreshed = claims.refresh_idle(state.session_manager.idle_timeout_secs());
            state.session_cache.insert(cache_key, refreshed);

            tracing::debug!(
                target: "audit",
                profile = %state.profile,
                "Session refreshed via sliding window"
            );
        }
        Err(e) => {
            tracing::warn!(
                target: "audit",
                profile = %state.profile,
                "Failed to refresh session: {}",
                e
            );
        }
    }
    response
}

/// Build a simple error response.
fn error_response(status: StatusCode, message: &str) -> Response {
    let body = serde_json::json!({ "error": message });
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_code_param_present() {
        let uri: Uri = "http://example.com/invoice?code=abc123&other=val"
            .parse()
            .unwrap();
        assert_eq!(extract_code_param(&uri), Some("abc123".to_string()));
    }

    #[test]
    fn test_extract_code_param_absent() {
        let uri: Uri = "http://example.com/invoice?other=val".parse().unwrap();
        assert_eq!(extract_code_param(&uri), None);
    }

    #[test]
    fn test_extract_code_param_empty_value() {
        let uri: Uri = "http://example.com/invoice?code=&other=val"
            .parse()
            .unwrap();
        assert_eq!(extract_code_param(&uri), None);
    }

    #[test]
    fn test_extract_code_param_url_encoded() {
        let uri: Uri = "http://example.com/callback?code=abc%2Bdef"
            .parse()
            .unwrap();
        assert_eq!(extract_code_param(&uri), Some("abc+def".to_string()));
    }

    #[test]
    fn test_strip_code_param_only_code() {
        let uri: Uri = "http://example.com/invoice?code=abc123".parse().unwrap();
        assert_eq!(strip_code_param(&uri), "/invoice");
    }

    #[test]
    fn test_strip_code_param_with_other_params() {
        let uri: Uri = "http://example.com/invoice?foo=bar&code=abc123&baz=qux"
            .parse()
            .unwrap();
        assert_eq!(strip_code_param(&uri), "/invoice?foo=bar&baz=qux");
    }

    #[tokio::test]
    #[ignore]
    async fn test_strip_code_param_no_query() {
        let uri: Uri = "http://example.com/invoice".parse().unwrap();
        assert_eq!(strip_code_param(&uri), "/invoice");
    }
}
