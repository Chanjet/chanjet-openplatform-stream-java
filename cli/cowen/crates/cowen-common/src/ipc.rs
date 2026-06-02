use crate::config::Config;
use crate::daemon::DaemonService;
use crate::{CowenError, CowenResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IpcEnvelope {
    pub token: String,
    pub request: DaemonRequest,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApiResponseDto {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DaemonRequest {
    StartWorker { profile: String, config: Config },
    StopWorker { profile: String },
    StopAllWorkers,
    ReloadWorker { profile: String },
    GetStatus { profile: Option<String> },
    Ping,
    InitProfile {
        profile: String,
        app_key: Option<String>,
        app_secret: Option<String>,
        certificate: Option<String>,
        encrypt_key: Option<String>,
        webhook_target: Option<String>,
        openapi_url: Option<String>,
        stream_url: Option<String>,
        app_mode: Option<String>,
        proxy_port: Option<u16>,
    },
    CallApi {
        profile: String,
        method: String,
        path: String,
        data: Option<String>,
        force: bool,
    },
    GetAuthUrl {
        profile: String,
        #[serde(default)]
        force: bool,
    },
    WaitForAuth {
        profile: String,
        state: String,
    },
    GetToken {
        profile: String,
        refresh: bool,
    },
    ClearToken {
        profile: String,
    },
    Doctor {
        profile: String,
    },
    GetGlobalConfig,
    SetGlobalConfig {
        key: String,
        value: String,
    },
    SystemStatus {
        profile: String,
        all: bool,
    },
    SystemReset {
        profile: Option<String>,
        dry_run: bool,
    },
    RenameProfile {
        old_name: String,
        new_name: String,
    },
    DlqList {
        profile: String,
        page: usize,
        page_size: usize,
    },
    DlqView {
        profile: String,
        id: String,
    },
    DlqRetry {
        profile: String,
        id: String,
    },
    DlqPurge {
        profile: String,
    },
    TailAudit {
        profile: String,
        lines: usize,
    },
    ApiList {
        profile: String,
        search: Option<String>,
        page: usize,
        page_size: usize,
        refresh: bool,
    },
    ApiSpec {
        profile: String,
        method: String,
        path: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DaemonResponse {
    Success { message: String },
    Status(HashMap<String, WorkerStateDto>),
    Pong,
    Error { code: i32, message: String },
    ApiResponse(ApiResponseDto),
    AuthUrl { url: String, state: String },
    AuthSuccess { token: String },
    AuthRotated,
    DoctorReport { report: String },
    ConfigData { config_json: String },
    TokenData { token_json: String },
    DlqData { json: String },
    SystemStatusData { json: String },
    AuditData { content: String },
    ApiListData { total: usize, json: String, plugin_used: Option<String> },
    ApiSpecData { json: String },
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
        token: &str,
    ) -> Result<DaemonResponse> {
        let envelope = IpcEnvelope {
            token: token.to_string(),
            request: req.clone(),
        };
        let payload = serde_json::to_vec(&envelope)?;
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

        pub async fn init_profile(
            &self,
            profile: &str,
            app_key: Option<String>,
            app_secret: Option<String>,
            certificate: Option<String>,
            encrypt_key: Option<String>,
            webhook_target: Option<String>,
            openapi_url: Option<String>,
            stream_url: Option<String>,
            app_mode: Option<String>,
            proxy_port: Option<u16>,
        ) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::InitProfile {
                profile: profile.to_string(),
                app_key,
                app_secret,
                certificate,
                encrypt_key,
                webhook_target,
                openapi_url,
                stream_url,
                app_mode,
                proxy_port,
            }).await
        }

        pub async fn call_api(
            &self,
            profile: &str,
            method: &str,
            path: &str,
            data: Option<String>,
            force: bool,
        ) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::CallApi {
                profile: profile.to_string(),
                method: method.to_string(),
                path: path.to_string(),
                data,
                force,
            }).await
        }

        pub async fn get_auth_url(&self, profile: &str, force: bool) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::GetAuthUrl {
                profile: profile.to_string(),
                force,
            }).await
        }

        pub async fn wait_for_auth(&self, profile: &str, state: &str) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::WaitForAuth {
                profile: profile.to_string(),
                state: state.to_string(),
            }).await
        }

        pub async fn get_token(&self, profile: &str, refresh: bool) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::GetToken {
                profile: profile.to_string(),
                refresh,
            }).await
        }

        pub async fn clear_token(&self, profile: &str) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::ClearToken {
                profile: profile.to_string(),
            }).await
        }

        pub async fn doctor(&self, profile: &str) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::Doctor {
                profile: profile.to_string(),
            }).await
        }

        pub async fn dlq_list(&self, profile: &str, page: usize, page_size: usize) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::DlqList {
                profile: profile.to_string(),
                page,
                page_size,
            }).await
        }

        pub async fn dlq_view(&self, profile: &str, id: &str) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::DlqView {
                profile: profile.to_string(),
                id: id.to_string(),
            }).await
        }

        pub async fn dlq_retry(&self, profile: &str, id: &str) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::DlqRetry {
                profile: profile.to_string(),
                id: id.to_string(),
            }).await
        }

        pub async fn dlq_purge(&self, profile: &str) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::DlqPurge {
                profile: profile.to_string(),
            }).await
        }

        pub async fn system_status(&self, profile: &str, all: bool) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::SystemStatus {
                profile: profile.to_string(),
                all,
            }).await
        }

        pub async fn system_reset(&self, profile: Option<&str>, dry_run: bool) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::SystemReset {
                profile: profile.map(|s| s.to_string()),
                dry_run,
            }).await
        }

        pub async fn rename_profile(&self, old_name: &str, new_name: &str) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::RenameProfile {
                old_name: old_name.to_string(),
                new_name: new_name.to_string(),
            }).await
        }

        pub async fn tail_audit(&self, profile: &str, lines: usize) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::TailAudit {
                profile: profile.to_string(),
                lines,
            }).await
        }

        pub async fn get_global_config(&self) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::GetGlobalConfig).await
        }
        
        pub async fn api_list(&self, profile: &str, search: Option<&str>, page: usize, page_size: usize, refresh: bool) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::ApiList {
                profile: profile.to_string(),
                search: search.map(|s| s.to_string()),
                page,
                page_size,
                refresh,
            }).await
        }

        pub async fn api_spec(&self, profile: &str, method: &str, path: &str) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::ApiSpec {
                profile: profile.to_string(),
                method: method.to_string(),
                path: path.to_string(),
            }).await
        }

        pub async fn set_global_config(&self, key: &str, value: &str) -> CowenResult<DaemonResponse> {
            self.request(DaemonRequest::SetGlobalConfig {
                key: key.to_string(),
                value: value.to_string(),
            }).await
        }

        async fn request(&self, req: DaemonRequest) -> CowenResult<DaemonResponse> {
            let token_path = self.port_path.with_file_name("ipc.token");
            let token = std::fs::read_to_string(&token_path).unwrap_or_default();
            
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

                match send_request(&mut stream, &req, &token).await {
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
        ) -> CowenResult<()> {
            let _ = ensure_daemon(&self.port_path).await.map_err(|e| CowenError::api(e.to_string()))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_envelope_serialization() {
        let req = DaemonRequest::Ping;
        let env = IpcEnvelope {
            token: "secret-token".to_string(),
            request: req,
        };
        let serialized = serde_json::to_string(&env).unwrap();
        assert!(serialized.contains("secret-token"));
        assert!(serialized.contains("Ping"));
        
        let deserialized: IpcEnvelope = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.token, "secret-token");
        assert!(matches!(deserialized.request, DaemonRequest::Ping));
    }

    #[test]
    fn test_new_ipc_payloads_serialization() {
        // 1. Test InitProfile
        let init_req = DaemonRequest::InitProfile {
            profile: "default".to_string(),
            app_key: Some("test_app_key".to_string()),
            app_secret: Some("test_app_secret".to_string()),
            certificate: None,
            encrypt_key: None,
            webhook_target: None,
            openapi_url: None,
            stream_url: None,
            app_mode: Some("self_built".to_string()),
            proxy_port: Some(8080),
        };
        let env_init = IpcEnvelope {
            token: "tok".to_string(),
            request: init_req,
        };
        let ser_init = serde_json::to_string(&env_init).unwrap();
        assert!(ser_init.contains("InitProfile"));
        assert!(ser_init.contains("test_app_key"));

        let de_init: IpcEnvelope = serde_json::from_str(&ser_init).unwrap();
        if let DaemonRequest::InitProfile { profile, app_key, proxy_port, .. } = de_init.request {
            assert_eq!(profile, "default");
            assert_eq!(app_key.unwrap(), "test_app_key");
            assert_eq!(proxy_port.unwrap(), 8080);
        } else {
            panic!("expected InitProfile");
        }

        // 2. Test CallApi and ApiResponse
        let api_req = DaemonRequest::CallApi {
            profile: "inte".to_string(),
            method: "POST".to_string(),
            path: "/v1/test".to_string(),
            data: Some("{}".to_string()),
            force: true,
        };
        let env_api = IpcEnvelope {
            token: "tok2".to_string(),
            request: api_req,
        };
        let ser_api = serde_json::to_string(&env_api).unwrap();
        assert!(ser_api.contains("CallApi"));

        let de_api: IpcEnvelope = serde_json::from_str(&ser_api).unwrap();
        if let DaemonRequest::CallApi { method, force, .. } = de_api.request {
            assert_eq!(method, "POST");
            assert!(force);
        } else {
            panic!("expected CallApi");
        }

        // 3. Test ApiResponse response payload
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        let api_res = ApiResponseDto {
            status: 200,
            headers,
            body: "ok".to_string(),
        };
        let resp = DaemonResponse::ApiResponse(api_res);
        let ser_resp = serde_json::to_string(&resp).unwrap();
        assert!(ser_resp.contains("ApiResponse"));
        assert!(ser_resp.contains("Content-Type"));

        let de_resp: DaemonResponse = serde_json::from_str(&ser_resp).unwrap();
        if let DaemonResponse::ApiResponse(dto) = de_resp {
            assert_eq!(dto.status, 200);
            assert_eq!(dto.headers.get("Content-Type").unwrap(), "application/json");
            assert_eq!(dto.body, "ok");
        } else {
            panic!("expected ApiResponse");
        }
    }
}
