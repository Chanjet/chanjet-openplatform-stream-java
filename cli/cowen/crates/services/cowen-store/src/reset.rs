use async_trait::async_trait;
use cowen_common::reset::ResetTask;
use std::path::PathBuf;

pub struct StorageResetTask {
    app_dir: PathBuf,
    profile: Option<String>,
}

impl StorageResetTask {
    pub fn new(app_dir: PathBuf, profile: Option<String>) -> Self {
        Self { app_dir, profile }
    }

    fn get_files_to_check(&self) -> Vec<String> {
        if self.profile.is_none() {
            vec![
                "cowen.db".to_string(),
                "cowen.db-shm".to_string(),
                "cowen.db-wal".to_string(),
                "cowen.ddl.lock".to_string(),
            ]
        } else if let Some(ref profile) = self.profile {
            vec![
                format!("{}_dlq.db", profile),
                format!("{}_dlq.db-wal", profile),
                format!("{}_dlq.db-shm", profile),
                format!("{}_status.json", profile),
                format!("{}_status.json.tmp", profile),
            ]
        } else {
            vec![]
        }
    }
}

#[async_trait]
impl ResetTask for StorageResetTask {
    fn name(&self) -> &'static str {
        "Storage Reset"
    }

    fn description(&self) -> &'static str {
        "Cleans up storage databases (cowen.db, etc.) and related files."
    }

    async fn dry_run(&self) -> anyhow::Result<Vec<String>> {
        let mut actions = Vec::new();
        for file in self.get_files_to_check() {
            let path = self.app_dir.join(file);
            if path.exists() {
                actions.push(format!("Delete file: {}", path.display()));
            }
        }
        Ok(actions)
    }

    async fn execute(&self) -> anyhow::Result<()> {
        for file in self.get_files_to_check() {
            let path = self.app_dir.join(file);
            if path.exists() {
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::error!("Failed to delete {:?}: {}", path, e);
                } else {
                    tracing::info!("Deleted {:?}", path);
                }
            }
        }
        Ok(())
    }
}
