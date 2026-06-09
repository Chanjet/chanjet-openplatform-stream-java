use std::sync::Arc;
use tracing::info;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;
use cowen_common::daemon::DaemonService;
use cowen_common::grpc::proto::*;
use cowen_common::CowenError;
use cowen_macros::{rbac, rbac_controller};
use cowen_auth::client::Client;

#[tonic::async_trait]
pub trait NativeDaemonCapability: Send + Sync {
    async fn start_worker(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: StartWorkerRequest) -> Result<StartWorkerResponse, CowenError>;
    async fn start_all_workers(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: StartAllWorkersRequest) -> Result<StartAllWorkersResponse, CowenError>;
    async fn stop_worker(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: StopWorkerRequest) -> Result<StopWorkerResponse, CowenError>;
    async fn stop_all_workers(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: StopAllWorkersRequest) -> Result<StopAllWorkersResponse, CowenError>;
    async fn reload_worker(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: ReloadWorkerRequest) -> Result<ReloadWorkerResponse, CowenError>;
    async fn get_status(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: GetStatusRequest) -> Result<GetStatusResponse, CowenError>;
    async fn init_profile(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: InitProfileRequest) -> Result<InitProfileResponse, CowenError>;
    async fn get_auth_url(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: GetAuthUrlRequest) -> Result<GetAuthUrlResponse, CowenError>;
    async fn wait_for_auth(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: WaitForAuthRequest) -> Result<WaitForAuthResponse, CowenError>;
    async fn get_token(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: GetTokenRequest) -> Result<GetTokenResponse, CowenError>;
    async fn clear_token(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: ClearTokenRequest) -> Result<ClearTokenResponse, CowenError>;
    async fn get_global_config(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: GetGlobalConfigRequest) -> Result<GetGlobalConfigResponse, CowenError>;
    async fn set_global_config(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: SetGlobalConfigRequest) -> Result<SetGlobalConfigResponse, CowenError>;
    async fn get_config(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: GetConfigRequest) -> Result<GetConfigResponse, CowenError>;
    async fn list_config(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: ListConfigRequest) -> Result<ListConfigResponse, CowenError>;
    async fn set_config(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: SetConfigRequest) -> Result<SetConfigResponse, CowenError>;
    async fn rename_profile(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: RenameProfileRequest) -> Result<RenameProfileResponse, CowenError>;
    async fn tail_audit(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: TailAuditRequest) -> Result<TailAuditResponse, CowenError>;
}

pub struct DefaultDaemonCapability {
    service: Arc<dyn DaemonService>,
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
}

impl DefaultDaemonCapability {
    pub fn new(service: Arc<dyn DaemonService>, vault: Arc<dyn Vault>, cfg_mgr: ConfigManager) -> Self {
        Self { service, vault, cfg_mgr }
    }
}

#[rbac_controller(domain = "native.daemon")]
#[tonic::async_trait]
impl NativeDaemonCapability for DefaultDaemonCapability {
    #[rbac]
    async fn start_worker(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: StartWorkerRequest) -> Result<StartWorkerResponse, CowenError> {
        info!("StartWorker requested for {}", req.profile);
        
        let _config = if req.config_json.is_empty() {
            match self.cfg_mgr.load(&req.profile).await {
                Ok(c) => c,
                Err(e) => return Ok(StartWorkerResponse { success: false, message: format!("Profile not found: {}", e) })
            }
        } else {
            match serde_json::from_str(&req.config_json) {
                Ok(c) => c,
                Err(e) => return Ok(StartWorkerResponse { success: false, message: e.to_string() })
            }
        };

        match self.service.start_daemon(&req.profile).await {
            Ok(_) => Ok(StartWorkerResponse { success: true, message: format!("Worker {} started", req.profile) }),
            Err(e) => Ok(StartWorkerResponse { success: false, message: e.to_string() }),
        }
    }

