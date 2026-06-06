use std::path::{Path, PathBuf};
use anyhow::{bail, Result, Context};
use std::collections::HashMap;
#[derive(Debug, Clone)]
pub enum WorkerStateDto {
    Created,
    Starting,
    Running,
    Backoff { retry_count: u32, last_error: String, next_retry_in_secs: u64 },
    Failed { reason: String },
    Draining,
    Stopped,
}

#[derive(Debug, Clone)]
pub struct ApiResponseDto {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}
use crate::grpc::proto::{self as grpc_proto, cowen_daemon_service_client::CowenDaemonServiceClient};
use tokio::time::Duration;
use tonic::transport::Channel;


use tonic::service::Interceptor;
use tonic::{Request, Status};

#[derive(Clone)]
pub struct AuthInterceptor {
    pub token: String,
}

impl Interceptor for AuthInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        if !self.token.is_empty() {
            if let Ok(meta_val) = tonic::metadata::MetadataValue::try_from(format!("Bearer {}", self.token)) {
                request.metadata_mut().insert("authorization", meta_val);
            }
        }
        Ok(request)
    }
}

pub type InterceptedClient = CowenDaemonServiceClient<tonic::codegen::InterceptedService<Channel, AuthInterceptor>>;

#[derive(Debug)]
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
    StoreStatusData { json: String },
    AuditData { content: String },
    ApiListData { total: usize, json: String, plugin_used: Option<String> },
    ApiSpecData { json: String },
    Empty,
}

#[derive(Clone)]
pub struct DaemonClient {
    pub port_path: PathBuf,
}

impl DaemonClient {
    pub fn new<P: AsRef<Path>>(port_path: P) -> Self {
        Self {
            port_path: port_path.as_ref().to_path_buf(),
        }
    }

