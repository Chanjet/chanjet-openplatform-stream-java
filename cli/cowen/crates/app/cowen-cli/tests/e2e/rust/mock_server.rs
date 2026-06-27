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
}

pub type SharedState = Arc<Mutex<MockState>>;

pub async fn spawn_mock_server() -> (u16, SharedState) {
    let state = Arc::new(Mutex::new(MockState {
        active_ws: HashMap::new(),
        received_webhooks: Vec::new(),
        webhook_delay_ms: 0,
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
        // Webhooks
        .route("/webhook_sink", post(webhook_sink_handler))
        .route("/control/broadcast", post(broadcast_handler))
        .route("/control/webhooks", get(get_webhooks_handler))
        .route("/control/config", post(config_handler))
        .route("/control/kill_connections", post(kill_connections_handler))
        .route("/v1/mock/secure", get(mock_secure_handler))
        .route("/v1/mock/ping", get(mock_secure_handler))
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
    let org_id = temp_code.strip_prefix("code_").unwrap_or("900000000");

    Json(json!({
        "result": {
            "appName": "MockStoreApp",
            "appId": "12345",
            "permanentAuthCode": format!("mock_opc_{}", temp_code),
            "orgId": org_id
        },
        "code": "200",
        "message": "success"
    }))
}

async fn generate_token_oauth2() -> impl IntoResponse {
    let ts = chrono::Utc::now().timestamp_millis();
    Json(json!({
        "code": "200",
        "result": true,
        "value": {
            "access_token": format!("mock_at_oa2_{}", ts),
            "refresh_token": format!("mock_rt_oa2_{}", ts),
            "expires_in": 7200,
            "refresh_token_expires_in": 604800
        }
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
    ws.on_upgrade(move |socket| handle_socket(socket, state, client_id))
}

async fn handle_socket(mut socket: WebSocket, state: SharedState, client_id: String) {
    let (tx, mut rx) = mpsc::unbounded_channel();
    {
        let mut st = state.lock().unwrap();
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
    let delay = {
        let mut st = state.lock().unwrap();
        st.received_webhooks.push(payload);
        st.webhook_delay_ms
    };
    if delay > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    }
    Json(json!({"code": "200", "message": "success"}))
}

async fn broadcast_handler(
    State(state): State<SharedState>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let msg_str = serde_json::to_string(&payload).unwrap();
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
    if let Some(delay) = payload.get("webhook_delay_ms").and_then(|v| v.as_u64()) {
        let mut st = state.lock().unwrap();
        st.webhook_delay_ms = delay;
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

async fn mock_secure_handler(
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    println!("mock_secure_handler received headers: {:?}", headers);
    println!("mock_secure_handler received params: {:?}", params);

    let auth_header = headers.get("Authorization").and_then(|v| v.to_str().ok());
    let has_auth = auth_header.is_some()
        || params.contains_key("access_token")
        || params.contains_key("appTicket")
        || headers.contains_key("appKey");

    if has_auth {
        Json(json!({"status": "verified", "auth_injected": true}))
    } else {
        Json(json!({"status": "unauthorized", "auth_injected": false}))
    }
}
