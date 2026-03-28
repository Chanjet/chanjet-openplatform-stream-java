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

    pub fn get(&self, id: &str) -> Result<DLQEntry> {
        let path = self.dir.join(format!("{}.json", id));
        let data = fs::read_to_string(path)?;
        let entry: DLQEntry = serde_json::from_str(&data)?;
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_dlq_lifecycle() -> Result<()> {
        let tmp = tempdir()?;
        let store = DlqStore { dir: tmp.path().to_path_buf() };

        // 1. Save
        store.save("msg1", "test", "{}", "{}", "some error")?;
        
        // 2. List
        let entries = store.list()?;
        assert_eq!(entries.len(), 1);
        let id = &entries[0].id;
        assert_eq!(entries[0].msg_id, "msg1");

        // 3. Get
        let entry = store.get(id)?;
        assert_eq!(entry.error, "some error");

        // 4. Delete
        store.delete(id)?;
        let entries_after = store.list()?;
        assert_eq!(entries_after.len(), 0);

        Ok(())
    }
}