    #[rbac]
    async fn start_all_workers(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, _req: StartAllWorkersRequest) -> Result<StartAllWorkersResponse, CowenError> {
        info!("StartAllWorkers requested");
        match self.service.start_all().await {
            Ok(_) => Ok(StartAllWorkersResponse { success: true, message: "All workers started".to_string() }),
            Err(e) => Ok(StartAllWorkersResponse { success: false, message: e.to_string() }),
        }
    }

    #[rbac]
    async fn stop_worker(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: StopWorkerRequest) -> Result<StopWorkerResponse, CowenError> {
        info!("StopWorker requested for {}", req.profile);
        match self.service.stop_daemon(&req.profile).await {
            Ok(_) => Ok(StopWorkerResponse { success: true, message: format!("Worker {} stopped", req.profile) }),
            Err(e) => Ok(StopWorkerResponse { success: false, message: e.to_string() }),
        }
    }

    #[rbac]
    async fn stop_all_workers(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, _req: StopAllWorkersRequest) -> Result<StopAllWorkersResponse, CowenError> {
        info!("StopAllWorkers requested");
        match self.service.stop_all().await {
            Ok(_) => Ok(StopAllWorkersResponse { success: true, message: "All workers stopped".to_string() }),
            Err(e) => Ok(StopAllWorkersResponse { success: false, message: e.to_string() }),
        }
    }

    #[rbac]
    async fn reload_worker(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: ReloadWorkerRequest) -> Result<ReloadWorkerResponse, CowenError> {
        info!("ReloadWorker requested for {}", req.profile);
        match self.service.reload_daemon(&req.profile).await {
            Ok(_) => Ok(ReloadWorkerResponse { success: true, message: format!("Worker {} reloaded", req.profile) }),
            Err(e) => Ok(ReloadWorkerResponse { success: false, message: e.to_string() }),
        }
    }

    // get_status has no rbac in original
    async fn get_status(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, _req: GetStatusRequest) -> Result<GetStatusResponse, CowenError> {
        Ok(GetStatusResponse { statuses: std::collections::HashMap::new() })
    }

    #[rbac]
    async fn init_profile(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: InitProfileRequest) -> Result<InitProfileResponse, CowenError> {
        info!("InitProfile requested for {}", req.profile);
        let _is_new = !self.cfg_mgr.exists(&req.profile).await;
        
        let mode_str = req.app_mode.unwrap_or_else(|| "oauth2".to_string());
        let mode = match mode_str.parse::<cowen_common::models::AuthMode>() {
            Ok(m) => m,
            Err(e) => return Err(CowenError::config(e.to_string()))
        };

        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let provider = auth_cli.provider(&mode);

        if let Some(ak) = &req.app_key {
            if let Ok(Some(existing_profile)) = provider.find_conflicting_profile(ak, &self.cfg_mgr).await {
                if existing_profile != req.profile {
                    let _ = self.cfg_mgr.set_default_profile(&existing_profile);
                    return Ok(InitProfileResponse { success: true, message: format!("CONFLICT_SWITCH:{}", existing_profile) });
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
                    Ok(InitProfileResponse { success: true, message: format!("Profile {} initialized", req.profile) })
                }
                Err(e) => Err(CowenError::config(e.to_string()))
            }
        } else {
            match provider.initialize(&req.profile, &mut config, self.vault.clone(), &self.cfg_mgr, params, Some(self.service.clone())).await {
                Ok(_) => {
                    let _ = self.cfg_mgr.set_default_profile(&req.profile);
                    Ok(InitProfileResponse { success: true, message: format!("Profile {} initialized", req.profile) })
                }
                Err(e) => Err(CowenError::config(e.to_string()))
            }
        }
    }

    // get_auth_url has no rbac in original controller
    async fn get_auth_url(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: GetAuthUrlRequest) -> Result<GetAuthUrlResponse, CowenError> {
        info!("GetAuthUrl requested for profile={}, force={}", req.profile, req.force);
        let mut config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Ok(GetAuthUrlResponse { success: false, url: "".to_string(), state: "".to_string(), error_message: Some(format!("Profile not found: {}", e)) })
        };

        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let provider = auth_cli.provider(&config.app_mode);
        let _ = provider.hydrate_config(&req.profile, &mut config, self.vault.clone()).await;

