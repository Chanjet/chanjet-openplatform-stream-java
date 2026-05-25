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
    StopAllWorkers,
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

pub fn get_ipc_port_path() -> std::path::PathBuf {
    let app_dir = crate::config::get_app_dir();
    app_dir.join("ipc.port")
}

pub mod client {
    use super::*;
    use anyhow::{bail, Context, Result};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    async fn connect_to_daemon(port_path: &Path) -> Result<TcpStream> {
        if !port_path.exists() {
            bail!("Port file missing");
        }
        let port_str = std::fs::read_to_string(port_path)?;
        let port: u16 = port_str.trim().parse()?;
        let stream = TcpStream::connect(format!("127.0.0.1:{}", port)).await?;
        Ok(stream)
    }

    pub async fn ensure_daemon(port_path: &Path) -> Result<TcpStream> {
        if let Ok(stream) = connect_to_daemon(port_path).await {
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
            .arg("--ipc-port-file")
            .arg(port_path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::from(stdout_file))
            .stderr(std::process::Stdio::from(stderr_file))
            .spawn()
            .context(format!(
                "Failed to spawn cowen-daemon at {}",
                daemon_path.display()
            ))?;

        // Retry logic: MAX 25 times, 200ms delay, total 5s
        for _ in 0..25 {
            tokio::time::sleep(Duration::from_millis(200)).await;
            if let Ok(stream) = connect_to_daemon(port_path).await {
                return Ok(stream);
            }
        }

        bail!("FATAL: Failed to connect to cowen-daemon after spawning")
    }

    pub async fn send_request(
        stream: &mut TcpStream,
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
        pub port_path: PathBuf,
    }

    impl IpcDaemonService {
        pub fn new(port_path: PathBuf) -> Self {
            Self { port_path }
        }

        pub async fn ping(&self) -> CowenResult<()> {
            let res = self.request(DaemonRequest::Ping).await?;
            if let DaemonResponse::Pong = res {
                Ok(())
            } else {
                Err(CowenError::api("Invalid ping response"))
            }
        }

        async fn request(&self, req: DaemonRequest) -> CowenResult<DaemonResponse> {
            let mut last_err = None;
            for _ in 0..15 {
                let mut stream = match connect_to_daemon(&self.port_path).await {
                    Ok(s) => s,
                    Err(e) => {
                        last_err = Some(CowenError::api(format!("IPC connection failed: {}", e)));
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
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
            let res = self.request(DaemonRequest::StopAllWorkers).await?;
            if let DaemonResponse::Error { message, .. } = res {
                return Err(CowenError::api(message));
            }
            Ok(())
        }
    }
}
