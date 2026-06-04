use std::sync::Arc;
use tonic::{Request, Response, Status};

use cowen_common::grpc::proto;
use proto::cowen_daemon_service_server::CowenDaemonService;
use proto::*;

use cowen_common::daemon::DaemonService;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;
use cowen_auth::client::Client;
use tracing::{info, error};

pub struct CowenDaemonController {
    service: Arc<dyn DaemonService>,
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
    search_orchestrator: crate::orchestrators::SearchOrchestrator,
    api_orchestrator: crate::orchestrators::ApiOrchestrator,
    system_orchestrator: crate::orchestrators::SystemOrchestrator,
    dlq_orchestrator: crate::orchestrators::DlqOrchestrator,
}

impl CowenDaemonController {
    pub fn new(service: Arc<dyn DaemonService>, vault: Arc<dyn Vault>, cfg_mgr: ConfigManager) -> Self {
        Self { 
            service, 
            vault: vault.clone(), 
            cfg_mgr: cfg_mgr.clone(),
            search_orchestrator: crate::orchestrators::SearchOrchestrator::new(),
            api_orchestrator: crate::orchestrators::ApiOrchestrator::new(vault.clone(), cfg_mgr.clone()),
            system_orchestrator: crate::orchestrators::SystemOrchestrator::new(vault.clone(), cfg_mgr.clone()),
            dlq_orchestrator: crate::orchestrators::DlqOrchestrator::new(vault, cfg_mgr),
        }
    }
}

fn check_rbac<T>(req: &Request<T>, target_profile: Option<&str>) -> Result<(), Status> {
    if let Some(claims) = req.extensions().get::<cowen_common::jwt::IpcClaims>() {
        if claims.role == cowen_common::jwt::IpcRole::Plugin {
            if let Some(p) = target_profile {
                if p != claims.sub {
                    return Err(Status::permission_denied(format!("Forbidden: Plugin '{}' is not authorized to access profile '{}'", claims.sub, p)));
                }
            } else {
                return Err(Status::permission_denied(format!("Forbidden: Plugin '{}' is not authorized for this action", claims.sub)));
            }
        }
    }
    Ok(())
}

#[tonic::async_trait]
impl CowenDaemonService for CowenDaemonController {
    async fn ping(&self, _request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        Ok(Response::new(PingResponse { message: "pong".to_string() }))
    }

