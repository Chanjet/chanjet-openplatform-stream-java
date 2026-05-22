use crate::config::Config;
use crate::daemon::DaemonService;
use crate::vault::Vault;
use crate::{CowenError, CowenResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkerStateDto {
    Created,
    Starting,
    Running,
    Backoff {
        retry_count: u32,
        last_error: String,
        next_retry_in_secs: u64,
    },
    Failed {
        reason: String,
    },
    Draining,
    Stopped,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonRequest {
    StartWorker { profile: String, config: Config },
    StopWorker { profile: String },
    ReloadWorker { profile: String },
    GetStatus { profile: Option<String> },
    Ping,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonResponse {
    Success { message: String },
    Status(HashMap<String, WorkerStateDto>),
    Pong,
    Error { code: i32, message: String },
}

pub fn get_uds_path() -> std::path::PathBuf {
    use sha2::{Digest, Sha256};
    let app_dir = crate::config::get_app_dir();
    let mut uds_path = app_dir.join("uds.sock");

    // SUN_LEN is usually 104-108. If path is too long (e.g. in deep parallel tests),
    // we use a hashed name in /tmp to ensure socket binding succeeds.
    if uds_path.to_string_lossy().len() >= 100 {
        let mut hasher = Sha256::new();
        hasher.update(uds_path.to_string_lossy().as_bytes());
        let hash_bytes = hasher.finalize();
        let hash = hash_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        uds_path = std::path::PathBuf::from(format!("/tmp/cowen_{}.sock", &hash[..16]));
    }
    uds_path
}

#[cfg(unix)]
pub mod client {
    use super::*;
    use anyhow::{bail, Context, Result};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    pub async fn ensure_daemon(uds_path: &Path) -> Result<UnixStream> {
        if let Ok(stream) = UnixStream::connect(uds_path).await {
            return Ok(stream);
        }

        // Daemon is not running, spawn it
        let daemon_path = if let Ok(env_path) = std::env::var("COWEN_DAEMON_BIN") {
            PathBuf::from(env_path)
        } else {
            let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
            exe_dir.join("cowen-daemon")
        };

        if !daemon_path.exists() {
            bail!(
                "cowen-daemon executable not found at {}",
                daemon_path.display()
            );
        }

        // Redirect stdout/stderr to files
        let app_dir = crate::config::get_app_dir();
        let log_dir = app_dir.join("logs");
        if !log_dir.exists() {
            let _ = std::fs::create_dir_all(&log_dir);
        }
        let stdout_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("daemon.stdout.log"))?;
        let stderr_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("daemon.stderr.log"))?;

        let _child = Command::new(&daemon_path)
            .arg("--uds")
            .arg(uds_path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::from(stdout_file))
            .stderr(std::process::Stdio::from(stderr_file))
            .spawn()
            .context(format!(
                "Failed to spawn cowen-daemon at {}",
                daemon_path.display()
            ))?;

        // Retry logic: MAX 5 times, 200ms delay, total 1s (LLD says RETRY_FAST 5 times/200ms)
        for _ in 0..5 {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if let Ok(stream) = UnixStream::connect(uds_path).await {
                return Ok(stream);
            }
        }

        bail!("FATAL: Failed to connect to cowen-daemon after spawning")
    }

    pub async fn send_request(
        stream: &mut UnixStream,
        req: &DaemonRequest,
    ) -> Result<DaemonResponse> {
        let payload = serde_json::to_vec(req)?;
        let len = payload.len() as u32;
        stream.write_all(&len.to_be_bytes()).await?;
        stream.write_all(&payload).await?;

        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut res_payload = vec![0u8; len];
        stream.read_exact(&mut res_payload).await?;

        let res: DaemonResponse = serde_json::from_slice(&res_payload)?;
        Ok(res)
    }

    pub struct IpcDaemonService {
        pub uds_path: PathBuf,
    }

    impl IpcDaemonService {
        pub fn new(uds_path: PathBuf) -> Self {
            Self { uds_path }
        }

        async fn request(&self, req: DaemonRequest) -> CowenResult<DaemonResponse> {
            let mut last_err = None;
            for _ in 0..3 {
                let mut stream = match ensure_daemon(&self.uds_path).await {
                    Ok(s) => s,
                    Err(e) => {
                        last_err = Some(CowenError::api(format!("IPC connection failed: {}", e)));
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        continue;
                    }
                };

                match send_request(&mut stream, &req).await {
                    Ok(res) => return Ok(res),
                    Err(e) => {
                        last_err = Some(CowenError::api(format!("IPC request failed: {}", e)));
                        // If it's early eof, maybe the daemon just restarted or is busy
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
            Err(last_err.unwrap_or_else(|| CowenError::api("IPC request failed after retries")))
        }
    }

    #[async_trait]
    impl DaemonService for IpcDaemonService {
        async fn start_daemon(
            &self,
            profile: &str,
            config: &Config,
            _vault: Arc<dyn Vault>,
        ) -> CowenResult<()> {
            let res = self
                .request(DaemonRequest::StartWorker {
                    profile: profile.to_string(),
                    config: config.clone(),
                })
                .await?;

            if let DaemonResponse::Error { message, .. } = res {
                return Err(CowenError::api(message));
            }
            Ok(())
        }

        async fn reload_daemon(&self, profile: &str) -> CowenResult<()> {
            let res = self
                .request(DaemonRequest::ReloadWorker {
                    profile: profile.to_string(),
                })
                .await?;

            if let DaemonResponse::Error { message, .. } = res {
                return Err(CowenError::api(message));
            }
            Ok(())
        }

        async fn stop_daemon(&self, profile: &str) -> CowenResult<()> {
            let res = self
                .request(DaemonRequest::StopWorker {
                    profile: profile.to_string(),
                })
                .await?;

            if let DaemonResponse::Error { message, .. } = res {
                return Err(CowenError::api(message));
            }
            Ok(())
        }

        async fn stop_all(&self) -> CowenResult<()> {
            // For IPC, we might not need stop_all unless we want to shut down the daemon.
            // But we can implement a StopDaemon if needed.
            Ok(())
        }
    }
}
