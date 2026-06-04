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
}

impl CowenDaemonController {
    pub fn new(service: Arc<dyn DaemonService>, vault: Arc<dyn Vault>, cfg_mgr: ConfigManager) -> Self {
        Self { service, vault, cfg_mgr }
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
        
        let mode_str = req.app_mode.unwrap_or_else(|| "self_built".to_string());
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
        let req = request.into_inner();
        info!("CallApi requested for profile={} method={} path={}", req.profile, req.method, req.path);
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(Status::not_found(format!("Profile not found: {}", e)))
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        if !auth_cli.supports_api_call(&config) {
            return Err(Status::invalid_argument(format!("Auth mode {:?} does not support direct CLI API calls.", config.app_mode)));
        }

        let app_cfg = match self.cfg_mgr.load_app_config().await {
            Ok(c) => c,
            Err(e) => return Err(Status::internal(e.to_string()))
        };

        let body_option = if req.data.is_none() || req.data.as_ref().unwrap().trim() == "{}" || req.data.as_ref().unwrap().trim().is_empty() {
            None
        } else {
            req.data.clone()
        };

        let method_upper = req.method.to_uppercase();

        if !req.force {
            let spec = match auth_cli.get_openapi_spec(&req.profile, &config, false).await {
                Ok(s) => s,
                Err(e) => return Err(Status::internal(e.to_string()))
            };
            if let Err(e) = cowen_common::openapi::validate_request(&spec, &method_upper, &req.path, &body_option) {
                return Err(Status::invalid_argument(format!("OpenAPI validation failed: {}", e)));
            }
            let path_no_query = req.path.split('?').next().unwrap_or(&req.path);
            if !cowen_auth::client::is_path_in_whitelist(path_no_query, &spec) {
                return Err(Status::permission_denied(format!("CLI Rejected: Target path {} is not in the OpenAPI whitelist.", path_no_query)));
            }
        }

        if req.path.starts_with("http") && !req.path.starts_with(&app_cfg.openapi_url) {
            return Err(Status::permission_denied("CLI Security Block: Absolute external URLs are not allowed.".to_string()));
        }

        let token = match auth_cli.get_token(&req.profile, &config, &reqwest::header::HeaderMap::new()).await {
            Ok(t) => t,
            Err(e) => return Err(Status::internal(format!("Failed to get token: {}", e)))
        };

        let ua = cowen_infra::get_user_agent("0.4.0");
        let client = match cowen_infra::create_client(&ua) {
            Ok(c) => c,
            Err(e) => return Err(Status::internal(e.to_string()))
        };
        let url = if req.path.starts_with("http") {
            req.path.clone()
        } else {
            let base = app_cfg.openapi_url.trim_end_matches('/');
            format!("{}{}", base, req.path)
        };

        let method_enum = match reqwest::Method::from_bytes(method_upper.as_bytes()) {
            Ok(m) => m,
            Err(_) => return Err(Status::invalid_argument(format!("Invalid HTTP method: {}", method_upper)))
        };

        let mut api_req = client.request(method_enum, &url)
            .header("openToken", token.value)
            .header("appKey", config.app_key.trim());

        if let Some(b) = body_option {
            let json_body: serde_json::Value = match serde_json::from_str(&b) {
                Ok(j) => j,
                Err(e) => return Err(Status::invalid_argument(format!("Invalid JSON payload: {}", e)))
            };
            api_req = api_req.json(&json_body);
        }

        match api_req.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let mut headers_map = std::collections::HashMap::new();
                for (k, v) in resp.headers().iter() {
                    if let Ok(v_str) = v.to_str() {
                        headers_map.insert(k.to_string(), v_str.to_string());
                    }
                }
                let body = resp.text().await.unwrap_or_default();
                Ok(Response::new(CallApiResponse { status: status as u32, headers: headers_map, body, error_message: None }))
            }
            Err(e) => Ok(Response::new(CallApiResponse { status: 520, headers: std::collections::HashMap::new(), body: "".to_string(), error_message: Some(format!("Request failed: {}", e)) }))
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
        let req = request.into_inner();
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

