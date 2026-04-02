use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use crate::daemon::dlq::DlqStore;
use std::sync::Arc;

#[derive(Clone)]
pub struct Forwarder {
    client: Client,
    dlq: Arc<DlqStore>,
    target_url: String,
}

impl Forwarder {
    pub fn new(dlq: Arc<DlqStore>, target_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            dlq,
            target_url: target_url.to_string(),
        }
    }

    pub async fn forward(&self, event: Value) {
        if self.target_url.is_empty() {
            tracing::warn!(target: "stream", "No webhook_target configured. Event dropped locally.");
            println!("⚠️ No webhook_target configured. Event dropped locally.");
            return;
        }

        let msg_id = event.get("id").and_then(|v| v.as_str()).unwrap_or("unknown_id").to_string();
        let msg_type = event.get("msgType").and_then(|v| v.as_str()).unwrap_or("UNKNOWN").to_string();
        let headers = event.get("headers").map(|v| v.to_string()).unwrap_or_else(|| "{}".to_string());
        let payload = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());

        tracing::info!(target: "stream", msg_id = %msg_id, msg_type = %msg_type, target = %self.target_url, "Forwarding event to webhook");
        println!("➡️ Forwarding event [{}] to {}...", msg_type, self.target_url);

        let resp = self.client.post(&self.target_url)
            .header("Content-Type", "application/json")
            .body(payload.clone())
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                tracing::info!(target: "stream", msg_id = %msg_id, status = %r.status(), "Event successfully forwarded");
                println!("✅ Successfully forwarded event [{}]", msg_id);
            }
            Ok(r) => {
                let err_msg = format!("HTTP error: {}", r.status());
                tracing::error!(target: "stream", msg_id = %msg_id, status = %r.status(), "Forward failed, saving to DLQ");
                println!("❌ Forward failed: {}", err_msg);
                let _ = self.dlq.save(&msg_id, &msg_type, &payload, &headers, &err_msg);
            }
            Err(e) => {
                let err_msg = format!("Network error: {}", e);
                tracing::error!(target: "stream", msg_id = %msg_id, error = %e, "Forward network failed, saving to DLQ");
                println!("❌ Forward network failed: {}", err_msg);
                let _ = self.dlq.save(&msg_id, &msg_type, &payload, &headers, &err_msg);
            }
        }
    }
}
