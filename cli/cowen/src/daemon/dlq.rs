use anyhow::Result;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize)]
pub struct DLQEntry {
    pub id: String,
    pub msg_id: String,
    pub msg_type: String,
    #[serde(serialize_with = "mask_json_field")]
    pub payload: String,
    #[serde(serialize_with = "mask_json_field")]
    pub headers: String,
    pub error: String,
    pub created_at: DateTime<Utc>,
    pub attempts: u32,
}

fn mask_json_field<S>(val: &str, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    s.serialize_str(&crate::core::utils::mask_sensitive_json(val))
}

use crate::core::vault::Vault;
use std::sync::Arc;

pub struct DlqStore {
    vault: Arc<dyn Vault>,
    profile: String,
}

impl DlqStore {
    pub fn new(profile: &str, vault: Arc<dyn Vault>) -> Result<Self> {
        Ok(Self { 
            vault, 
            profile: profile.to_string() 
        })
    }

    pub async fn save(&self, msg_id: &str, msg_type: &str, payload: &str, headers: &str, error: &str) -> Result<()> {
        let entry = DLQEntry {
            id: uuid::Uuid::new_v4().to_string(),
            msg_id: msg_id.to_string(),
            msg_type: msg_type.to_string(),
            payload: payload.to_string(),
            headers: headers.to_string(),
            error: error.to_string(),
            created_at: Utc::now(),
            attempts: 1,
        };

        let key = format!("dlq:{}", entry.id);
        let data = serde_json::to_string(&entry)?;
        self.vault.set(&self.profile, &key, &data).await?;
        Ok(())
    }

    pub async fn list(&self) -> Result<Vec<DLQEntry>> {
        let keys = self.vault.list_keys(&self.profile, "dlq:").await?;
        let mut entries = Vec::new();

        for key in keys {
            if let Ok(data) = self.vault.get(&self.profile, &key).await {
                if let Ok(dlq) = serde_json::from_str::<DLQEntry>(&data) {
                    entries.push(dlq);
                }
            }
        }
        entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(entries)
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let key = format!("dlq:{}", id);
        self.vault.delete(&self.profile, &key).await?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<DLQEntry> {
        let key = format!("dlq:{}", id);
        let data = self.vault.get(&self.profile, &key).await?;
        let entry: DLQEntry = serde_json::from_str(&data)?;
        Ok(entry)
    }
}


