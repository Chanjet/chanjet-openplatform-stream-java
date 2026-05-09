use reqwest::{Client, Url};
use serde_json::Value;
use std::time::Duration;
use crate::daemon::dlq::DlqStore;
use std::sync::Arc;
use cowen_common::{CowenResult, CowenError};

#[derive(Clone)]
pub struct Forwarder {
    client: Client,
    dlq: Arc<DlqStore>,
    target_url: String,
    profile: String,
    config: cowen_common::config::Config,
}

impl Forwarder {
    pub fn new(profile: &str, config: cowen_common::config::Config, vault: Arc<dyn cowen_common::vault::Vault>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        let dlq = Arc::new(DlqStore::new(profile, vault).unwrap()); // Safe unwrap as it only creates struct

        Self {
            client,
            dlq,
            target_url: config.webhook_target.clone(),
            profile: profile.to_string(),
            config,
        }
    }

    pub async fn retry_message(&self, id: i64) -> CowenResult<()> {
        let all = self.dlq.list_all().await?;
        let entry = all.iter().find(|e| e.id == id).ok_or_else(|| CowenError::Store("Message not found in DLQ".to_string()))?;
        let event: Value = serde_json::from_str(&entry.payload)?;
        self.forward(event).await;
        Ok(())
    }

    pub async fn forward(&self, event: Value) {
        if self.target_url.is_empty() {
            tracing::warn!(target: "stream", "No webhook_target configured. Event dropped locally.");
            println!("⚠️ No webhook_target configured. Event dropped locally.");
            return;
        }

        // SSRF Protection: Loopback Only
        if let Ok(url) = Url::parse(&self.target_url) {
            let host = url.host_str().unwrap_or("");
            if host != "localhost" && host != "127.0.0.1" && host != "[::1]" {
                let err_msg = format!("Security Violation: Webhook target '{}' is NOT a loopback address. For security reasons (SSRF prevention), only localhost/127.0.0.1 is allowed.", host);
                tracing::error!(target: "stream", error = %err_msg);
                println!("❌ {}", err_msg);
                return;
            }
        } else {
            println!("❌ Invalid webhook_target URL: {}", self.target_url);
            return;
        }

        let msg_id = event.get("msgId").or(event.get("id")).and_then(|v| v.as_str()).unwrap_or("unknown_id").to_string();
        let msg_type = event.get("msg_type").or(event.get("msgType")).and_then(|v| v.as_str()).unwrap_or("UNKNOWN").to_string();
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
                let _ = self.dlq.save(&msg_id, &msg_type, &payload, &headers, &err_msg).await;
            }
            Err(e) => {
                let err_msg = format!("Network error: {}", e);
                tracing::error!(target: "stream", msg_id = %msg_id, error = %e, "Forward network failed, saving to DLQ");
                println!("❌ Forward network failed: {}", err_msg);
                let _ = self.dlq.save(&msg_id, &msg_type, &payload, &headers, &err_msg).await;
            }
        }
    }
}
