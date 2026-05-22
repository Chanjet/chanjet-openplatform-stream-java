use anyhow::Result;
use async_trait::async_trait;
use cowen_common::reset::ResetTask;
use std::path::PathBuf;

pub struct ConfigResetTask {
    app_dir: PathBuf,
}

impl ConfigResetTask {
    pub fn new(app_dir: PathBuf) -> Self {
        Self { app_dir }
    }
}

#[async_trait]
impl ResetTask for ConfigResetTask {
    fn name(&self) -> &'static str {
        "Configuration & Vault"
    }

    fn description(&self) -> &'static str {
        "Clears all local YAML configurations (app.yaml) and the encrypted SQLite Vault databases."
    }

    async fn dry_run(&self) -> Result<Vec<String>> {
        let mut actions = Vec::new();
        
        // Profiles (Vaults)
        for entry in std::fs::read_dir(&self.app_dir)?.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".db") {
                    actions.push(format!("Delete Vault DB: {}", entry.path().display()));
                } else if name.ends_with(".yaml") {
                    actions.push(format!("Delete Config YAML: {}", entry.path().display()));
                } else if name == "profiles" {
                    actions.push(format!("Delete legacy profiles directory: {}", entry.path().display()));
                }
            }
        }
        
        Ok(actions)
    }

    async fn execute(&self) -> Result<()> {
        let actions = self.dry_run().await?;
        if actions.is_empty() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&self.app_dir)?.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".db") || name.ends_with(".yaml") {
                    let _ = std::fs::remove_file(entry.path());
                } else if name == "profiles" {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }

        Ok(())
    }
}
