use anyhow::Result;
use async_trait::async_trait;
use cowen_common::reset::ResetTask;
use std::path::PathBuf;

pub struct TelemetryResetTask {
    app_dir: PathBuf,
}

impl TelemetryResetTask {
    pub fn new(app_dir: PathBuf) -> Self {
        Self { app_dir }
    }
}

#[async_trait]
impl ResetTask for TelemetryResetTask {
    fn name(&self) -> &'static str {
        "Telemetry & Logs"
    }

    fn description(&self) -> &'static str {
        "Clears telemetry databases (telemetry.db), daemon PID files, and local log directories."
    }

    async fn dry_run(&self) -> Result<Vec<String>> {
        let mut actions = Vec::new();
        
        let telemetry_db = self.app_dir.join("telemetry.db");
        if telemetry_db.exists() {
            actions.push(format!("Delete Telemetry DB: {}", telemetry_db.display()));
        }

        for entry in std::fs::read_dir(&self.app_dir)?.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".pid") {
                    actions.push(format!("Delete Daemon PID: {}", entry.path().display()));
                } else if name == "logs" {
                    actions.push(format!("Delete Logs Directory: {}", entry.path().display()));
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

        let telemetry_db = self.app_dir.join("telemetry.db");
        if telemetry_db.exists() {
            let _ = std::fs::remove_file(telemetry_db);
        }

        for entry in std::fs::read_dir(&self.app_dir)?.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".pid") {
                    let _ = std::fs::remove_file(entry.path());
                } else if name == "logs" {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }

        Ok(())
    }
}
