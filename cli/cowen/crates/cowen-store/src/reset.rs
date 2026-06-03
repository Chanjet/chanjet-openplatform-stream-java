use cowen_common::reset::ResetTask;
use async_trait::async_trait;
use std::path::PathBuf;

pub struct StorageResetTask {
    app_dir: PathBuf,
    profile: Option<String>,
}

impl StorageResetTask {
    pub fn new(app_dir: PathBuf, profile: Option<String>) -> Self {
        Self { app_dir, profile }
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
        if self.profile.is_none() {
            let files_to_check = vec![
                "cowen.db",
                "cowen.db-shm",
                "cowen.db-wal",
                "cowen.ddl.lock",
            ];
            for file in files_to_check {
                let path = self.app_dir.join(file);
                if path.exists() {
                    actions.push(format!("Delete file: {}", path.display()));
                }
            }
        }
        Ok(actions)
    }

    async fn execute(&self) -> anyhow::Result<()> {
        if self.profile.is_none() {
            let files_to_check = vec![
                "cowen.db",
                "cowen.db-shm",
                "cowen.db-wal",
                "cowen.ddl.lock",
            ];
            for file in files_to_check {
                let path = self.app_dir.join(file);
                if path.exists() {
                    if let Err(e) = std::fs::remove_file(&path) {
                        tracing::error!("Failed to delete {:?}: {}", path, e);
                    } else {
                        tracing::info!("Deleted {:?}", path);
                    }
                }
            }
        }
        Ok(())
    }
}
