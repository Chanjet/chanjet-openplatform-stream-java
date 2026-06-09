use std::sync::Arc;
use tonic::{Request, Response, Status};
use cowen_macros::rbac;

use cowen_common::grpc::proto;
use proto::cowen_daemon_service_server::CowenDaemonService;
use proto::*;

use cowen_common::daemon::DaemonService;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;
use cowen_auth::client::Client;
use tracing::info;

pub struct CowenDaemonController {
    service: Arc<dyn DaemonService>,
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
    capabilities: Arc<cowen_capabilities::CapabilityRegistry>,
}

impl CowenDaemonController {
    pub fn new(service: Arc<dyn DaemonService>, vault: Arc<dyn Vault>, cfg_mgr: ConfigManager, capabilities: Arc<cowen_capabilities::CapabilityRegistry>) -> Self {
        Self { 
            service, 
            vault,
            cfg_mgr,
            capabilities,
        }
    }
}


#[tonic::async_trait]
impl CowenDaemonService for CowenDaemonController {
    type TunnelPluginStream = std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<TunnelPluginResponse, Status>> + Send + 'static>>;

    async fn ping(&self, _request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        Ok(Response::new(PingResponse { message: "pong".to_string() }))
    }

    #[rbac]
    async fn start_worker(&self, request: Request<StartWorkerRequest>) -> Result<Response<StartWorkerResponse>, Status> {
        let req = request.into_inner();
        info!("StartWorker requested for {}", req.profile);
        
        let _config = if req.config_json.is_empty() {
            match self.cfg_mgr.load(&req.profile).await {
                Ok(c) => c,
                Err(e) => return Err(Status::not_found(format!("Profile not found: {}", e)))
            }
        } else {
            match serde_json::from_str(&req.config_json) {
                Ok(c) => c,
                Err(e) => return Err(Status::invalid_argument(e.to_string()))
            }
        };

        match self.service.start_daemon(&req.profile).await {
            Ok(_) => Ok(Response::new(StartWorkerResponse { success: true, message: format!("Worker {} started", req.profile) })),
            Err(e) => Ok(Response::new(StartWorkerResponse { success: false, message: e.to_string() })),
        }
    }

    #[rbac]
    async fn start_all_workers(&self, request: Request<StartAllWorkersRequest>) -> Result<Response<StartAllWorkersResponse>, Status> {
        let _ = &request;
        info!("StartAllWorkers requested");
        match self.service.start_all().await {
            Ok(_) => Ok(Response::new(StartAllWorkersResponse { success: true, message: "All workers started".to_string() })),
            Err(e) => Ok(Response::new(StartAllWorkersResponse { success: false, message: e.to_string() })),
        }
    }

    #[rbac]
    async fn stop_worker(&self, request: Request<StopWorkerRequest>) -> Result<Response<StopWorkerResponse>, Status> {
        let req = request.into_inner();
        info!("StopWorker requested for {}", req.profile);
        match self.service.stop_daemon(&req.profile).await {
            Ok(_) => Ok(Response::new(StopWorkerResponse { success: true, message: format!("Worker {} stopped", req.profile) })),
            Err(e) => Ok(Response::new(StopWorkerResponse { success: false, message: e.to_string() })),
        }
    }

    #[rbac]
    async fn stop_all_workers(&self, request: Request<StopAllWorkersRequest>) -> Result<Response<StopAllWorkersResponse>, Status> {
        let _ = &request;
        info!("StopAllWorkers requested");
        match self.service.stop_all().await {
            Ok(_) => Ok(Response::new(StopAllWorkersResponse { success: true, message: "All workers stopped".to_string() })),
            Err(e) => Ok(Response::new(StopAllWorkersResponse { success: false, message: e.to_string() })),
        }
    }

    #[rbac]
    async fn reload_worker(&self, request: Request<ReloadWorkerRequest>) -> Result<Response<ReloadWorkerResponse>, Status> {
        let req = request.into_inner();
        info!("ReloadWorker requested for {}", req.profile);
        match self.service.reload_daemon(&req.profile).await {
            Ok(_) => Ok(Response::new(ReloadWorkerResponse { success: true, message: format!("Worker {} reloaded", req.profile) })),
            Err(e) => Ok(Response::new(ReloadWorkerResponse { success: false, message: e.to_string() })),
        }
    }

