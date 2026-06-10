use anyhow::Result;
use async_trait::async_trait;
use cowen_common::reset::ResetTask;
use std::path::PathBuf;

pub struct ConfigResetTask {
    app_dir: PathBuf,
    target_profile: Option<String>,
}

impl ConfigResetTask {
    pub fn new(app_dir: PathBuf, target_profile: Option<String>) -> Self {
        Self {
            app_dir,
            target_profile,
        }
    }
}

#[async_trait]
impl ResetTask for ConfigResetTask {
    fn name(&self) -> &'static str {
        "Configuration & Vault"
    }

    fn description(&self) -> &'static str {
        "Clears local YAML configurations and the encrypted SQLite Vault databases."
    }

    async fn dry_run(&self) -> Result<Vec<String>> {
        let mut actions = Vec::new();

        if let Some(ref profile) = self.target_profile {
            let config_file = self.app_dir.join(format!("{}.yaml", profile));
            if config_file.exists() {
                actions.push(format!("Delete Config YAML: {}", config_file.display()));
            }
        } else {
            // Profiles (Vaults) and configs
            for entry in std::fs::read_dir(&self.app_dir)?.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".db") {
                        actions.push(format!("Delete Vault DB: {}", entry.path().display()));
                    } else if name.ends_with(".db-wal") {
                        actions.push(format!("Delete Vault DB WAL: {}", entry.path().display()));
                    } else if name.ends_with(".db-shm") {
                        actions.push(format!("Delete Vault DB SHM: {}", entry.path().display()));
                    } else if name.ends_with(".ddl.lock") {
                        actions.push(format!("Delete DDL Lock: {}", entry.path().display()));
                    } else if name.ends_with(".yaml") {
                        actions.push(format!("Delete Config YAML: {}", entry.path().display()));
                    } else if name == "profiles" {
                        actions.push(format!(
                            "Delete legacy profiles directory: {}",
                            entry.path().display()
                        ));
                    }
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

        if let Some(ref profile) = self.target_profile {
            let config_file = self.app_dir.join(format!("{}.yaml", profile));
            if config_file.exists() {
                if let Err(e) = std::fs::remove_file(&config_file) {
                    tracing::error!("Failed to delete {:?}: {}", config_file, e);
                } else {
                    tracing::info!("Deleted {:?}", config_file);
                }
            }
        } else {
            for entry in std::fs::read_dir(&self.app_dir)?.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".db")
                        || name.ends_with(".db-wal")
                        || name.ends_with(".db-shm")
                        || name.ends_with(".ddl.lock")
                        || name.ends_with(".yaml")
                    {
                        let _ = std::fs::remove_file(entry.path());
                    } else if name == "profiles" {
                        let _ = std::fs::remove_dir_all(entry.path());
                    }
                }
            }
        }

        Ok(())
    }
}