    async fn start_worker(&self, request: Request<StartWorkerRequest>) -> Result<Response<StartWorkerResponse>, Status> {
        check_rbac(&request, None)?;
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

    async fn start_all_workers(&self, request: Request<StartAllWorkersRequest>) -> Result<Response<StartAllWorkersResponse>, Status> {
        check_rbac(&request, None)?;
        info!("StartAllWorkers requested");
        match self.service.start_all().await {
            Ok(_) => Ok(Response::new(StartAllWorkersResponse { success: true, message: "All workers started".to_string() })),
            Err(e) => Ok(Response::new(StartAllWorkersResponse { success: false, message: e.to_string() })),
        }
    }

    async fn stop_worker(&self, request: Request<StopWorkerRequest>) -> Result<Response<StopWorkerResponse>, Status> {
        check_rbac(&request, None)?;
        let req = request.into_inner();
        info!("StopWorker requested for {}", req.profile);
        match self.service.stop_daemon(&req.profile).await {
            Ok(_) => Ok(Response::new(StopWorkerResponse { success: true, message: format!("Worker {} stopped", req.profile) })),
            Err(e) => Ok(Response::new(StopWorkerResponse { success: false, message: e.to_string() })),
        }
    }

    async fn stop_all_workers(&self, request: Request<StopAllWorkersRequest>) -> Result<Response<StopAllWorkersResponse>, Status> {
        check_rbac(&request, None)?;
        info!("StopAllWorkers requested");
        match self.service.stop_all().await {
            Ok(_) => Ok(Response::new(StopAllWorkersResponse { success: true, message: "All workers stopped".to_string() })),
            Err(e) => Ok(Response::new(StopAllWorkersResponse { success: false, message: e.to_string() })),
        }
    }

    async fn reload_worker(&self, request: Request<ReloadWorkerRequest>) -> Result<Response<ReloadWorkerResponse>, Status> {
        check_rbac(&request, None)?;
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

    async fn init_profile(&self, request: Request<InitProfileRequest>) -> Result<Response<InitProfileResponse>, Status> {
        check_rbac(&request, None)?;
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

    async fn call_api(&self, request: Request<CallApiRequest>) -> Result<Response<CallApiResponse>, Status> {
        check_rbac(&request, None)?;
        self.api_orchestrator.call_api(request.into_inner()).await
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

    async fn clear_token(&self, request: Request<ClearTokenRequest>) -> Result<Response<ClearTokenResponse>, Status> {
        check_rbac(&request, None)?;
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
        self.system_orchestrator.doctor(request.into_inner()).await
    }

    async fn get_global_config(&self, _request: Request<GetGlobalConfigRequest>) -> Result<Response<GetGlobalConfigResponse>, Status> {
        match self.cfg_mgr.load_app_config().await {
            Ok(c) => Ok(Response::new(GetGlobalConfigResponse { config_json: serde_json::to_string_pretty(&c).unwrap_or_default(), error_message: None })),
            Err(e) => Ok(Response::new(GetGlobalConfigResponse { config_json: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }

    async fn set_global_config(&self, request: Request<SetGlobalConfigRequest>) -> Result<Response<SetGlobalConfigResponse>, Status> {
        check_rbac(&request, None)?;
        let req = request.into_inner();
        let value = req.value.trim();
        
        match self.cfg_mgr.set_value("", &req.key, value).await {
            Ok(_) => Ok(Response::new(SetGlobalConfigResponse { success: true, error_message: None })),
            Err(e) => Ok(Response::new(SetGlobalConfigResponse { success: false, error_message: Some(e.to_string()) }))
        }
    }

    async fn get_config(&self, request: Request<GetConfigRequest>) -> Result<Response<GetConfigResponse>, Status> {
        check_rbac(&request, Some(&request.get_ref().profile))?;
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

    async fn list_config(&self, request: Request<ListConfigRequest>) -> Result<Response<ListConfigResponse>, Status> {
        check_rbac(&request, Some(&request.get_ref().profile))?;
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

    async fn set_config(&self, request: Request<SetConfigRequest>) -> Result<Response<SetConfigResponse>, Status> {
        check_rbac(&request, Some(&request.get_ref().profile))?;
        let req = request.into_inner();
        let value = req.value.trim();
        
        match self.cfg_mgr.set_value(&req.profile, &req.key, value).await {
            Ok(_) => Ok(Response::new(SetConfigResponse { success: true, error_message: None })),
            Err(e) => Ok(Response::new(SetConfigResponse { success: false, error_message: Some(e.to_string()) }))
        }
    }

    async fn store_status(&self, request: Request<StoreStatusRequest>) -> Result<Response<StoreStatusResponse>, Status> {
        self.system_orchestrator.store_status(request.into_inner()).await
    }

    async fn system_status(&self, request: Request<SystemStatusRequest>) -> Result<Response<SystemStatusResponse>, Status> {
        self.system_orchestrator.system_status(request.into_inner()).await
    }

    async fn system_reset(&self, request: Request<SystemResetRequest>) -> Result<Response<SystemResetResponse>, Status> {
        check_rbac(&request, None)?;
        self.system_orchestrator.system_reset(request.into_inner()).await
    }

    async fn rename_profile(&self, request: Request<RenameProfileRequest>) -> Result<Response<RenameProfileResponse>, Status> {
        check_rbac(&request, None)?;
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
        self.dlq_orchestrator.dlq_list(request.into_inner()).await
    }

    async fn dlq_view(&self, request: Request<DlqViewRequest>) -> Result<Response<DlqViewResponse>, Status> {
        self.dlq_orchestrator.dlq_view(request.into_inner()).await
    }

    async fn dlq_retry(&self, request: Request<DlqRetryRequest>) -> Result<Response<DlqRetryResponse>, Status> {
        check_rbac(&request, None)?;
        self.dlq_orchestrator.dlq_retry(request.into_inner()).await
    }

    async fn dlq_purge(&self, request: Request<DlqPurgeRequest>) -> Result<Response<DlqPurgeResponse>, Status> {
        check_rbac(&request, None)?;
        self.dlq_orchestrator.dlq_purge(request.into_inner()).await
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

    async fn api_list(&self, request: Request<ApiListRequest>) -> Result<Response<ApiListResponse>, Status> {
        let req = request.into_inner();
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(Status::not_found(e.to_string()))
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        match auth_cli.get_openapi_spec(&req.profile, &config, req.refresh).await {
            Ok(spec) => {
                let mut ops = crate::openapi_parser::OpenApiParser::parse_operations(&spec);
                
                let (filtered_ops, used_plugin_name) = self.search_orchestrator.search_if_needed(
                    &req.profile, 
                    ops, 
                    &req.search
                ).await;
                
                ops = filtered_ops;
                
                let total = ops.len() as u32;
                
                let page = req.page.max(1) as usize;
                let page_size = req.page_size.max(1) as usize;
                let start = (page - 1) * page_size;
                let end = (start + page_size).min(ops.len());
                
                let paged_ops = if start < ops.len() {
                    ops[start..end].to_vec()
                } else {
                    Vec::new()
                };

                let json = serde_json::to_string(&paged_ops).unwrap_or_default();
                Ok(Response::new(ApiListResponse { total, json, plugin_used: used_plugin_name, error_message: None }))
            }
            Err(e) => Ok(Response::new(ApiListResponse { total: 0, json: "".to_string(), plugin_used: None, error_message: Some(e.to_string()) }))
        }
    }

    async fn api_spec(&self, request: Request<ApiSpecRequest>) -> Result<Response<ApiSpecResponse>, Status> {
        check_rbac(&request, None)?;
        self.api_orchestrator.api_spec(request.into_inner()).await
    }
}