    async fn get_status(&self, _request: Request<GetStatusRequest>) -> Result<Response<GetStatusResponse>, Status> {
        Ok(Response::new(GetStatusResponse { statuses: std::collections::HashMap::new() }))
    }

    #[rbac]
    async fn init_profile(&self, request: Request<InitProfileRequest>) -> Result<Response<InitProfileResponse>, Status> {
        let req = request.into_inner();
        info!("InitProfile requested for {}", req.profile);
        let _is_new = !self.cfg_mgr.exists(&req.profile).await;
        
        let mode_str = req.app_mode.unwrap_or_else(|| "oauth2".to_string());
        let mode = match mode_str.parse::<cowen_common::models::AuthMode>() {
            Ok(m) => m,
            Err(e) => return Err(Status::invalid_argument(e.to_string()))
        };

        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let provider = auth_cli.provider(&mode);

        if let Some(ak) = &req.app_key {
            if let Ok(Some(existing_profile)) = provider.find_conflicting_profile(ak, &self.cfg_mgr).await {
                if existing_profile != req.profile {
                    let _ = self.cfg_mgr.set_default_profile(&existing_profile);
                    return Ok(Response::new(InitProfileResponse { success: true, message: format!("CONFLICT_SWITCH:{}", existing_profile) }));
                }
            }
        }

        let mut config = self.cfg_mgr.load(&req.profile).await.unwrap_or_else(|_| cowen_common::Config::default_with_profile(&req.profile));
        config.app_mode = mode.clone();

        if mode == cowen_common::models::AuthMode::Oauth2 {
            config.app_key = cowen_auth::models::BUILTIN_CLIENT_ID.to_string();
            config.app_secret = "".to_string();
        } else {
            if let Some(ak) = &req.app_key { config.app_key = ak.clone(); }
            if let Some(as_) = &req.app_secret { config.app_secret = as_.clone(); }
        }
        if let Some(ref wt) = req.webhook_target { config.webhook_target = wt.clone(); }
        if let Some(pp) = req.proxy_port { config.proxy_port = pp as u16; }

        let mut app_config: cowen_common::config::AppConfig = self.cfg_mgr.load_app_config().await.unwrap_or_default();
        if let Some(url) = &req.openapi_url { app_config.openapi_url = url.clone(); }
        if let Some(url) = &req.stream_url { app_config.stream_url = url.clone(); }
        let _ = self.cfg_mgr.save_app_config(&app_config).await;

        let params = cowen_auth::provider::InitParams {
            app_key: req.app_key.clone(),
            app_secret: req.app_secret.clone(),
            certificate: req.certificate.clone(),
            encrypt_key: req.encrypt_key.clone(),
            webhook_target: req.webhook_target.clone(),
            openapi_url: req.openapi_url.clone(),
            stream_url: req.stream_url.clone(),
            proxy_port: req.proxy_port.map(|p| p as u16),
            auto_start: true,
            is_new: _is_new,
        };

        if mode == cowen_common::models::AuthMode::Oauth2 {
            match self.cfg_mgr.save(&req.profile, &mut config).await {
                Ok(_) => {
                    let _ = self.cfg_mgr.set_default_profile(&req.profile);
                    Ok(Response::new(InitProfileResponse { success: true, message: format!("Profile {} initialized", req.profile) }))
                }
                Err(e) => Err(Status::internal(e.to_string()))
            }
        } else {
            match provider.initialize(&req.profile, &mut config, self.vault.clone(), &self.cfg_mgr, params, Some(self.service.clone())).await {
                Ok(_) => {
                    let _ = self.cfg_mgr.set_default_profile(&req.profile);
                    Ok(Response::new(InitProfileResponse { success: true, message: format!("Profile {} initialized", req.profile) }))
                }
                Err(e) => Err(Status::internal(e.to_string()))
            }
        }
    }