    pub async fn ensure_daemon(&self) -> Result<InterceptedClient> {
        if let Ok(client) = self.connect_to_daemon().await {
            let mut retry_count = 0;
            loop {
                let mut test_client = client.clone();
                let ping_fut = test_client.ping(tonic::Request::new(grpc_proto::PingRequest {}));
                if let Ok(Ok(_)) = tokio::time::timeout(Duration::from_millis(1000), ping_fut).await {
                    return Ok(client);
                }
                retry_count += 1;
                if retry_count >= 3 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
        let daemon_path = if let Ok(env_path) = std::env::var("COWEN_DAEMON_BIN") {
            PathBuf::from(env_path)
        } else {
            let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
            exe_dir.join("cowen-daemon")
        };
        if !daemon_path.exists() {
            bail!("cowen-daemon executable not found at {}", daemon_path.display());
        }
        let app_dir = crate::config::get_app_dir();
        let log_dir = app_dir.join("logs");
        if !log_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&log_dir) {
                bail!("Failed to create daemon logs directory at {}: {}", log_dir.display(), e);
            }
        }
        let stdout_file = std::fs::OpenOptions::new().create(true).append(true).open(log_dir.join("daemon.stdout.log"))
            .with_context(|| format!("Failed to open daemon stdout log at {}", log_dir.join("daemon.stdout.log").display()))?;
        let stderr_file = std::fs::OpenOptions::new().create(true).append(true).open(log_dir.join("daemon.stderr.log"))
            .with_context(|| format!("Failed to open daemon stderr log at {}", log_dir.join("daemon.stderr.log").display()))?;
        let _child = std::process::Command::new(&daemon_path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::from(stdout_file))
            .stderr(std::process::Stdio::from(stderr_file))
            .spawn()
            .context("Failed to spawn cowen-daemon process")?;
        eprintln!("🚀 Starting daemon...");
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if let Ok(mut client) = self.connect_to_daemon().await {
                let ping_fut = client.ping(tonic::Request::new(grpc_proto::PingRequest {}));
                if let Ok(Ok(_)) = tokio::time::timeout(Duration::from_millis(300), ping_fut).await {
                    return Ok(client);
                }
            }
        }
        let mut err_msg = "Daemon process was spawned but failed to bind to port within timeout".to_string();
        if let Ok(stderr) = std::fs::read_to_string(log_dir.join("daemon.stderr.log")) {
            let tail: Vec<&str> = stderr.lines().rev().take(10).collect();
            if !tail.is_empty() {
                err_msg.push_str("\nDaemon stderr tail:\n");
                for line in tail.into_iter().rev() {
                    err_msg.push_str(line);
                    err_msg.push('\n');
                }
            }
        }
        bail!("{}", err_msg)
    }

    pub async fn connect_to_daemon(&self) -> Result<InterceptedClient> {
        let app_dir = self.port_path.parent().unwrap_or_else(|| Path::new(""));
        let handshake_json = cowen_sys::get_ipc_binder().fetch_handshake(app_dir).await.context("Failed to fetch IPC handshake")?;
        
        let parsed: serde_json::Value = serde_json::from_str(&handshake_json).context("Invalid handshake payload")?;
        let port = parsed["port"].as_u64().context("Missing port in handshake")? as u16;
        let fetched_token = parsed["token"].as_str().unwrap_or_default().to_string();

        let endpoint = format!("http://127.0.0.1:{}", port);
        let channel = tonic::transport::Endpoint::new(endpoint)?
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .connect()
            .await?;
            
        let token = if let Ok(t) = std::env::var("COWEN_CLI_TOKEN").or_else(|_| std::env::var("COWEN_PLUGIN_IPC_TOKEN")) {
            t
        } else {
            fetched_token
        };

        let interceptor = AuthInterceptor { token: token.trim().to_string() };
        Ok(CowenDaemonServiceClient::with_interceptor(channel, interceptor))
    }

    async fn build_client(&self) -> Result<InterceptedClient> {
        self.ensure_daemon().await
    }

    pub async fn init_profile(&self, profile: &str, app_key: Option<&str>, app_secret: Option<&str>, certificate: Option<&str>, encrypt_key: Option<&str>, webhook_target: Option<&str>, openapi_url: Option<&str>, stream_url: Option<&str>, app_mode: Option<&str>, proxy_port: Option<u32>) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.init_profile(tonic::Request::new(grpc_proto::InitProfileRequest { profile: profile.to_string(), app_key: app_key.map(|s| s.to_string()), app_secret: app_secret.map(|s| s.to_string()), certificate: certificate.map(|s| s.to_string()), encrypt_key: encrypt_key.map(|s| s.to_string()), webhook_target: webhook_target.map(|s| s.to_string()), openapi_url: openapi_url.map(|s| s.to_string()), stream_url: stream_url.map(|s| s.to_string()), app_mode: app_mode.map(|s| s.to_string()), proxy_port })).await?.into_inner();
        if res.success { Ok(DaemonResponse::Success { message: res.message }) } else { Ok(DaemonResponse::Error { code: 500, message: res.message }) }
    }

    pub async fn start_daemon(&self, profile: &str) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.start_worker(tonic::Request::new(grpc_proto::StartWorkerRequest { profile: profile.to_string(), config_json: String::new() })).await?.into_inner();
        if res.success { Ok(DaemonResponse::Success { message: res.message }) } else { Ok(DaemonResponse::Error { code: 500, message: res.message }) }
    }

    pub async fn start_all(&self) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.start_all_workers(tonic::Request::new(grpc_proto::StartAllWorkersRequest {})).await?.into_inner();
        if res.success { Ok(DaemonResponse::Success { message: res.message }) } else { Ok(DaemonResponse::Error { code: 500, message: res.message }) }
    }

    pub async fn stop_daemon(&self, profile: &str) -> Result<DaemonResponse> {
        let mut client = match self.connect_to_daemon().await {
            Ok(c) => c,
            Err(_) => return Ok(DaemonResponse::Success { message: "Daemon is not running.".to_string() })
        };
        let res = client.stop_worker(tonic::Request::new(grpc_proto::StopWorkerRequest { profile: profile.to_string() })).await?.into_inner();
        if res.success { Ok(DaemonResponse::Success { message: res.message }) } else { Ok(DaemonResponse::Error { code: 500, message: res.message }) }
    }

    pub async fn stop_all(&self) -> Result<DaemonResponse> {
        let mut client = match self.connect_to_daemon().await {
            Ok(c) => c,
            Err(_) => return Ok(DaemonResponse::Success { message: "Daemon is not running.".to_string() })
        };
        let res = client.stop_all_workers(tonic::Request::new(grpc_proto::StopAllWorkersRequest {})).await?.into_inner();
        if res.success { Ok(DaemonResponse::Success { message: res.message }) } else { Ok(DaemonResponse::Error { code: 500, message: res.message }) }
    }

    pub async fn ping(&self) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let _res = client.ping(tonic::Request::new(grpc_proto::PingRequest {})).await?.into_inner();
        Ok(DaemonResponse::Pong)
    }

    pub async fn reload_daemon(&self, profile: &str) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.reload_worker(tonic::Request::new(grpc_proto::ReloadWorkerRequest { profile: profile.to_string() })).await?.into_inner();
        if res.success { Ok(DaemonResponse::Success { message: res.message }) } else { Ok(DaemonResponse::Error { code: 500, message: res.message }) }
    }

    pub async fn system_reset(&self, profile: Option<&str>, dry_run: bool) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.system_reset(tonic::Request::new(grpc_proto::SystemResetRequest { profile: profile.map(|s| s.to_string()), dry_run })).await?.into_inner();
        if res.success { Ok(DaemonResponse::Success { message: res.message }) } else { Ok(DaemonResponse::Error { code: 500, message: res.message }) }
    }

    pub async fn get_auth_url(&self, profile: &str, force: bool) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.get_auth_url(tonic::Request::new(grpc_proto::GetAuthUrlRequest { profile: profile.to_string(), force })).await?.into_inner();
        if res.success {
            if res.url == "rotated" {
                Ok(DaemonResponse::AuthRotated)
            } else if res.state == "direct" {
                Ok(DaemonResponse::AuthSuccess { token: res.url })
            } else {
                Ok(DaemonResponse::AuthUrl { url: res.url, state: res.state })
            }
        } else {
            Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap_or_default() })
        }
    }

    pub async fn wait_for_auth(&self, profile: &str, state: &str) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.wait_for_auth(tonic::Request::new(grpc_proto::WaitForAuthRequest { profile: profile.to_string(), state: state.to_string() })).await?.into_inner();
        if res.success { Ok(DaemonResponse::AuthSuccess { token: res.token }) } else { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap_or_default() }) }
    }

    pub async fn get_token(&self, profile: &str, refresh: bool) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.get_token(tonic::Request::new(grpc_proto::GetTokenRequest { profile: profile.to_string(), refresh })).await?.into_inner();
        if res.error_message.is_none() { Ok(DaemonResponse::TokenData { token_json: res.token_json }) } else { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap_or_default() }) }
    }

    pub async fn clear_token(&self, profile: &str) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.clear_token(tonic::Request::new(grpc_proto::ClearTokenRequest { profile: profile.to_string() })).await?.into_inner();
        if res.success { Ok(DaemonResponse::Success { message: res.message }) } else { Ok(DaemonResponse::Error { code: 500, message: res.message }) }
    }

    pub async fn dlq_list(&self, profile: &str, page: usize, page_size: usize) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.dlq_list(tonic::Request::new(grpc_proto::DlqListRequest { profile: profile.to_string(), page: page as u32, page_size: page_size as u32 })).await?.into_inner();
        if res.json.is_empty() && res.error_message.is_some() { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap() }) } else { Ok(DaemonResponse::DlqData { json: res.json }) }
    }

    pub async fn dlq_view(&self, profile: &str, id: &str) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.dlq_view(tonic::Request::new(grpc_proto::DlqViewRequest { profile: profile.to_string(), id: id.to_string() })).await?.into_inner();
        if res.json.is_empty() && res.error_message.is_some() { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap() }) } else { Ok(DaemonResponse::DlqData { json: res.json }) }
    }

    pub async fn dlq_retry(&self, profile: &str, id: &str) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.dlq_retry(tonic::Request::new(grpc_proto::DlqRetryRequest { profile: profile.to_string(), id: id.to_string() })).await?.into_inner();
        if res.success { Ok(DaemonResponse::Success { message: "Retry triggered".to_string() }) } else { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap_or_default() }) }
    }

    pub async fn dlq_purge(&self, profile: &str) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.dlq_purge(tonic::Request::new(grpc_proto::DlqPurgeRequest { profile: profile.to_string() })).await?.into_inner();
        if res.success { Ok(DaemonResponse::Success { message: res.message }) } else { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap_or_default() }) }
    }

    pub async fn api_list(&self, profile: &str, search: Option<&str>, page: u32, page_size: u32, refresh: bool) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.api_list(tonic::Request::new(grpc_proto::ApiListRequest { profile: profile.to_string(), search: search.map(|s| s.to_string()), page, page_size, refresh })).await?.into_inner();
        if res.json.is_empty() && res.error_message.is_some() { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap() }) } else { Ok(DaemonResponse::ApiListData { total: res.total as usize, json: res.json, plugin_used: res.plugin_used }) }
    }

    pub async fn api_spec(&self, profile: &str, method: &str, path: &str) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.api_spec(tonic::Request::new(grpc_proto::ApiSpecRequest { profile: profile.to_string(), method: method.to_string(), path: path.to_string() })).await?.into_inner();
        if res.json.is_empty() && res.error_message.is_some() { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap() }) } else { Ok(DaemonResponse::ApiSpecData { json: res.json }) }
    }

    pub async fn doctor(&self, profile: &str) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.doctor(tonic::Request::new(grpc_proto::DoctorRequest { profile: profile.to_string() })).await?.into_inner();
        if res.report.is_empty() && res.error_message.is_some() { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap() }) } else { Ok(DaemonResponse::DoctorReport { report: res.report }) }
    }

    pub async fn system_status(&self, profile: &str, all: bool) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.system_status(tonic::Request::new(grpc_proto::SystemStatusRequest { profile: profile.to_string(), all })).await?.into_inner();
        if res.json.is_empty() && res.error_message.is_some() { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap() }) } else { Ok(DaemonResponse::SystemStatusData { json: res.json }) }
    }

    pub async fn list_config(&self, profile: &str, format: &str, all: bool) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.list_config(tonic::Request::new(grpc_proto::ListConfigRequest { profile: profile.to_string(), format: format.to_string(), all })).await?.into_inner();
        if res.config_json.is_empty() && res.error_message.is_some() { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap() }) } else { Ok(DaemonResponse::ConfigData { config_json: res.config_json }) }
    }

    pub async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.rename_profile(tonic::Request::new(grpc_proto::RenameProfileRequest { old_name: old_name.to_string(), new_name: new_name.to_string() })).await?.into_inner();
        if res.success { Ok(DaemonResponse::Success { message: res.message }) } else { Ok(DaemonResponse::Error { code: 500, message: res.message }) }
    }

    pub async fn set_global_config(&self, key: &str, value: &str) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.set_global_config(tonic::Request::new(grpc_proto::SetGlobalConfigRequest { key: key.to_string(), value: value.to_string() })).await?.into_inner();
        if res.success { Ok(DaemonResponse::Success { message: "Updated global config".to_string() }) } else { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap_or_default() }) }
    }

    pub async fn store_status(&self) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.store_status(tonic::Request::new(grpc_proto::StoreStatusRequest {})).await?.into_inner();
        if res.json.is_empty() && res.error_message.is_some() { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap() }) } else { Ok(DaemonResponse::StoreStatusData { json: res.json }) }
    }

    pub async fn tail_audit(&self, profile: &str, lines: usize) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.tail_audit(tonic::Request::new(grpc_proto::TailAuditRequest { profile: profile.to_string(), lines: lines as u32 })).await?.into_inner();
        if res.content.is_empty() && res.error_message.is_some() { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap() }) } else { Ok(DaemonResponse::AuditData { content: res.content }) }
    }

    pub async fn call_api(&self, profile: &str, method: &str, path: &str, body: Option<&str>, force: bool) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.call_api(tonic::Request::new(grpc_proto::CallApiRequest { profile: profile.to_string(), method: method.to_string(), path: path.to_string(), data: body.map(|s| s.to_string()), force })).await?.into_inner();
        if res.status >= 400 && res.error_message.is_some() { Ok(DaemonResponse::Error { code: res.status as i32, message: res.error_message.unwrap() }) } else { Ok(DaemonResponse::ApiResponse(ApiResponseDto { status: res.status as u16, headers: res.headers, body: res.body })) }
    }

    pub async fn get_config(&self, profile: &str, key: &str) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.get_config(tonic::Request::new(grpc_proto::GetConfigRequest { profile: profile.to_string(), key: key.to_string() })).await?.into_inner();
        if res.config_json.is_empty() && res.error_message.is_some() { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap() }) } else { Ok(DaemonResponse::ConfigData { config_json: res.config_json }) }
    }

    pub async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<DaemonResponse> {
        let mut client = self.build_client().await?;
        let res = client.set_config(tonic::Request::new(grpc_proto::SetConfigRequest { profile: profile.to_string(), key: key.to_string(), value: value.to_string() })).await?.into_inner();
        if res.success { Ok(DaemonResponse::Success { message: "Config updated".to_string() }) } else { Ok(DaemonResponse::Error { code: 500, message: res.error_message.unwrap_or_default() }) }
    }
}
