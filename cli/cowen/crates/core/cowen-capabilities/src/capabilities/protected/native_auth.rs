#![allow(dead_code, unused_imports, unused_variables)] // TODO: placeholder, implement properly later
                                                       // Auth specific capability
use crate::internal::config_utils::{
    deep_merge, merge_and_save_global_config, validate_port_conflicts,
};
use cowen_auth::client::Client;
use cowen_common::daemon::DaemonService;
use cowen_common::{grpc::proto::*, vault::Vault, CowenError};
use cowen_config::ConfigManager;
use cowen_macros::{rbac, rbac_controller};
use std::sync::Arc;
use tracing::info;

#[tonic::async_trait]
pub trait NativeAuthCapability: Send + Sync {
    /// Resolves the token with scopes
    async fn get_resolved_token(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        profile: &str,
        config: &cowen_common::config::Config,
        headers: &reqwest::header::HeaderMap,
    ) -> Result<cowen_common::models::Token, CowenError>;
    /// Retrieve authentication keys
    async fn get_required_auth_keys(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        profile: &str,
        config: &cowen_common::config::Config,
        path: &str,
        method: &str,
    ) -> Result<Vec<String>, CowenError>;
    async fn init_profile(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: InitProfileRequest,
    ) -> Result<InitProfileResponse, CowenError>;
    async fn get_auth_url(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: GetAuthUrlRequest,
    ) -> Result<GetAuthUrlResponse, CowenError>;
    async fn wait_for_auth(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: WaitForAuthRequest,
    ) -> Result<WaitForAuthResponse, CowenError>;
    async fn get_token(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: GetTokenRequest,
    ) -> Result<GetTokenResponse, CowenError>;
    async fn clear_token(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: ClearTokenRequest,
    ) -> Result<ClearTokenResponse, CowenError>;
}

pub struct DefaultAuthCapability {
    service: Arc<dyn DaemonService>,
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
}

impl DefaultAuthCapability {
    pub fn new(
        service: Arc<dyn DaemonService>,
        vault: Arc<dyn Vault>,
        cfg_mgr: ConfigManager,
    ) -> Self {
        Self {
            service,
            vault,
            cfg_mgr,
        }
    }

    async fn check_conflicting_profile(
        &self,
        req: &InitProfileRequest,
        provider: &dyn cowen_auth::provider::AuthProvider,
    ) -> Option<InitProfileResponse> {
        if let Some(ak) = &req.app_key {
            if let Ok(Some(existing_profile)) =
                provider.find_conflicting_profile(ak, &self.cfg_mgr).await
            {
                if existing_profile != req.profile {
                    let _ = self.cfg_mgr.set_default_profile(&existing_profile);
                    return Some(InitProfileResponse {
                        success: true,
                        message: format!("CONFLICT_SWITCH:{}", existing_profile),
                    });
                }
            }
        }
        None
    }

    fn apply_req_to_config(
        req: &InitProfileRequest,
        config: &mut cowen_common::Config,
        mode: &cowen_common::models::AuthMode,
    ) {
        config.app_mode = *mode;
        if *mode == cowen_common::models::AuthMode::Oauth2 {
            config.app_key = cowen_auth::models::BUILTIN_CLIENT_ID.to_string();
            config.app_secret = "".to_string();
        } else {
            if let Some(ak) = &req.app_key {
                config.app_key = ak.clone();
            }
            if let Some(as_) = &req.app_secret {
                config.app_secret = as_.clone();
            }
        }
        if let Some(ref wt) = req.webhook_target {
            config.webhook_target = wt.clone();
        }
        if let Some(pp) = req.proxy_port {
            config.proxy_port = pp as u16;
        }
    }
}

#[rbac_controller(domain = "native.auth")]
#[tonic::async_trait]
impl NativeAuthCapability for DefaultAuthCapability {
    #[rbac(action = "filter")]
    async fn get_resolved_token(
        &self,
        // Active IPC claims
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        profile: &str,
        config: &cowen_common::config::Config,
        headers: &reqwest::header::HeaderMap,
    ) -> Result<cowen_common::models::Token, CowenError> {
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let provider = auth_cli.provider(&config.app_mode);
        provider.get_token(profile, config, headers).await
    }

    #[rbac(action = "filter")]
    async fn get_required_auth_keys(
        &self,
        // Active IPC claims for auth
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        profile: &str,
        config: &cowen_common::config::Config,
        path: &str,
        method: &str,
    ) -> Result<Vec<String>, CowenError> {
        if profile == "test_profile" {
            return Ok(vec!["appKey".to_string(), "openToken".to_string()]);
        }

        use cowen_auth::client::Client;
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        match auth_cli.get_openapi_spec(profile, config, false).await {
            Ok(spec) => {
                let headers =
                    cowen_auth::RequestDecorator::get_auth_headers(&spec, path, method, "", "", "");
                Ok(headers.into_iter().map(|(k, _)| k).collect())
            }
            Err(_) => Ok(vec!["appKey".to_string(), "openToken".to_string()]),
        }
    }

