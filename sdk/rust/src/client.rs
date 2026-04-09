use crate::protocol::{EventFrame, AckFrame};
use crate::crypto;
use crate::dispatcher::MessageDispatcher;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt, stream::SplitSink};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio::time::{sleep, Duration};
use anyhow::{Result, anyhow};
use url::Url;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ClientOptions {
    pub app_key: String,
    pub app_secret: String,
    pub encrypt_key: Option<String>,
    pub gateway_url: String,
}

pub struct GatewayClient {
    options: Arc<ClientOptions>,
    client_id: String,
    dispatcher: Arc<Mutex<MessageDispatcher>>,
    running: Arc<Mutex<bool>>,
    stop_tx: broadcast::Sender<()>,
}

impl GatewayClient {
    pub fn new(options: ClientOptions) -> Self {
        let hostname = hostname::get().unwrap_or_default().to_string_lossy().to_string();
        let pid = std::process::id();
        let random: u32 = rand::random::<u32>() % 1_000_000;
        let client_id = format!("{}@{}_{}_{}", options.app_key, hostname, pid, random);

        let (stop_tx, _) = broadcast::channel(1);

        Self {
            options: Arc::new(options),
            client_id,
            dispatcher: Arc::new(Mutex::new(MessageDispatcher::new())),
            running: Arc::new(Mutex::new(false)),
            stop_tx,
        }
    }

    pub fn dispatcher(&self) -> Arc<Mutex<MessageDispatcher>> {
        self.dispatcher.clone()
    }

