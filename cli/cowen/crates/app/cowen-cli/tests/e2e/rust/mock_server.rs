use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

#[derive(Default)]
pub struct MockState {
    pub active_ws: HashMap<String, mpsc::UnboundedSender<String>>,
    pub received_webhooks: Vec<serde_json::Value>,
    pub webhook_delay_ms: u64,
    pub webhook_sink_status: u16,
}

pub type SharedState = Arc<Mutex<MockState>>;

pub async fn spawn_mock_server() -> (u16, SharedState) {
    let state = Arc::new(Mutex::new(MockState {
        active_ws: HashMap::new(),
        received_webhooks: Vec::new(),
        webhook_delay_ms: 0,
        webhook_sink_status: 200,
    }));

    let app = Router::new()
        .route(
            "/v1/common/auth/selfBuiltApp/generateToken",
            post(generate_token_self_built),
        )
        .route(
            "/developer/api/apiPermissions/isv/open/getInterfaceList",
            get(get_interface_list),
        )
        .route("/v1/ws/challenge", get(generate_nonce))
        .route("/connect/v1/ws/challenge", get(generate_nonce))
        .route("/connect", get(ws_handler))
        .route("/auth/appTicket/resend", post(resend_app_ticket))
        // OAuth2 routes
        .route("/v1/common/auth/oauth2/token", post(generate_token_oauth2))
        .route("/oauth2/token", post(generate_token_oauth2))
        // Store App routes
        .route(
            "/auth/appAuth/getAppAccessToken",
            post(generate_token_store_app),
        )
        .route(
            "/auth/orgAuth/getPermanentAuthCode",
            post(get_permanent_auth_code),
        )
        .route(
            "/auth/orgAuth/getOrgAccessToken",
            post(get_org_access_token),
        )
        .route(
            "/auth/userAuth/getUserAccessToken",
            post(get_user_access_token),
        )
        // Webhooks
        .route("/webhook_sink", post(webhook_sink_handler))
        .route("/control/broadcast", post(broadcast_handler))
        .route("/control/webhooks", get(get_webhooks_handler))
        .route("/control/config", post(config_handler))
        .route("/control/kill_connections", post(kill_connections_handler))
        .route("/control/connection_count", get(connection_count_handler))
        .route("/v1/mock/secure", axum::routing::any(mock_secure_handler))
        .route("/v1/mock/ping", get(mock_secure_handler))
        .route("/v1/app/data/get", post(handle_generic_success))
        .route("/v1/app/data/save", post(handle_generic_success))
        .with_state(state.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (port, state)
}

async fn generate_token_self_built() -> impl IntoResponse {
    let ts = chrono::Utc::now().timestamp_millis();
    Json(json!({
        "code": "200",
        "result": true,
        "value": {
            "access_token": format!("mock_at_sb_{}", ts),
            "accessToken": format!("mock_at_sb_{}", ts),
            "expiresIn": 3600
        }
    }))
}

async fn generate_token_store_app() -> impl IntoResponse {
    let ts = chrono::Utc::now().timestamp_millis();
    Json(json!({
        "code": "200",
        "result": {
            "app_access_token": format!("mock_at_sa_{}", ts),
            "appAccessToken": format!("mock_at_sa_{}", ts),
            "expiresIn": 3600
        }
    }))
}

async fn get_permanent_auth_code(Json(payload): Json<serde_json::Value>) -> impl IntoResponse {
    let temp_code = payload
        .get("tempAuthCode")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let org_id = if temp_code.starts_with("code_") {
        temp_code.replace("code_", "")
    } else {
        "900000000".to_string()
    };

    Json(json!({
        "result": {
            "appName": "MockStoreApp",
            "appId": "12345",
            "permanentAuthCode": format!("mock_opc_{}", org_id),
            "orgId": org_id
        },
        "code": "200",
        "message": "success"
    }))
}

async fn get_org_access_token(Json(payload): Json<serde_json::Value>) -> impl IntoResponse {
    let opc = payload
        .get("permanentAuthCode")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    Json(json!({
        "result": {
            "accessToken": format!("mock_at_oa2_{}", opc),
            "expireTime": 7200
        },
        "code": "200",
        "message": "success"
    }))
}

async fn get_user_access_token(Json(payload): Json<serde_json::Value>) -> impl IntoResponse {
    let upc = payload
        .get("userPermanentCode")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    Json(json!({
        "result": {
            "accessToken": format!("mock_at_user_{}", upc),
            "expireTime": 7200
        },
        "code": "200",
        "message": "success"
    }))
}

async fn generate_token_oauth2() -> impl IntoResponse {
    let ts = chrono::Utc::now().timestamp_millis();
    let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1c2VySWQiOiAibW9ja191c2VyXzEyMyIsICJvcmdJZCI6ICJtb2NrX29yZ180NTYiLCAiYXBwSWQiOiAibW9ja19hcHBfNzg5In0.fakesignature";
    Json(json!({
        "access_token": jwt,
        "refresh_token": format!("mock_rt_oa2_{}", ts),
        "expires_in": 7200,
        "refresh_expires_in": 604800,
        "permanent_auth_code": "mock_opc_from_exchange",
        "user_auth_permanent_code": "mock_upc_from_exchange"
    }))
}

async fn get_interface_list() -> impl IntoResponse {
    Json(json!({
        "result": true,
        "value": {
            "currentPage": 0,
            "totalPages": 1,
            "resultList": [
                {
                    "requestPath": "/webhook_sink",
                    "interfaceName": "Webhook Sink",
                },
                {
                    "requestPath": "/v1/mock/secure",
                    "interfaceName": "Mock Secure",
                },
                {
                    "requestPath": "/v1/mock/ping",
                    "interfaceName": "Mock Ping",
                }
            ]
        }
    }))
}

async fn generate_nonce() -> impl IntoResponse {
    Json(json!({"data": {"nonce": "mock_nonce_123"}}))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let client_id = params
        .get("client_id")
        .cloned()
        .unwrap_or_else(|| "default".to_string());

    let is_exclusive = params
        .get("exclusive")
        .map(|v| v == "true")
        .unwrap_or(false);

    ws.on_upgrade(move |socket| handle_socket(socket, state, client_id, is_exclusive))
}

async fn handle_socket(
    mut socket: WebSocket,
    state: SharedState,
    client_id: String,
    is_exclusive: bool,
) {
    let (tx, mut rx) = mpsc::unbounded_channel();
    {
        let mut st = state.lock().unwrap();
        if is_exclusive {
            // Evict all other clients with the same app_key prefix
            let app_key = client_id.split('@').next().unwrap_or("unknown");
            let mut to_evict = Vec::new();
            for cid in st.active_ws.keys() {
                if cid.starts_with(&format!("{}@", app_key)) && cid != &client_id {
                    to_evict.push(cid.clone());
                }
            }
            for cid in to_evict {
                println!("🔪 [MOCK] Exclusive Eviction: AppKey {} requested exclusive access. Kicking client {}", app_key, cid);
                st.active_ws.remove(&cid); // dropping tx will cause rx to close, which closes socket
            }
        }
        st.active_ws.insert(client_id.clone(), tx);
    }

    loop {
        tokio::select! {
            msg = rx.recv() => {
                if let Some(text) = msg {
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                } else {
                    break;
                }
            }
            res = socket.recv() => {
                if let Some(Ok(msg)) = res {
                    if let Message::Text(text) = msg {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            if json.get("msg_type").and_then(|v| v.as_str()) == Some("ping") {
                                let _ = socket.send(Message::Text(serde_json::json!({"msg_type": "pong"}).to_string().into())).await;
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }
    }

    {
        let mut st = state.lock().unwrap();
        st.active_ws.remove(&client_id);
    }
}

async fn resend_app_ticket(
    headers: axum::http::HeaderMap,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    let app_key = headers
        .get("appKey")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let app_ticket_msg = json!({
        "msg_type": "APP_TICKET",
        "msgType": "APP_TICKET",
        "msgId": "mock_msg_id",
        "appKey": app_key,
        "time": "2026-06-26 12:00:00",
        "biz_content": {
            "app_ticket": "mock_ticket_push",
            "appTicket": "mock_ticket_push"
        },
        "bizContent": {
            "appTicket": "mock_ticket_push"
        }
    });

    let msg_str = serde_json::to_string(&app_ticket_msg).unwrap();

    let senders: Vec<_> = {
        let st = state.lock().unwrap();
        st.active_ws.values().cloned().collect()
    };

    for sender in senders {
        let _ = sender.send(msg_str.clone());
    }

    Json(json!({"code": "200", "message": "success"}))
}

async fn webhook_sink_handler(
    State(state): State<SharedState>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let (delay, status) = {
        let mut st = state.lock().unwrap();
        st.received_webhooks.push(payload);
        (st.webhook_delay_ms, st.webhook_sink_status)
    };
    if delay > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    }

    let mut resp = Json(json!({"code": "200", "message": "success"})).into_response();
    *resp.status_mut() =
        axum::http::StatusCode::from_u16(status).unwrap_or(axum::http::StatusCode::OK);
    resp
}

async fn broadcast_handler(
    State(state): State<SharedState>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let msg_str = serde_json::to_string(&payload).unwrap();
    println!("mock_server broadcast_handler received msg: {}", msg_str);
    let mode = payload
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("broadcast");

    let senders: Vec<_> = {
        let st = state.lock().unwrap();
        st.active_ws.values().cloned().collect()
    };

    if senders.is_empty() {
        return Json(json!({"code": "200", "message": "no connections"}));
    }

    if mode == "lb" {
        // Simple round robin or just pick the first one for mock purposes.
        // Actually, let's use the current time length or something to vary it,
        // or just pick random. Since we just need to deliver 1 copy per message.
        use std::time::SystemTime;
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos() as usize;
        let idx = nanos % senders.len();
        let _ = senders[idx].send(msg_str);
    } else {
        for sender in senders {
            let _ = sender.send(msg_str.clone());
        }
    }
    Json(json!({"code": "200", "message": "success"}))
}

async fn get_webhooks_handler(State(state): State<SharedState>) -> impl IntoResponse {
    let st = state.lock().unwrap();
    Json(st.received_webhooks.clone())
}

async fn config_handler(
    State(state): State<SharedState>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut st = state.lock().unwrap();
    if let Some(delay) = payload.get("webhook_delay_ms").and_then(|v| v.as_u64()) {
        st.webhook_delay_ms = delay;
    }
    if let Some(status) = payload.get("webhook_sink_status").and_then(|v| v.as_u64()) {
        st.webhook_sink_status = status as u16;
    }
    Json(json!({"code": "200", "message": "success"}))
}

async fn kill_connections_handler(State(state): State<SharedState>) -> impl IntoResponse {
    let mut st = state.lock().unwrap();
    // Clearing the map drops all the unbounded senders,
    // which effectively forces all active websocket handling loops to terminate.
    st.active_ws.clear();
    Json(json!({"code": "200", "message": "success"}))
}

async fn connection_count_handler(State(state): State<SharedState>) -> impl IntoResponse {
    let st = state.lock().unwrap();
    Json(json!({"count": st.active_ws.len()}))
}

async fn mock_secure_handler(
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    println!("mock_secure_handler received headers: {:?}", headers);
    println!("mock_secure_handler received params: {:?}", params);

    let auth_header = headers.get("Authorization").and_then(|v| v.to_str().ok());
    let opentoken = headers.get("opentoken").and_then(|v| v.to_str().ok());

    let has_auth = auth_header.is_some()
        || opentoken.is_some()
        || params.contains_key("access_token")
        || params.contains_key("appTicket")
        || headers.contains_key("appKey");

    let token_used = auth_header.or(opentoken).unwrap_or("");

    if has_auth {
        Json(json!({"status": "verified", "auth_injected": true, "token_used": token_used}))
    } else {
        Json(json!({"status": "unauthorized", "auth_injected": false}))
    }
}

async fn handle_generic_success(headers: axum::http::HeaderMap) -> impl IntoResponse {
    let open_token = headers
        .get("openToken")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let app_key = headers
        .get("appKey")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    Json(json!({
        "code": "200",
        "message": "success",
        "data": {
            "openToken": open_token,
            "appKey": app_key
        }
    }))
}