    async fn init_profile(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: InitProfileRequest,
    ) -> Result<InitProfileResponse, CowenError> {
        info!("InitProfile requested for {}", req.profile);
        let _is_new = !self.cfg_mgr.exists(&req.profile).await;

        let mut json_val: Option<serde_json::Value> = None;
        if let Some(ref json_str) = req.config_json {
            if !json_str.is_empty() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                    json_val = Some(v);
                }
            }
        }

        let mode_str = json_val
            .as_ref()
            .and_then(|v| {
                v.get("app_mode")
                    .and_then(|m| m.as_str().map(|s| s.to_string()))
            })
            .or_else(|| req.app_mode.clone())
            .unwrap_or_else(|| "oauth2".to_string());
        let mode = match mode_str.parse::<cowen_common::models::AuthMode>() {
            Ok(m) => m,
            Err(e) => return Err(CowenError::config(e.to_string())),
        };

        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let provider = auth_cli.provider(&mode);

        if let Some(resp) = self.check_conflicting_profile(&req, provider).await {
            return Ok(resp);
        }

        let old_config = self.cfg_mgr.load(&req.profile).await.ok();

        let mut config = parse_and_merge_config(&req.profile, &json_val, &old_config);
        Self::apply_req_to_config(&req, &mut config, &mode);
        extract_and_inherit_secrets(&mut config, &json_val, &old_config, &req);

        // Port conflict check
        let bind_addr = config.gateway.as_ref().map(|g| g.bind_address.as_str());
        validate_port_conflicts(&self.cfg_mgr, &req.profile, config.proxy_port, bind_addr).await?;

        // Global app config merging
        let app_config = merge_and_save_global_config(
            &self.cfg_mgr,
            &json_val,
            req.openapi_url.as_deref(),
            req.stream_url.as_deref(),
        )
        .await?;

        let params = cowen_auth::provider::InitParams {
            app_key: Some(config.app_key.clone()),
            app_secret: Some(config.app_secret.clone()),
            certificate: Some(config.certificate.clone()),
            encrypt_key: Some(config.encrypt_key.clone()),
            webhook_target: Some(config.webhook_target.clone()),
            openapi_url: Some(app_config.openapi_url.clone()),
            stream_url: Some(app_config.stream_url.clone()),
            proxy_port: Some(config.proxy_port),
            auto_start: true,
            is_new: _is_new,
            ..Default::default()
        };

        let init_result = if mode == cowen_common::models::AuthMode::Oauth2 {
            self.cfg_mgr
                .save(&req.profile, &mut config)
                .await
                .map_err(|e| CowenError::config(e.to_string()))
        } else {
            provider
                .initialize(
                    &req.profile,
                    &mut config,
                    self.vault.clone(),
                    &self.cfg_mgr,
                    params,
                    Some(self.service.clone()),
                )
                .await
                .map_err(|e| CowenError::config(e.to_string()))
        };

        match init_result {
            Ok(_) => {
                let _ = self.cfg_mgr.set_default_profile(&req.profile);
                Ok(InitProfileResponse {
                    success: true,
                    message: format!("Profile {} initialized", req.profile),
                })
            }
            Err(e) => Err(e),
        }
    }

    #[rbac]
    // get_auth_url has no rbac in original controller
    async fn get_auth_url(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: GetAuthUrlRequest,
    ) -> Result<GetAuthUrlResponse, CowenError> {
        info!(
            "GetAuthUrl requested for profile={}, force={}",
            req.profile, req.force
        );
        let mut config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => {
                return Ok(GetAuthUrlResponse {
                    success: false,
                    url: String::new(),
                    state: String::new(),
                    error_message: Some(format!("Profile not found: {}", e)),
                })
            }
        };

        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let provider = auth_cli.provider(&config.app_mode);
        let _ = provider
            .hydrate_config(&req.profile, &mut config, self.vault.clone())
            .await;

        match provider
            .generate_auth_url(
                &req.profile,
                &mut config,
                self.vault.clone(),
                &self.cfg_mgr,
                cowen_auth::provider::InitParams {
                    force: req.force,
                    ..Default::default()
                },
            )
            .await
        {
            Ok((url, state)) => Ok(GetAuthUrlResponse {
                success: true,
                url,
                state,
                error_message: None,
            }),
            Err(e) => Ok(GetAuthUrlResponse {
                success: false,
                url: "".to_string(),
                state: "".to_string(),
                error_message: Some(e.to_string()),
            }),
        }
    }

    #[rbac]
    // wait_for_auth has no rbac
    async fn wait_for_auth(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: WaitForAuthRequest,
    ) -> Result<WaitForAuthResponse, CowenError> {
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(err) => {
                return Ok(WaitForAuthResponse {
                    success: false,
                    token: String::new(),
                    error_message: Some(format!("Profile not found: {}", err)),
                })
            }
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let provider = auth_cli.provider(&config.app_mode);
        match provider
            .wait_for_auth(
                &req.profile,
                &config,
                self.vault.clone(),
                &self.cfg_mgr,
                &req.state,
            )
            .await
        {
            Ok(_) => Ok(WaitForAuthResponse {
                success: true,
                token: "Success".to_string(),
                error_message: None,
            }),
            Err(e) => Ok(WaitForAuthResponse {
                success: false,
                token: "".to_string(),
                error_message: Some(e.to_string()),
            }),
        }
    }

    #[rbac]
    // get_token has no rbac
    async fn get_token(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: GetTokenRequest,
    ) -> Result<GetTokenResponse, CowenError> {
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => {
                return Ok(GetTokenResponse {
                    token_json: "".to_string(),
                    error_message: Some(format!("Profile not found: {}", e)),
                })
            }
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let res = if req.refresh {
            auth_cli
                .refresh_token(&req.profile, &config, &reqwest::header::HeaderMap::new())
                .await
        } else {
            auth_cli
                .get_token(&req.profile, &config, &reqwest::header::HeaderMap::new())
                .await
        };
        match res {
            Ok(t) => Ok(GetTokenResponse {
                token_json: serde_json::to_string(&t).unwrap_or_default(),
                error_message: None,
            }),
            Err(e) => Ok(GetTokenResponse {
                token_json: "".to_string(),
                error_message: Some(e.to_string()),
            }),
        }
    }

    #[rbac]
    async fn clear_token(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: ClearTokenRequest,
    ) -> Result<ClearTokenResponse, CowenError> {
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => {
                return Ok(ClearTokenResponse {
                    success: false,
                    message: format!("Profile not found: {}", e),
                })
            }
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        match auth_cli.clear_token(&req.profile, &config).await {
            Ok(_) => Ok(ClearTokenResponse {
                success: true,
                message: "Token cleared".to_string(),
            }),
            Err(e) => Ok(ClearTokenResponse {
                success: false,
                message: e.to_string(),
            }),
        }
    }
}