    async fn get_auth_url(&self, request: Request<GetAuthUrlRequest>) -> Result<Response<GetAuthUrlResponse>, Status> {
        let req = request.into_inner();
        info!("GetAuthUrl requested for profile={}, force={}", req.profile, req.force);
        let mut config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(Status::not_found(format!("Profile not found: {}", e)))
        };

        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let provider = auth_cli.provider(&config.app_mode);
        let _ = provider.hydrate_config(&req.profile, &mut config, self.vault.clone()).await;

        if !req.force && config.app_mode == cowen_common::models::AuthMode::Oauth2 {
            if let Ok(rt) = self.vault.get_refresh_token(&req.profile).await {
                if !rt.is_expired() {
                    if let Ok(_token) = auth_cli.refresh_token(&req.profile, &config, &reqwest::header::HeaderMap::new()).await {
                        return Ok(Response::new(GetAuthUrlResponse { success: true, url: "rotated".to_string(), state: "".to_string(), error_message: None }));
                    }
                }
            }
        }

        if config.app_mode == cowen_common::models::AuthMode::SelfBuilt {
            match auth_cli.get_token(&req.profile, &config, &reqwest::header::HeaderMap::new()).await {
                Ok(t) => return Ok(Response::new(GetAuthUrlResponse { success: true, url: t.value, state: "direct".to_string(), error_message: None })),
                Err(e) => return Ok(Response::new(GetAuthUrlResponse { success: false, url: "".to_string(), state: "".to_string(), error_message: Some(e.to_string()) }))
            }
        }

