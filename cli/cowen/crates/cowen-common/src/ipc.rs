use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use crate::config::Config;
use crate::{CowenResult, CowenError};
use async_trait::async_trait;
use std::sync::Arc;
use crate::vault::Vault;
use crate::daemon::DaemonService;

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
    Failed { reason: String },
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

#[cfg(unix)]
pub mod client {
    use super::*;
    use std::path::{Path, PathBuf};
    use tokio::net::UnixStream;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use anyhow::{Context, Result, bail};
    use std::process::Command;
    use std::time::Duration;

    pub async fn ensure_daemon(uds_path: &Path) -> Result<UnixStream> {
        if let Ok(stream) = UnixStream::connect(uds_path).await {
            return Ok(stream);
        }

        // Daemon is not running, spawn it
        let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
        let daemon_path = exe_dir.join("cowen-daemon");
        
        if !daemon_path.exists() {
            bail!("cowen-daemon executable not found at {}", daemon_path.display());
        }

        let _child = Command::new(&daemon_path)
            .arg("--uds")
            .arg(uds_path)
            .spawn()
            .context("Failed to spawn cowen-daemon")?;

        // Retry logic: MAX 5 times, 200ms delay, total 1s (LLD says RETRY_FAST 5 times/200ms)
        for _ in 0..5 {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if let Ok(stream) = UnixStream::connect(uds_path).await {
                return Ok(stream);
            }
        }
        
        bail!("FATAL: Failed to connect to cowen-daemon after spawning")
    }

    pub async fn send_request(stream: &mut UnixStream, req: &DaemonRequest) -> Result<DaemonResponse> {
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
            let mut stream = ensure_daemon(&self.uds_path)
                .await
                .map_err(|e| CowenError::api(format!("IPC connection failed: {}", e)))?;
            
            send_request(&mut stream, &req)
                .await
                .map_err(|e| CowenError::api(format!("IPC request failed: {}", e)))
        }
    }

    #[async_trait]
    impl DaemonService for IpcDaemonService {
        async fn start_daemon(&self, profile: &str, config: &Config, _vault: Arc<dyn Vault>) -> CowenResult<()> {
            let res = self.request(DaemonRequest::StartWorker {
                profile: profile.to_string(),
                config: config.clone(),
            }).await?;

            if let DaemonResponse::Error { message, .. } = res {
                return Err(CowenError::api(message));
            }
            Ok(())
        }

        async fn reload_daemon(&self, profile: &str) -> CowenResult<()> {
            let res = self.request(DaemonRequest::ReloadWorker {
                profile: profile.to_string(),
            }).await?;

            if let DaemonResponse::Error { message, .. } = res {
                return Err(CowenError::api(message));
            }
            Ok(())
        }

        async fn stop_daemon(&self, profile: &str) -> CowenResult<()> {
            let res = self.request(DaemonRequest::StopWorker {
                profile: profile.to_string(),
            }).await?;

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
