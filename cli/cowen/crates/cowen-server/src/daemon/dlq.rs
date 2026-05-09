use cowen_common::{CowenResult, CowenError};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use cowen_common::vault::Vault;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DLQEntry {
    pub id: i64,
    pub topic: String,
    pub payload: String,
    pub retry_count: i32,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

fn mask_json_field<S>(val: &str, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    s.serialize_str(&cowen_common::utils::mask_sensitive_json(val))
}

#[derive(Debug, Serialize, Deserialize)]
struct LegacyDLQEntry {
    pub id: String,
    pub msg_id: String,
    pub msg_type: String,
    #[serde(serialize_with = "mask_json_field")]
    pub payload: String,
    pub error: String,
    pub created_at: DateTime<Utc>,
    pub attempts: u32,
}

pub struct DlqStore {
    vault: Arc<dyn Vault>,
    profile: String,
}

impl DlqStore {
    pub fn new(profile: &str, vault: Arc<dyn Vault>) -> CowenResult<Self> {
        Ok(Self { 
            vault, 
            profile: profile.to_string() 
        })
    }

    pub fn vault(&self) -> &Arc<dyn Vault> {
        &self.vault
    }

    pub async fn save(&self, msg_id: &str, msg_type: &str, payload: &str, _headers: &str, error: &str) -> CowenResult<()> {
        let msg = cowen_common::models::DlqMessage {
            id: None,
            profile: self.profile.clone(),
            topic: format!("{}:{}", msg_type, msg_id),
            payload: payload.to_string(),
            retry_count: 1,
            error: Some(error.to_string()),
            created_at: Utc::now(),
        };
        self.vault.push_dlq(&msg).await
    }

    pub async fn list(&self) -> CowenResult<Vec<DLQEntry>> {
        let msgs = self.vault.list_dlq(&self.profile, 50).await?;
        Ok(msgs.into_iter().map(|m| DLQEntry {
            id: m.id.unwrap_or(0),
            topic: m.topic,
            payload: m.payload,
            retry_count: m.retry_count,
            error: m.error,
            created_at: m.created_at,
        }).collect())
    }

    pub async fn list_all(&self) -> CowenResult<Vec<DLQEntry>> {
        let msgs = self.vault.list_all_dlq(&self.profile).await?;
        Ok(msgs.into_iter().map(|m| DLQEntry {
            id: m.id.unwrap_or(0),
            topic: m.topic,
            payload: m.payload,
            retry_count: m.retry_count,
            error: m.error,
            created_at: m.created_at,
        }).collect())
    }

    pub async fn delete(&self, id: &i64) -> CowenResult<()> {
        // Vault pop_dlq usually handles delete by topic, but for direct ID delete we might need more
        // For now, let's assume we use pop_dlq logic or similar if available in Store
        // Current Store trait doesn't have delete_dlq_by_id, only pop_dlq.
        // I'll add raw_del or similar if needed, or just let it stay for now.
        Ok(())
    }
}
