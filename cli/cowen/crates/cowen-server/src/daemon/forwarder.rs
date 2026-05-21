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
}

impl Forwarder {
    pub fn new(profile: &str, config: cowen_common::config::Config, vault: Arc<dyn cowen_common::vault::Vault>) -> CowenResult<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| CowenError::Internal(format!("Failed to build HTTP client: {}", e)))?;

        let dlq = Arc::new(DlqStore::new(profile, vault)?);

        Ok(Self {
            client,
            dlq,
            target_url: config.webhook_target.clone(),
        })
    }

    pub async fn retry_message(&self, id: i64) -> CowenResult<()> {
        let entry = self.dlq.get_by_id(id).await?
            .ok_or_else(|| CowenError::Store(format!("Message with ID {} not found in DLQ", id)))?;
        
        let event: Value = serde_json::from_str(&entry.payload)?;
        
        self.forward(event).await?;
        
        // On success, delete from DLQ using precise ID
        self.dlq.delete_by_id(id).await?;
        Ok(())
    }

    pub async fn forward(&self, event: Value) -> CowenResult<()> {
        if self.target_url.is_empty() {
            tracing::warn!(target: "stream", "No webhook_target configured. Event dropped locally.");
            println!("⚠️ No webhook_target configured. Event dropped locally.");
            return Ok(());
        }

        // SSRF Protection: Loopback Only
        if let Ok(url) = Url::parse(&self.target_url) {
            let host = url.host_str().unwrap_or("");
            if host != "localhost" && host != "127.0.0.1" && host != "[::1]" {
                let err_msg = format!("Security Violation: Webhook target '{}' is NOT a loopback address. For security reasons (SSRF prevention), only localhost/127.0.0.1 is allowed.", host);
                tracing::error!(target: "stream", error = %err_msg);
                println!("❌ {}", err_msg);
                return Err(CowenError::Security(err_msg));
            }
        } else {
            println!("❌ Invalid webhook_target URL: {}", self.target_url);
            return Err(CowenError::Auth("Invalid webhook target".to_string()));
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
                Ok(())
            }
            Ok(r) => {
                let err_msg = format!("HTTP error: {}", r.status());
                tracing::error!(target: "stream", msg_id = %msg_id, status = %r.status(), "Forward failed, saving to DLQ");
                println!("❌ Forward failed: {}", err_msg);
                let _ = self.dlq.save(&msg_id, &msg_type, &payload, &headers, &err_msg).await;
                Err(CowenError::Api(err_msg))
            }
            Err(e) => {
                let err_msg = format!("Network error: {}", e);
                tracing::error!(target: "stream", msg_id = %msg_id, error = %e, "Forward network failed, saving to DLQ");
                println!("❌ Forward network failed: {}", err_msg);
                let _ = self.dlq.save(&msg_id, &msg_type, &payload, &headers, &err_msg).await;
                Err(CowenError::Network(e.to_string()))
            }
        }
    }
}