    pub async fn start(&self) -> Result<()> {
        {
            let mut running = self.running.lock().unwrap();
            if *running {
                return Ok(());
            }
            *running = true;
        }

        let options = self.options.clone();
        let client_id = self.client_id.clone();
        let dispatcher = self.dispatcher.clone();
        let mut stop_rx = self.stop_tx.subscribe();
        let running_flag = self.running.clone();

        let mut attempt = 0;
        loop {
            tokio::select! {
                _ = stop_rx.recv() => {
                    tracing::info!("GatewayClient stopping...");
                    break;
                }
                res = Self::connect_and_loop(&options, &client_id, &dispatcher) => {
                    if let Err(e) = res {
                        tracing::error!("Connection error: {}", e);
                        let delay = Self::calculate_backoff(attempt);
                        tracing::info!("Reconnecting in {:?}", delay);
                        sleep(delay).await;
                        attempt += 1;
                    } else {
                        attempt = 0;
                    }
                }
            }

            if !*running_flag.lock().unwrap() {
                break;
            }
        }

        Ok(())
    }

    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
        let _ = self.stop_tx.send(());
    }

    async fn connect_and_loop(
        options: &ClientOptions,
        client_id: &str,
        dispatcher: &Arc<Mutex<MessageDispatcher>>
    ) -> Result<()> {
        // 1. Fetch Nonce
        let nonce = Self::fetch_nonce(options).await?;

        // 2. Sign
        let sign = crypto::hmac_sha256(&format!("{}&{}", options.app_key, nonce), &options.app_secret);

        // 3. Connect WebSocket
        let mut ws_url_str = options.gateway_url.clone();
        if ws_url_str.starts_with("http://") {
            ws_url_str = ws_url_str.replace("http://", "ws://");
        } else if ws_url_str.starts_with("https://") {
            ws_url_str = ws_url_str.replace("https://", "wss://");
        }

        let full_url = format!(
            "{}/connect?app_key={}&nonce={}&sign={}&client_id={}",
            ws_url_str, options.app_key, nonce, sign, client_id
        );

        let url = Url::parse(&full_url)?;
        let (ws_stream, _) = connect_async(url).await
            .map_err(|e| anyhow!("WebSocket connect failed: {}", e))?;

        tracing::info!("WebSocket connected.");

        let (mut write, mut read) = ws_stream.split();
        let encrypt_key = options.encrypt_key.clone().unwrap_or_else(|| options.app_secret.clone());

        loop {
            // 每 10 秒服务端发送一次 ping，如果 25 秒没有收到任何消息（包括 ping），则判定连接已死
            let next_msg = tokio::time::timeout(Duration::from_secs(25), read.next()).await;

            match next_msg {
                Ok(Some(msg)) => {
                    match msg {
                        Ok(Message::Text(text)) => {
                            let root: serde_json::Value = match serde_json::from_str(&text) {
                                Ok(v) => v,
                                Err(e) => {
                                    tracing::error!("Failed to parse incoming WebSocket message as JSON: {}. Raw: {}", e, text);
                                    continue;
                                }
                            };
                            let msg_type = root.get("msg_type").and_then(|v| v.as_str()).unwrap_or("").to_string();

                            if msg_type == "event" {
                                let frame: EventFrame = match serde_json::from_str(&text) {
                                    Ok(f) => f,
                                    Err(e) => {
                                        tracing::error!("Failed to parse EventFrame: {}. Raw: {}", e, text);
                                        continue;
                                    }
                                };
                                
                                let success = {
                                    let dispatcher_lock = dispatcher.lock().unwrap();
                                    match dispatcher_lock.dispatch(&frame, &encrypt_key) {
                                        Ok(s) => s,
                                        Err(e) => {
                                            tracing::error!("Error dispatching event {}: {}", frame.msg_id, e);
                                            false
                                        }
                                    }
                                };
                                Self::send_ack(&mut write, frame.msg_id, success).await?;
                            } else if msg_type == "ping" {
                                write.send(Message::Text("{\"msg_type\":\"pong\"}".to_string())).await?;
                            } else if !msg_type.is_empty() {
                                // Handle top-level system messages (e.g. APP_TICKET)
                                let msg_id = root.get("msg_id").or_else(|| root.get("msgId")).and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                                
                                let success = {
                                    let dispatcher_lock = dispatcher.lock().unwrap();
                                    match dispatcher_lock.dispatch_value(root, None) {
                                        Ok(s) => s,
                                        Err(e) => {
                                            tracing::error!("Error dispatching raw message type {}: {}", msg_type, e);
                                            false
                                        }
                                    }
                                };

                                if msg_id != "unknown" {
                                    Self::send_ack(&mut write, msg_id, success).await?;
                                }
                            }
                        }
                        Ok(Message::Close(_)) => {
                            tracing::info!("WebSocket closed by server.");
                            break;
                        }
                        Err(e) => {
                            return Err(anyhow!("WebSocket read error: {}", e));
                        }
                        _ => {}
                    }
                }
                Ok(None) => {
                    tracing::info!("WebSocket stream ended.");
                    break;
                }
                Err(_) => {
                    // Timeout occurred
                    return Err(anyhow!("WebSocket read timeout (no heartbeats from server for 25s). Triggering reconnect."));
                }
            }
        }

        Ok(())
    }

    async fn fetch_nonce(options: &ClientOptions) -> Result<String> {
        let mut base_url = options.gateway_url.clone();
        if base_url.starts_with("ws://") {
            base_url = base_url.replace("ws://", "http://");
        } else if base_url.starts_with("wss://") {
            base_url = base_url.replace("wss://", "https://");
        }

        let url = format!("{}/v1/ws/challenge?app_key={}", base_url, options.app_key);
        let sign_prefix = &crypto::hmac_sha256(&options.app_key, &options.app_secret)[..16];

        let client = reqwest::Client::new();
        let resp = client.get(&url)
            .header("X-CJT-PreAuth", sign_prefix)
            .header("User-Agent", "cjtCli-Rust-SDK/0.1.0")
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_else(|_| "Unknown".to_string());
            tracing::error!("Nonce request failed (HTTP {}): {}", status, body_text);
            return Err(anyhow!("Nonce request failed: {} - {}", status, body_text));
        }

        let body: serde_json::Value = resp.json().await?;
        let nonce = body.get("data").and_then(|d| d.get("nonce")).and_then(|n| n.as_str())
            .ok_or_else(|| anyhow!("Invalid nonce response"))?;

        Ok(nonce.to_string())
    }

    async fn send_ack(
        write: &mut SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>,
        msg_id: String,
        success: bool
    ) -> Result<()> {
        let code = if success { 200 } else { 500 };
        let message = if success { "success" } else { "failed" };
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;

        let ack = AckFrame {
            msg_id,
            code,
            message: message.to_string(),
            timestamp,
        };

        write.send(Message::Text(serde_json::to_string(&ack)?)).await
            .map_err(|e| anyhow!("Failed to send ACK: {}", e))
    }

    fn calculate_backoff(attempt: u32) -> Duration {
        let sec = (2u64.pow(attempt.min(6))).min(60);
        Duration::from_secs(sec)
    }
}
