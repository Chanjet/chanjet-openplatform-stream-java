use chrono::{DateTime, Utc};
use cowen_common::vault::Vault;
use cowen_common::CowenResult;
use serde::{Deserialize, Serialize};
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

pub struct DlqStore {
    vault: Arc<dyn Vault>,
    profile: String,
}

impl DlqStore {
    pub fn new(profile: &str, vault: Arc<dyn Vault>) -> CowenResult<Self> {
        Ok(Self {
            vault,
            profile: profile.to_string(),
        })
    }

    pub fn vault(&self) -> &Arc<dyn Vault> {
        &self.vault
    }

    pub async fn save(
        &self,
        msg_id: &str,
        msg_type: &str,
        payload: &str,
        _headers: &str,
        error: &str,
    ) -> CowenResult<()> {
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
        Ok(msgs
            .into_iter()
            .map(|m| DLQEntry {
                id: m.id.unwrap_or(0),
                topic: m.topic,
                payload: m.payload,
                retry_count: m.retry_count,
                error: m.error,
                created_at: m.created_at,
            })
            .collect())
    }

    pub async fn list_all(&self) -> CowenResult<Vec<DLQEntry>> {
        let msgs = self.vault.list_all_dlq(&self.profile).await?;
        Ok(msgs
            .into_iter()
            .map(|m| DLQEntry {
                id: m.id.unwrap_or(0),
                topic: m.topic,
                payload: m.payload,
                retry_count: m.retry_count,
                error: m.error,
                created_at: m.created_at,
            })
            .collect())
    }

    pub async fn list_paged(&self, page: usize, page_size: usize) -> CowenResult<Vec<DLQEntry>> {
        let offset = (page.max(1) - 1) * page_size;
        let msgs = self
            .vault
            .list_dlq_paged(&self.profile, offset, page_size)
            .await?;
        Ok(msgs
            .into_iter()
            .map(|m| DLQEntry {
                id: m.id.unwrap_or(0),
                topic: m.topic,
                payload: m.payload,
                retry_count: m.retry_count,
                error: m.error,
                created_at: m.created_at,
            })
            .collect())
    }

    pub async fn get_by_id(&self, id: i64) -> CowenResult<Option<DLQEntry>> {
        let msg = self.vault.get_dlq_by_id(id).await?;
        Ok(msg.map(|m| DLQEntry {
            id: m.id.unwrap_or(0),
            topic: m.topic,
            payload: m.payload,
            retry_count: m.retry_count,
            error: m.error,
            created_at: m.created_at,
        }))
    }

    pub async fn delete_by_id(&self, id: i64) -> CowenResult<()> {
        self.vault.delete_dlq_by_id(id).await
    }

    pub async fn delete(&self, _id: i64, topic: &str) -> CowenResult<()> {
        let _ = self.vault.pop_dlq(&self.profile, topic).await?;
        Ok(())
    }
}
