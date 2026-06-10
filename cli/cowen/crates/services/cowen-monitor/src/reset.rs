use anyhow::Result;
use async_trait::async_trait;
use cowen_common::reset::ResetTask;
use std::path::PathBuf;

pub struct TelemetryResetTask {
    app_dir: PathBuf,
    target_profile: Option<String>,
}

impl TelemetryResetTask {
    pub fn new(app_dir: PathBuf, target_profile: Option<String>) -> Self {
        Self {
            app_dir,
            target_profile,
        }
    }
    fn find_pid_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        if let Some(ref profile) = self.target_profile {
            let pid_file = self.app_dir.join(format!("{}_daemon.pid", profile));
            if pid_file.exists() {
                files.push(pid_file);
            }
        } else {
            for entry in std::fs::read_dir(&self.app_dir)?.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".pid") {
                        files.push(entry.path());
                    }
                }
            }
        }
        Ok(files)
    }
}

#[async_trait]
impl ResetTask for TelemetryResetTask {
    fn name(&self) -> &'static str {
        "Telemetry & Logs"
    }

    fn description(&self) -> &'static str {
        "Clears telemetry databases, daemon PID files, and local log directories."
    }

    async fn dry_run(&self) -> Result<Vec<String>> {
        let mut actions = Vec::new();

        let telemetry_db = self.app_dir.join("telemetry.db");
        if telemetry_db.exists() {
            actions.push(format!("Delete Telemetry DB: {}", telemetry_db.display()));
        }
        let telemetry_wal = self.app_dir.join("telemetry.db-wal");
        if telemetry_wal.exists() {
            actions.push(format!(
                "Delete Telemetry DB WAL: {}",
                telemetry_wal.display()
            ));
        }
        let telemetry_shm = self.app_dir.join("telemetry.db-shm");
        if telemetry_shm.exists() {
            actions.push(format!(
                "Delete Telemetry DB SHM: {}",
                telemetry_shm.display()
            ));
        }

        let logs_dir = self.app_dir.join("logs");
        if logs_dir.exists() {
            actions.push(format!("Delete Logs Directory: {}", logs_dir.display()));
        }

        for pid_file in self.find_pid_files()? {
            actions.push(format!("Delete Daemon PID: {}", pid_file.display()));
        }

        Ok(actions)
    }

    async fn execute(&self) -> Result<()> {
        let telemetry_db = self.app_dir.join("telemetry.db");
        if telemetry_db.exists() {
            let _ = std::fs::remove_file(telemetry_db);
        }
        let telemetry_wal = self.app_dir.join("telemetry.db-wal");
        if telemetry_wal.exists() {
            let _ = std::fs::remove_file(telemetry_wal);
        }
        let telemetry_shm = self.app_dir.join("telemetry.db-shm");
        if telemetry_shm.exists() {
            let _ = std::fs::remove_file(telemetry_shm);
        }

        let logs_dir = self.app_dir.join("logs");
        if logs_dir.exists() {
            let _ = std::fs::remove_dir_all(logs_dir);
        }

        for pid_file in self.find_pid_files()? {
            let _ = std::fs::remove_file(pid_file);
        }

        Ok(())
    }
}
