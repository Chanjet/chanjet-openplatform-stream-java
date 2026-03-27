use anyhow::Result;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct DLQEntry {
    pub id: String,
    pub msg_id: String,
    pub msg_type: String,
    pub payload: String,
    pub headers: String,
    pub error: String,
    pub created_at: DateTime<Utc>,
    pub attempts: u32,
}

pub struct DlqStore {
    dir: PathBuf,
}

impl DlqStore {
    pub fn new(profile: &str) -> Result<Self> {
        let app_dir = crate::core::config::get_app_dir();
        let dir = app_dir.join("dlq").join(profile);
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        Ok(Self { dir })
    }

    pub fn save(&self, msg_id: &str, msg_type: &str, payload: &str, headers: &str, error: &str) -> Result<()> {
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

        let path = self.dir.join(format!("{}.json", entry.id));
        let data = serde_json::to_string_pretty(&entry)?;
        fs::write(path, data)?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<DLQEntry>> {
        let mut entries = Vec::new();
        if !self.dir.exists() {
            return Ok(entries);
        }

        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(data) = fs::read_to_string(&path) {
                    if let Ok(dlq) = serde_json::from_str::<DLQEntry>(&data) {
                        entries.push(dlq);
                    }
                }
            }
        }
        entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(entries)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let path = self.dir.join(format!("{}.json", id));
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}
