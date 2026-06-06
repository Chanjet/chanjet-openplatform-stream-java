use std::sync::Arc;
use cowen_config::ConfigManager;
use cowen_common::vault::Vault;
use tonic::{Response, Status};

use tokio::process::Command;
use std::process::Stdio;
use tokio_stream::wrappers::ReceiverStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_stream::StreamExt;
use cowen_common::grpc::proto::{TunnelPluginRequest, TunnelPluginResponse};

use cowen_common::grpc::proto::{
    DoctorRequest, DoctorResponse,
    StoreStatusRequest, StoreStatusResponse,
    SystemStatusRequest, SystemStatusResponse,
    SystemResetRequest, SystemResetResponse,
};

pub struct SystemOrchestrator {
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
    ipc_port: u16,
}

impl SystemOrchestrator {
    pub fn new(vault: Arc<dyn Vault>, cfg_mgr: ConfigManager, ipc_port: u16) -> Self {
        Self { vault, cfg_mgr, ipc_port }
    }

    pub async fn doctor(&self, req: DoctorRequest) -> Result<Response<DoctorResponse>, Status> {
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(Status::not_found(format!("Profile not found: {}", e)))
        };
        let ctx = cowen_doctor::DoctorContext { profile: req.profile.clone(), config, verbose: false, fix: false, vault: self.vault.clone(), cfg_mgr: self.cfg_mgr.clone() };
        let results = match cowen_doctor::run_all_diagnostics(&ctx).await {
            Ok(r) => r,
            Err(e) => return Err(Status::internal(e.to_string()))
        };
        let mut report = String::new();
        for (i, res) in results.iter().enumerate() {
            let (status_str, details) = match &res.status {
                cowen_doctor::DiagnosticStatus::Ok => ("OK", None),
                cowen_doctor::DiagnosticStatus::Warning(msg) => ("WARN", Some(msg)),
                cowen_doctor::DiagnosticStatus::Error(msg) => ("ERROR", Some(msg)),
                cowen_doctor::DiagnosticStatus::Fixed(msg) => ("FIXED", Some(msg)),
            };
            report.push_str(&format!("{}. [{}] {}\n", i + 1, status_str, res.name));
            if let Some(msg) = details {
                report.push_str(&format!("   Details: {}\n", msg));
            }
        }
        Ok(Response::new(DoctorResponse { report, error_message: None }))
    }

    pub async fn store_status(&self, _req: StoreStatusRequest) -> Result<Response<StoreStatusResponse>, Status> {
        let app_config: cowen_common::config::AppConfig = self.cfg_mgr.load_app_config().await.unwrap_or_default();
        let json = serde_json::to_string(&app_config.storage).unwrap_or_else(|_| "{}".to_string());
        Ok(Response::new(StoreStatusResponse { json, error_message: None }))
    }

    pub async fn system_status(&self, req: SystemStatusRequest) -> Result<Response<SystemStatusResponse>, Status> {
        let mut results = Vec::new();
        let list = self.cfg_mgr.list_profiles().await.unwrap_or_default();
        
        let profiles = if req.all {
            list
        } else {
            vec![req.profile.clone()]
        };
        
        if !profiles.is_empty() {
            for prof in profiles {
                let mut entries = Vec::new();
                let config = match self.cfg_mgr.load(&prof).await {
                    Ok(c) => c,
                    Err(_) => {
                        let mut c = cowen_common::config::Config::default_with_profile(&prof);
                        c.apply_env_overrides();
                        c
                    },
                };
                
                if !self.cfg_mgr.exists(&prof).await && config.app_key.is_empty() && config.app_secret.is_empty() {
                    continue;
                }
                let app_config = match self.cfg_mgr.load_app_config().await {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                
                let ctx = cowen_common::status::StatusContext {
                    profile: prof.clone(),
                    config: &config,
                    app_config: &app_config,
                    vault: self.vault.clone(),
                };
                
                // Add Configuration Status Entry
                let mode_str = format!("{:?}", config.app_mode).to_lowercase();
                let mut details = vec![];
                details.push(format!("Build ID:   {}", cowen_common::BUILD_ID));
                details.push(format!("Build Time: {}", cowen_common::BUILD_TIME));
                details.push(format!("OpenAPI:    {}", app_config.openapi_url));
                details.push(format!("Stream:     {}", app_config.stream_url));

                let ak_level = if config.app_key.trim().is_empty() {
                    cowen_common::status::StatusLevel::ERROR
                } else {
                    cowen_common::status::StatusLevel::OK
                };
                let ak_msg = if ak_level == cowen_common::status::StatusLevel::OK {
                    format!("AppKey: {} (Mode: {})", config.app_key, mode_str)
                } else {
                    "AppKey is missing".to_string()
                };

                let config_entry = cowen_common::status::StatusEntry {
                    name: "Configuration".to_string(),
                    icon: "⚙️".to_string(),
                    level: ak_level.clone(),
                    message: ak_msg,
                    reason: if ak_level == cowen_common::status::StatusLevel::ERROR {
                        Some("AppKey is missing".to_string())
                    } else {
                        None
                    },
                    details,
                    children: vec![],
                };
                entries.push(config_entry);

                let daemon_entry = cowen_common::status::collect_daemon_status(&ctx, "Daemon", "Tips", true, None).await;
                if let Ok(e) = daemon_entry {
                    entries.push(e);
                }
                
                if config.proxy_enabled {
                    let status_file = cowen_common::config::get_app_dir().join(format!("{}_status.json", prof));
                    let mut active_proxy_port = None;
                    if let Ok(content) = std::fs::read_to_string(status_file) {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                            active_proxy_port = json.get("proxy_port").and_then(|v| v.as_u64()).map(|p| p as u16);
                        }
                    }
                    let port = active_proxy_port.unwrap_or(config.proxy_port);
                    if port != 0 {
                        entries.push(cowen_common::status::StatusEntry {
                            name: "Local Proxy".to_string(),
                            icon: "⚡".to_string(),
                            level: cowen_common::status::StatusLevel::OK,
                            message: format!("http://127.0.0.1:{} (Auth-Free) [ACTIVE]", port),
                            reason: None,
                            details: vec![],
                            children: vec![],
                        });
                    }
                }

                
                use cowen_auth::client::Client;
                let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
                if let Ok(mut diag_entries) = auth_cli.get_diagnostics(&ctx).await {
                    entries.append(&mut diag_entries);
                }
                
                let entry_val = serde_json::json!({
                    "profile": prof,
                    "entries": entries,
                });
                results.push(entry_val);
            }
        }
        
        let json = serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string());
        Ok(Response::new(SystemStatusResponse { json, error_message: None }))
    }

    pub async fn system_reset(&self, req: SystemResetRequest) -> Result<Response<SystemResetResponse>, Status> {
        let profile = req.profile;
        let dry_run = req.dry_run;

        if dry_run {
            use cowen_common::reset::ResetTask;
            let app_dir = cowen_common::config::get_app_dir();
            let config_task = cowen_config::reset::ConfigResetTask::new(app_dir.clone(), profile.clone());
            let telemetry_task = cowen_monitor::reset::TelemetryResetTask::new(app_dir.clone(), profile.clone());
            let storage_task = cowen_store::reset::StorageResetTask::new(app_dir.clone(), profile.clone());
            
            let mut out = String::new();
            out.push_str("🔍 [DRY RUN] Reset Execution Plan:\n");
            
            for task in vec![Box::new(config_task) as Box<dyn ResetTask>, Box::new(telemetry_task), Box::new(storage_task)] {
                out.push_str(&format!("\n  📦 Module: {}\n", task.name()));
                out.push_str(&format!("  ℹ️  {}\n", task.description()));
                if let Ok(actions) = task.dry_run().await {
                    if actions.is_empty() {
                        out.push_str("      - No actions to perform.\n");
                    } else {
                        for a in actions {
                            out.push_str(&format!("      - {}\n", a));
                        }
                    }
                }
            }
            Ok(Response::new(SystemResetResponse { success: true, message: out }))
        } else {
            let app_dir = cowen_common::config::get_app_dir();
            let config_task = cowen_config::reset::ConfigResetTask::new(app_dir.clone(), profile.clone());
            let telemetry_task = cowen_monitor::reset::TelemetryResetTask::new(app_dir.clone(), profile.clone());
            let storage_task = cowen_store::reset::StorageResetTask::new(app_dir.clone(), profile.clone());
            
            let mut errors = vec![];
            for task in vec![Box::new(config_task) as Box<dyn cowen_common::reset::ResetTask>, Box::new(telemetry_task), Box::new(storage_task)] {
                if let Err(e) = task.execute().await {
                    errors.push(format!("{}: {}", task.name(), e));
                }
            }
            
            // OCP: Clear profile from memory cache and trigger vault deletion via ConfigManager
            {
                let cfg_mgr = &self.cfg_mgr;
                if let Some(ref p) = profile {
                    if !p.is_empty() {
                        if let Err(e) = cfg_mgr.delete(p).await {
                            errors.push(format!("ConfigManager Reset: {}", e));
                        }
                    }
                } else {
                    if let Ok(profiles) = cfg_mgr.list_profiles().await {
                        for p in profiles {
                            if let Err(e) = cfg_mgr.delete(&p).await {
                                errors.push(format!("ConfigManager Reset: {}", e));
                            }
                        }
                    }
                }
            }
            
            if errors.is_empty() {
                Ok(Response::new(SystemResetResponse { success: true, message: "System reset successful".to_string() }))
            } else {
                Ok(Response::new(SystemResetResponse { success: false, message: format!("Errors occurred: {}", errors.join(", ")) }))
            }
        }
    }

    pub async fn tunnel_plugin(
        &self,
        request: tonic::Streaming<TunnelPluginRequest>,
    ) -> Result<Response<std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<TunnelPluginResponse, Status>> + Send + 'static>>>, Status> {
        let mut stream = request;
        
        let first_msg = match stream.message().await {
            Ok(Some(msg)) => msg,
            Ok(None) => return Err(Status::invalid_argument("Empty stream")),
            Err(e) => return Err(Status::internal(format!("Stream error: {}", e))),
        };

        let plugin_name = first_msg.plugin_name.ok_or_else(|| Status::invalid_argument("First message must contain plugin_name"))?;
        
        let plugins_dir = cowen_common::config::get_app_dir().join("plugins");
        let expected_path = if cfg!(target_os = "windows") {
            plugins_dir.join(format!("{}.exe", plugin_name))
        } else {
            plugins_dir.join(&plugin_name)
        };

        if !expected_path.exists() {
            return Err(Status::not_found(format!("Plugin {} not found at {:?}", plugin_name, expected_path)));
        }

        let jwt_secret_vec = cowen_common::jwt::get_global_daemon_secret().cloned().unwrap_or_default();
        let plugin_claims = cowen_common::jwt::IpcClaims::new(
            plugin_name.clone(), 
            cowen_common::jwt::IpcRole::Plugin, 
            vec!["*".to_string()], 
            86400
        );
        let bridge_token = cowen_common::jwt::sign_jwt(&plugin_claims, &jwt_secret_vec)
            .map_err(|e| Status::internal(format!("Failed to sign token: {}", e)))?;

        let port_str = self.ipc_port.to_string();
            
        let profile = first_msg.envs.get("COWEN_PROFILE").cloned().unwrap_or_else(|| "default".to_string());

        let mut cmd = Command::new(&expected_path);
        cmd.args(first_msg.args);
        
        for (k, v) in first_msg.envs {
            cmd.env(k, v);
        }
        
        // Force the bridge token and ipc port from Daemon
        cmd.env("COWEN_PROFILE", profile);
        cmd.env("COWEN_IPC_PORT", port_str);
        cmd.env("COWEN_BRIDGE_TOKEN", bridge_token);
        
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return Err(Status::internal(format!("Failed to spawn plugin: {}", e))),
        };

        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // STDIN task
        tokio::spawn(async move {
            if let Some(payload) = first_msg.stdin_payload {
                if !payload.is_empty() {
                    let _ = stdin.write_all(&payload).await;
                    let _ = stdin.flush().await;
                }
            }
            while let Ok(Some(msg)) = stream.message().await {
                if let Some(payload) = msg.stdin_payload {
                    if payload.is_empty() { break; }
                    if stdin.write_all(&payload).await.is_err() { break; }
                    if stdin.flush().await.is_err() { break; }
                }
            }
        });

        // STDOUT task
        let tx_out = tx.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            loop {
                match stdout.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx_out.send(Ok(TunnelPluginResponse {
                            stdout_payload: Some(buf[..n].to_vec()),
                            stderr_payload: None,
                            error_message: None,
                        })).await.is_err() { break; }
                    }
                    Err(e) => {
                        let _ = tx_out.send(Err(Status::internal(e.to_string()))).await;
                        break;
                    }
                }
            }
        });

        // STDERR task
        let tx_err = tx.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            loop {
                match stderr.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx_err.send(Ok(TunnelPluginResponse {
                            stdout_payload: None,
                            stderr_payload: Some(buf[..n].to_vec()),
                            error_message: None,
                        })).await.is_err() { break; }
                    }
                    Err(e) => {
                        let _ = tx_err.send(Err(Status::internal(e.to_string()))).await;
                        break;
                    }
                }
            }
        });

        // Wait task
        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }
}