        if !req.force && config.app_mode == cowen_common::models::AuthMode::Oauth2 {
            if let Ok(rt) = self.vault.get_refresh_token(&req.profile).await {
                if !rt.is_expired() {
                    if let Ok(_token) = auth_cli.refresh_token(&req.profile, &config, &reqwest::header::HeaderMap::new()).await {
                        return Ok(GetAuthUrlResponse { success: true, url: "rotated".to_string(), state: "".to_string(), error_message: None });
                    }
                }
            }
        }

        if config.app_mode == cowen_common::models::AuthMode::SelfBuilt {
            match auth_cli.get_token(&req.profile, &config, &reqwest::header::HeaderMap::new()).await {
                Ok(t) => return Ok(GetAuthUrlResponse { success: true, url: t.value, state: "direct".to_string(), error_message: None }),
                Err(e) => return Ok(GetAuthUrlResponse { success: false, url: "".to_string(), state: "".to_string(), error_message: Some(e.to_string()) })
            }
        }

        match provider.generate_auth_url(&req.profile, &mut config, self.vault.clone(), &self.cfg_mgr, cowen_auth::provider::InitParams {
            app_key: None, app_secret: None, certificate: None, encrypt_key: None, openapi_url: None, stream_url: None, webhook_target: None, proxy_port: None, auto_start: false, is_new: false,
        }).await {
            Ok((url, state)) => Ok(GetAuthUrlResponse { success: true, url, state, error_message: None }),
            Err(e) => Ok(GetAuthUrlResponse { success: false, url: "".to_string(), state: "".to_string(), error_message: Some(e.to_string()) })
        }
    }

    // wait_for_auth has no rbac
    async fn wait_for_auth(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: WaitForAuthRequest) -> Result<WaitForAuthResponse, CowenError> {
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Ok(WaitForAuthResponse { success: false, token: "".to_string(), error_message: Some(format!("Profile not found: {}", e)) })
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let provider = auth_cli.provider(&config.app_mode);
        match provider.wait_for_auth(&req.profile, &config, self.vault.clone(), &self.cfg_mgr, &req.state).await {
            Ok(_) => Ok(WaitForAuthResponse { success: true, token: "Success".to_string(), error_message: None }),
            Err(e) => Ok(WaitForAuthResponse { success: false, token: "".to_string(), error_message: Some(e.to_string()) })
        }
    }

    // get_token has no rbac
    async fn get_token(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: GetTokenRequest) -> Result<GetTokenResponse, CowenError> {
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Ok(GetTokenResponse { token_json: "".to_string(), error_message: Some(format!("Profile not found: {}", e)) })
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let res = if req.refresh {
            auth_cli.refresh_token(&req.profile, &config, &reqwest::header::HeaderMap::new()).await
        } else {
            auth_cli.get_token(&req.profile, &config, &reqwest::header::HeaderMap::new()).await
        };
        match res {
            Ok(t) => Ok(GetTokenResponse { token_json: serde_json::to_string(&t).unwrap_or_default(), error_message: None }),
            Err(e) => Ok(GetTokenResponse { token_json: "".to_string(), error_message: Some(e.to_string()) })
        }
    }

    #[rbac]
    async fn clear_token(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: ClearTokenRequest) -> Result<ClearTokenResponse, CowenError> {
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Ok(ClearTokenResponse { success: false, message: format!("Profile not found: {}", e) })
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        match auth_cli.clear_token(&req.profile, &config).await {
            Ok(_) => Ok(ClearTokenResponse { success: true, message: "Token cleared".to_string() }),
            Err(e) => Ok(ClearTokenResponse { success: false, message: e.to_string() })
        }
    }

    // get_global_config has no rbac
    async fn get_global_config(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, _req: GetGlobalConfigRequest) -> Result<GetGlobalConfigResponse, CowenError> {
        match self.cfg_mgr.load_app_config().await {
            Ok(c) => Ok(GetGlobalConfigResponse { config_json: serde_json::to_string_pretty(&c).unwrap_or_default(), error_message: None }),
            Err(e) => Ok(GetGlobalConfigResponse { config_json: "".to_string(), error_message: Some(e.to_string()) })
        }
    }

    #[rbac]
    async fn set_global_config(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: SetGlobalConfigRequest) -> Result<SetGlobalConfigResponse, CowenError> {
        let value = req.value.trim();
        
        match self.cfg_mgr.set_value("", &req.key, value).await {
            Ok(_) => Ok(SetGlobalConfigResponse { success: true, error_message: None }),
            Err(e) => Ok(SetGlobalConfigResponse { success: false, error_message: Some(e.to_string()) })
        }
    }

    #[rbac(profile = "req.profile.as_str()")]
    async fn get_config(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: GetConfigRequest) -> Result<GetConfigResponse, CowenError> {
        match self.cfg_mgr.get_value(&req.profile, &req.key).await {
            Ok(v) => {
                let val = match v {
                    serde_json::Value::String(s) => s,
                    _ => v.to_string(),
                };
                Ok(GetConfigResponse { config_json: val, error_message: None })
            }
            Err(e) => Ok(GetConfigResponse { config_json: "".to_string(), error_message: Some(e.to_string()) })
        }
    }

    #[rbac(profile = "req.profile.as_str()")]
    async fn list_config(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: ListConfigRequest) -> Result<ListConfigResponse, CowenError> {
        if req.all {
            match self.cfg_mgr.list_all_values().await {
                Ok(v) => Ok(ListConfigResponse { config_json: serde_json::to_string(&v).unwrap_or_default(), error_message: None }),
                Err(e) => Ok(ListConfigResponse { config_json: "".to_string(), error_message: Some(e.to_string()) })
            }
        } else {
            match self.cfg_mgr.list_values(&req.profile).await {
                Ok(v) => Ok(ListConfigResponse { config_json: serde_json::to_string_pretty(&v).unwrap_or_default(), error_message: None }),
                Err(e) => Ok(ListConfigResponse { config_json: "".to_string(), error_message: Some(e.to_string()) })
            }
        }
    }

    #[rbac(profile = "req.profile.as_str()")]
    async fn set_config(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: SetConfigRequest) -> Result<SetConfigResponse, CowenError> {
        let value = req.value.trim();
        
        match self.cfg_mgr.set_value(&req.profile, &req.key, value).await {
            Ok(_) => Ok(SetConfigResponse { success: true, error_message: None }),
            Err(e) => Ok(SetConfigResponse { success: false, error_message: Some(e.to_string()) })
        }
    }

    #[rbac]
    async fn rename_profile(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: RenameProfileRequest) -> Result<RenameProfileResponse, CowenError> {
        let old_name = req.old_name;
        let new_name = req.new_name;

        let _ = self.service.stop_daemon(&old_name).await;

        match self.cfg_mgr.rename(&old_name, &new_name).await {
            Ok(_) => Ok(RenameProfileResponse { success: true, message: format!("Renamed to {}", new_name) }),
            Err(e) => Ok(RenameProfileResponse { success: false, message: e.to_string() })
        }
    }

    // tail_audit has no rbac
    async fn tail_audit(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: TailAuditRequest) -> Result<TailAuditResponse, CowenError> {
        match self.vault.list_audit(&req.profile, req.lines as usize).await {
            Ok(entries) => {
                let mut content = String::new();
                for entry in entries.iter().rev() {
                    content.push_str(&format!("[{}] {}\\n", entry.timestamp, entry.message));
                }
                Ok(TailAuditResponse { content, error_message: None })
            }
            Err(e) => Ok(TailAuditResponse { content: "".to_string(), error_message: Some(e.to_string()) })
        }
    }
}