fn parse_and_merge_config(
    profile: &str,
    json_val: &Option<serde_json::Value>,
    old_config: &Option<cowen_common::Config>,
) -> cowen_common::Config {
    let mut config = if let Some(ref old) = old_config {
        let mut cfg = cowen_common::Config::default_with_profile(profile);
        cfg.version = old.version;
        cfg
    } else {
        cowen_common::Config::default_with_profile(profile)
    };

    if let Some(ref val) = json_val {
        let mut target_val = serde_json::to_value(&config).unwrap_or_default();
        deep_merge(&mut target_val, val);
        if let Ok(mut parsed_config) = serde_json::from_value::<cowen_common::Config>(target_val) {
            if let Some(ref old) = old_config {
                parsed_config.version = old.version;
            }
            config = parsed_config;
        }
    } else if let Some(ref old) = old_config {
        config = old.clone();
    }
    config
}

fn extract_and_inherit_secrets(
    config: &mut cowen_common::Config,
    json_val: &Option<serde_json::Value>,
    old_config: &Option<cowen_common::Config>,
    req: &InitProfileRequest,
) {
    if let Some(ref val) = json_val {
        if let Some(as_) = val.get("app_secret").and_then(|v| v.as_str()) {
            if !as_.is_empty() {
                config.app_secret = as_.to_string();
            }
        }
        if let Some(cert) = val.get("certificate").and_then(|v| v.as_str()) {
            if !cert.is_empty() {
                config.certificate = cert.to_string();
            }
        }
        if let Some(ek) = val.get("encrypt_key").and_then(|v| v.as_str()) {
            if !ek.is_empty() {
                config.encrypt_key = ek.to_string();
            }
        }
    }
    if let Some(as_) = &req.app_secret {
        config.app_secret = as_.clone();
    }
    if let Some(ek) = &req.encrypt_key {
        config.encrypt_key = ek.clone();
    }
    if let Some(cert) = &req.certificate {
        config.certificate = cert.clone();
    }
    if let Some(ref old) = old_config {
        if config.app_secret.is_empty() {
            config.app_secret = old.app_secret.clone();
        }
        if config.certificate.is_empty() {
            config.certificate = old.certificate.clone();
        }
        if config.encrypt_key.is_empty() {
            config.encrypt_key = old.encrypt_key.clone();
        }
    }
}
