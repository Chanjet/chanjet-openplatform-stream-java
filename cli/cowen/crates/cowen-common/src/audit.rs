use crate::{CowenResult, CowenError};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;
use crate::vault::Vault;
use crate::models::AuditEntry;

pub struct AuditStore;

impl AuditStore {
    pub async fn save(vault: &dyn Vault, entry: &AuditEntry) -> CowenResult<()> {
        vault.save_audit(entry).await
    }

    #[allow(dead_code)]
    pub async fn list(vault: &dyn Vault, profile: &str, limit: usize) -> CowenResult<Vec<AuditEntry>> {
        vault.list_audit(profile, limit).await
    }
}

/// A tracing layer that writes audit logs to Vault
pub struct VaultAuditLayer {
    vault_rx: tokio::sync::watch::Receiver<Option<Arc<dyn Vault>>>,
}

impl VaultAuditLayer {
    pub fn new(vault_rx: tokio::sync::watch::Receiver<Option<Arc<dyn Vault>>>) -> Self {
        Self { vault_rx }
    }
}

impl<S> tracing_subscriber::Layer<S> for VaultAuditLayer
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();
        if metadata.target() != "audit" {
            return;
        }

        let vault_opt = self.vault_rx.borrow().clone();
        let vault: Arc<dyn Vault> = match vault_opt {
            Some(v) => v,
            None => return, // Vault not yet initialized
        };

        let mut fields = serde_json::Map::new();
        let mut visitor = JsonVisitor(&mut fields);
        event.record(&mut visitor);

        let profile = fields.remove("profile")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "default".to_string());
        
        let message = fields.remove("message")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "".to_string());

        let entry = AuditEntry {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            profile: profile.clone(),
            level: metadata.level().to_string(),
            target: metadata.target().to_string(),
            message,
            fields: serde_json::Value::Object(fields),
        };

        // Spawn a task to save to vault (tracing is synchronous)
        tokio::spawn(async move {
            let _ = AuditStore::save(vault.as_ref(), &entry).await;
        });
    }
}

struct JsonVisitor<'a>(&'a mut serde_json::Map<String, serde_json::Value>);

impl<'a> tracing::field::Visit for JsonVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name().to_string(), serde_json::json!(format!("{:?}", value)));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(field.name().to_string(), serde_json::json!(value));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(field.name().to_string(), serde_json::json!(value));
    }
}