        match provider.generate_auth_url(&req.profile, &mut config, self.vault.clone(), &self.cfg_mgr, cowen_auth::provider::InitParams {
            app_key: None, app_secret: None, certificate: None, encrypt_key: None, openapi_url: None, stream_url: None, webhook_target: None, proxy_port: None, auto_start: false, is_new: false,
        }).await {
            Ok((url, state)) => Ok(Response::new(GetAuthUrlResponse { success: true, url, state, error_message: None })),
            Err(e) => Ok(Response::new(GetAuthUrlResponse { success: false, url: "".to_string(), state: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }

    async fn wait_for_auth(&self, request: Request<WaitForAuthRequest>) -> Result<Response<WaitForAuthResponse>, Status> {
        let req = request.into_inner();
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(Status::not_found(format!("Profile not found: {}", e)))
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let provider = auth_cli.provider(&config.app_mode);
        match provider.wait_for_auth(&req.profile, &config, self.vault.clone(), &self.cfg_mgr, &req.state).await {
            Ok(_) => Ok(Response::new(WaitForAuthResponse { success: true, token: "Success".to_string(), error_message: None })),
            Err(e) => Ok(Response::new(WaitForAuthResponse { success: false, token: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }

    async fn get_token(&self, request: Request<GetTokenRequest>) -> Result<Response<GetTokenResponse>, Status> {
        let req = request.into_inner();
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(Status::not_found(format!("Profile not found: {}", e)))
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let res = if req.refresh {
            auth_cli.refresh_token(&req.profile, &config, &reqwest::header::HeaderMap::new()).await
        } else {
            auth_cli.get_token(&req.profile, &config, &reqwest::header::HeaderMap::new()).await
        };
        match res {
            Ok(t) => Ok(Response::new(GetTokenResponse { token_json: serde_json::to_string(&t).unwrap_or_default(), error_message: None })),
            Err(e) => Ok(Response::new(GetTokenResponse { token_json: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }

    #[rbac]
    async fn clear_token(&self, request: Request<ClearTokenRequest>) -> Result<Response<ClearTokenResponse>, Status> {
        let req = request.into_inner();
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(Status::not_found(format!("Profile not found: {}", e)))
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        match auth_cli.clear_token(&req.profile, &config).await {
            Ok(_) => Ok(Response::new(ClearTokenResponse { success: true, message: "Token cleared".to_string() })),
            Err(e) => Ok(Response::new(ClearTokenResponse { success: false, message: e.to_string() }))
        }
    }

    async fn doctor(&self, request: Request<DoctorRequest>) -> Result<Response<DoctorResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_system::DomainDoctorRequest {
            profile: inner.profile,
        };
        match self.capabilities.native_system.doctor(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(DoctorResponse {
                report: resp.report,
                error_message: resp.error_message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn get_global_config(&self, _request: Request<GetGlobalConfigRequest>) -> Result<Response<GetGlobalConfigResponse>, Status> {
        match self.cfg_mgr.load_app_config().await {
            Ok(c) => Ok(Response::new(GetGlobalConfigResponse { config_json: serde_json::to_string_pretty(&c).unwrap_or_default(), error_message: None })),
            Err(e) => Ok(Response::new(GetGlobalConfigResponse { config_json: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }

    #[rbac]
    async fn set_global_config(&self, request: Request<SetGlobalConfigRequest>) -> Result<Response<SetGlobalConfigResponse>, Status> {
        let req = request.into_inner();
        let value = req.value.trim();
        
        match self.cfg_mgr.set_value("", &req.key, value).await {
            Ok(_) => Ok(Response::new(SetGlobalConfigResponse { success: true, error_message: None })),
            Err(e) => Ok(Response::new(SetGlobalConfigResponse { success: false, error_message: Some(e.to_string()) }))
        }
    }

    #[rbac(profile = "request.get_ref().profile.as_str()")]
    async fn get_config(&self, request: Request<GetConfigRequest>) -> Result<Response<GetConfigResponse>, Status> {
        let req = request.into_inner();
        match self.cfg_mgr.get_value(&req.profile, &req.key).await {
            Ok(v) => {
                let val = match v {
                    serde_json::Value::String(s) => s,
                    _ => v.to_string(),
                };
                Ok(Response::new(GetConfigResponse { config_json: val, error_message: None }))
            }
            Err(e) => Ok(Response::new(GetConfigResponse { config_json: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }

    #[rbac(profile = "request.get_ref().profile.as_str()")]
    async fn list_config(&self, request: Request<ListConfigRequest>) -> Result<Response<ListConfigResponse>, Status> {
        let req = request.into_inner();
        if req.all {
            match self.cfg_mgr.list_all_values().await {
                Ok(v) => Ok(Response::new(ListConfigResponse { config_json: serde_json::to_string(&v).unwrap_or_default(), error_message: None })),
                Err(e) => Ok(Response::new(ListConfigResponse { config_json: "".to_string(), error_message: Some(e.to_string()) }))
            }
        } else {
            match self.cfg_mgr.list_values(&req.profile).await {
                Ok(v) => Ok(Response::new(ListConfigResponse { config_json: serde_json::to_string_pretty(&v).unwrap_or_default(), error_message: None })),
                Err(e) => Ok(Response::new(ListConfigResponse { config_json: "".to_string(), error_message: Some(e.to_string()) }))
            }
        }
    }

    #[rbac(profile = "request.get_ref().profile.as_str()")]
    async fn set_config(&self, request: Request<SetConfigRequest>) -> Result<Response<SetConfigResponse>, Status> {
        let req = request.into_inner();
        let value = req.value.trim();
        
        match self.cfg_mgr.set_value(&req.profile, &req.key, value).await {
            Ok(_) => Ok(Response::new(SetConfigResponse { success: true, error_message: None })),
            Err(e) => Ok(Response::new(SetConfigResponse { success: false, error_message: Some(e.to_string()) }))
        }
    }

    async fn store_status(&self, request: Request<StoreStatusRequest>) -> Result<Response<StoreStatusResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let _inner = request.into_inner();
        let domain_req = cowen_capabilities::native_system::DomainStoreStatusRequest {};
        match self.capabilities.native_system.store_status(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(StoreStatusResponse {
                json: resp.json,
                error_message: resp.error_message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn system_status(&self, request: Request<SystemStatusRequest>) -> Result<Response<SystemStatusResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_system::DomainSystemStatusRequest {
            profile: inner.profile,
            all: inner.all,
        };
        match self.capabilities.native_system.system_status(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(SystemStatusResponse {
                json: resp.json,
                error_message: resp.error_message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn system_reset(&self, request: Request<SystemResetRequest>) -> Result<Response<SystemResetResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_system::DomainSystemResetRequest {
            profile: inner.profile.filter(|p| !p.trim().is_empty()),
            dry_run: inner.dry_run,
        };
        match self.capabilities.native_system.system_reset(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(SystemResetResponse {
                success: resp.success,
                message: resp.message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    #[rbac]
    async fn rename_profile(&self, request: Request<RenameProfileRequest>) -> Result<Response<RenameProfileResponse>, Status> {
        let req = request.into_inner();
        let old_name = req.old_name;
        let new_name = req.new_name;

        // stop old worker if running
        let _ = self.service.stop_daemon(&old_name).await;

        match self.cfg_mgr.rename(&old_name, &new_name).await {
            Ok(_) => Ok(Response::new(RenameProfileResponse { success: true, message: format!("Renamed to {}", new_name) })),
            Err(e) => Ok(Response::new(RenameProfileResponse { success: false, message: e.to_string() }))
        }
    }

    async fn dlq_list(&self, request: Request<DlqListRequest>) -> Result<Response<DlqListResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_dlq::DomainDlqListRequest {
            profile: inner.profile,
            page_size: inner.page_size,
        };
        match self.capabilities.native_dlq.dlq_list(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(DlqListResponse {
                json: resp.json,
                error_message: resp.error_message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn dlq_view(&self, request: Request<DlqViewRequest>) -> Result<Response<DlqViewResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_dlq::DomainDlqViewRequest {
            profile: inner.profile,
            id: inner.id,
        };
        match self.capabilities.native_dlq.dlq_view(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(DlqViewResponse {
                json: resp.json,
                error_message: resp.error_message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn dlq_retry(&self, request: Request<DlqRetryRequest>) -> Result<Response<DlqRetryResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_dlq::DomainDlqRetryRequest {
            profile: inner.profile,
            id: inner.id,
        };
        match self.capabilities.native_dlq.dlq_retry(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(DlqRetryResponse {
                success: resp.success,
                message: resp.message,
                error_message: resp.error_message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn dlq_purge(&self, request: Request<DlqPurgeRequest>) -> Result<Response<DlqPurgeResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_dlq::DomainDlqPurgeRequest {
            profile: inner.profile,
        };
        match self.capabilities.native_dlq.dlq_purge(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(DlqPurgeResponse {
                success: resp.success,
                message: resp.message,
                error_message: resp.error_message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn tail_audit(&self, request: Request<TailAuditRequest>) -> Result<Response<TailAuditResponse>, Status> {
        let req = request.into_inner();
        match self.vault.list_audit(&req.profile, req.lines as usize).await {
            Ok(entries) => {
                let mut content = String::new();
                for entry in entries.iter().rev() {
                    content.push_str(&format!("[{}] {}\\n", entry.timestamp, entry.message));
                }
                Ok(Response::new(TailAuditResponse { content, error_message: None }))
            }
            Err(e) => Ok(Response::new(TailAuditResponse { content: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }

    async fn tunnel_plugin(
        &self,
        request: Request<tonic::Streaming<TunnelPluginRequest>>,
    ) -> Result<Response<Self::TunnelPluginStream>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_system.tunnel_plugin(claims.as_ref(), request.into_inner()).await {
            Ok(stream) => {
                use tokio_stream::StreamExt;
                let mapped = stream.map(|res| res.map_err(|e| Status::internal(e.to_string())));
                Ok(Response::new(Box::pin(mapped) as Self::TunnelPluginStream))
            },
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn plugin_handshake(
        &self,
        request: Request<PluginHandshakeRequest>,
    ) -> Result<Response<PluginHandshakeResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_system::DomainPluginHandshakeRequest {
            plugin_name: inner.plugin_name,
            plugin_version: inner.plugin_version,
            required_capabilities: inner.required_capabilities,
            protocol_version: "1.0".to_string(), // For simplicity, we hardcode here or add to proto later
        };
        match self.capabilities.native_system.plugin_handshake(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(PluginHandshakeResponse {
                success: resp.accepted,
                message: if resp.accepted {
                    format!("Handshake successful. Daemon supports {} capabilities.", resp.supported_capabilities.len())
                } else {
                    resp.error_message.clone().unwrap_or_default()
                },
                supported_capabilities: resp.supported_capabilities,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }
}