    async fn store_status(&self, _request: Request<StoreStatusRequest>) -> Result<Response<StoreStatusResponse>, Status> {
        let app_config: cowen_common::config::AppConfig = self.cfg_mgr.load_app_config().await.unwrap_or_default();
        let json = serde_json::to_string(&app_config.storage).unwrap_or_else(|_| "{}".to_string());
        Ok(Response::new(StoreStatusResponse { json, error_message: None }))
    }

    async fn system_status(&self, request: Request<SystemStatusRequest>) -> Result<Response<SystemStatusResponse>, Status> {
        let req = request.into_inner();
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
                    level: ak_level,
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

    async fn system_reset(&self, request: Request<SystemResetRequest>) -> Result<Response<SystemResetResponse>, Status> {
        check_rbac(&request, None)?;
        let req = request.into_inner();
        let profile = req.profile;
        let dry_run = req.dry_run;

        if dry_run {
            use cowen_common::reset::ResetTask;
            let app_dir = cowen_common::config::get_app_dir();
            let config_task = cowen_config::reset::ConfigResetTask::new(app_dir.clone(), profile.clone());
            let telemetry_task = cowen_monitor::reset::TelemetryResetTask::new(app_dir.clone(), profile.clone());
            let storage_task = cowen_store::reset::StorageResetTask::new(app_dir.clone(), profile.clone());
            
            let mut out = String::new();
            out.push_str("🔍 [DRY RUN] Reset Execution Plan:
");
            
            for task in vec![Box::new(config_task) as Box<dyn ResetTask>, Box::new(telemetry_task), Box::new(storage_task)] {
                out.push_str(&format!("
  📦 Module: {}
", task.name()));
                out.push_str(&format!("  ℹ️  {}
", task.description()));
                if let Ok(actions) = task.dry_run().await {
                    if actions.is_empty() {
                        out.push_str("      - No actions to perform.
");
                    } else {
                        for a in actions {
                            out.push_str(&format!("      - {}
", a));
                        }
                    }
                }
            }
            Ok(Response::new(SystemResetResponse { success: true, message: out }))
        } else {
            use cowen_common::reset::ResetTask;
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
        let req = request.into_inner();
        match self.vault.list_dlq(&req.profile, req.page_size as usize).await {
            Ok(msgs) => Ok(Response::new(DlqListResponse { json: serde_json::to_string(&msgs).unwrap_or_default(), error_message: None })),
            Err(e) => Ok(Response::new(DlqListResponse { json: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }

    async fn dlq_view(&self, request: Request<DlqViewRequest>) -> Result<Response<DlqViewResponse>, Status> {
        let req = request.into_inner();
        let id_i64 = match req.id.parse::<i64>() {
            Ok(i) => i,
            Err(_) => return Err(Status::invalid_argument("Invalid DLQ ID format")),
        };
        match self.vault.get_dlq_by_id(id_i64).await {
            Ok(Some(msg)) => Ok(Response::new(DlqViewResponse { json: serde_json::to_string(&msg).unwrap_or_default(), error_message: None })),
            Ok(None) => Ok(Response::new(DlqViewResponse { json: "".to_string(), error_message: Some("Not found".to_string()) })),
            Err(e) => Ok(Response::new(DlqViewResponse { json: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }

    async fn dlq_retry(&self, request: Request<DlqRetryRequest>) -> Result<Response<DlqRetryResponse>, Status> {
        check_rbac(&request, None)?;
        let req = request.into_inner();
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(Status::not_found(e.to_string()))
        };
        let app_cfg: cowen_common::config::AppConfig = self.cfg_mgr.load_app_config().await.unwrap_or_default();
        let id_i64 = match req.id.parse::<i64>() {
            Ok(i) => i,
            Err(_) => return Err(Status::invalid_argument("Invalid DLQ ID format")),
        };
        match cowen_server::daemon::forwarder::Forwarder::new(&req.profile, config, &app_cfg, self.vault.clone()) {
            Ok(forwarder) => {
                match forwarder.retry_message(id_i64).await {
                    Ok(_) => Ok(Response::new(DlqRetryResponse { success: true, message: "Retried".to_string(), error_message: None })),
                    Err(e) => Ok(Response::new(DlqRetryResponse { success: false, message: "".to_string(), error_message: Some(e.to_string()) }))
                }
            }
            Err(e) => Ok(Response::new(DlqRetryResponse { success: false, message: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }

    async fn dlq_purge(&self, request: Request<DlqPurgeRequest>) -> Result<Response<DlqPurgeResponse>, Status> {
        check_rbac(&request, None)?;
        let req = request.into_inner();
        match self.vault.list_all_dlq(&req.profile).await {
            Ok(msgs) => {
                let mut count = 0;
                for m in msgs {
                    if let Some(id) = m.id {
                        if self.vault.delete_dlq_by_id(id).await.is_ok() { count += 1; }
                    }
                }
                Ok(Response::new(DlqPurgeResponse { success: true, message: format!("Purged {} messages", count), error_message: None }))
            }
            Err(e) => Ok(Response::new(DlqPurgeResponse { success: false, message: "".to_string(), error_message: Some(e.to_string()) }))
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

    async fn api_list(&self, request: Request<ApiListRequest>) -> Result<Response<ApiListResponse>, Status> {
        let req = request.into_inner();
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(Status::not_found(e.to_string()))
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        match auth_cli.get_openapi_spec(&req.profile, &config, req.refresh).await {
            Ok(spec) => {
                let mut ops = Vec::new();
                if let Some(paths) = spec.get("paths").and_then(|p: &serde_json::Value| p.as_object()) {
                    for (path, methods) in paths {
                        if let Some(methods_obj) = methods.as_object() {
                            for (method, details) in methods_obj {
                                let summary = details.get("summary").and_then(|v: &serde_json::Value| v.as_str()).unwrap_or("");
                                ops.push(serde_json::json!({
                                    "id": format!("{} {}", method.to_uppercase(), path),
                                    "method": method.to_uppercase(),
                                    "path": path,
                                    "summary": summary
                                }));
                            }
                        }
                    }
                }
                if let Some(query) = req.search.as_ref().filter(|q| !q.is_empty()) {
                    let query = query.to_lowercase();
                    ops.retain(|op| {
                        let id = op["id"].as_str().unwrap_or("").to_lowercase();
                        let summary = op["summary"].as_str().unwrap_or("").to_lowercase();
                        id.contains(&query) || summary.contains(&query)
                    });
                }
                
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
                Ok(Response::new(ApiListResponse { total, json, plugin_used: None, error_message: None }))
            }
            Err(e) => Ok(Response::new(ApiListResponse { total: 0, json: "".to_string(), plugin_used: None, error_message: Some(e.to_string()) }))
        }
    }

    async fn api_spec(&self, request: Request<ApiSpecRequest>) -> Result<Response<ApiSpecResponse>, Status> {
        let req = request.into_inner();
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(Status::not_found(e.to_string()))
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        match auth_cli.get_openapi_spec(&req.profile, &config, false).await {
            Ok(spec) => {
                let method_lower = req.method.to_lowercase();
                if let Some(op) = spec.get("paths").and_then(|p: &serde_json::Value| p.get(&req.path)).and_then(|p: &serde_json::Value| p.get(&method_lower)) {
                    Ok(Response::new(ApiSpecResponse { json: serde_json::to_string(op).unwrap_or_default(), error_message: None }))
                } else {
                    Ok(Response::new(ApiSpecResponse { json: "".to_string(), error_message: Some("Not found".to_string()) }))
                }
            }
            Err(e) => Ok(Response::new(ApiSpecResponse { json: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }
}
