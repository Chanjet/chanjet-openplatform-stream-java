#![allow(dead_code, unused_imports, unused_variables)]
use cowen_auth::client::Client;
use cowen_common::daemon::DaemonService;
use cowen_common::grpc::proto::*;
use cowen_common::vault::Vault;
use cowen_common::CowenError;
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

        let mode_str = req.app_mode.clone().unwrap_or_else(|| "oauth2".to_string());
        let mode = match mode_str.parse::<cowen_common::models::AuthMode>() {
            Ok(m) => m,
            Err(e) => return Err(CowenError::config(e.to_string())),
        };

        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let provider = auth_cli.provider(&mode);

        if let Some(resp) = self.check_conflicting_profile(&req, provider).await {
            return Ok(resp);
        }

        let mut config = self
            .cfg_mgr
            .load(&req.profile)
            .await
            .unwrap_or_else(|_| cowen_common::Config::default_with_profile(&req.profile));
        Self::apply_req_to_config(&req, &mut config, &mode);

        let mut app_config: cowen_common::config::AppConfig =
            self.cfg_mgr.load_app_config().await.unwrap_or_default();
        if let Some(url) = &req.openapi_url {
            app_config.openapi_url = url.clone();
        }
        if let Some(url) = &req.stream_url {
            app_config.stream_url = url.clone();
        }
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

        if !req.force && config.app_mode == cowen_common::models::AuthMode::Oauth2 {
            if let Ok(rt) = self.vault.get_refresh_token(&req.profile).await {
                if !rt.is_expired() {
                    if let Ok(_token) = auth_cli
                        .refresh_token(&req.profile, &config, &reqwest::header::HeaderMap::new())
                        .await
                    {
                        return Ok(GetAuthUrlResponse {
                            success: true,
                            url: "rotated".to_string(),
                            state: "".to_string(),
                            error_message: None,
                        });
                    }
                }
            }
        }

        if config.app_mode == cowen_common::models::AuthMode::SelfBuilt {
            match auth_cli
                .get_token(&req.profile, &config, &reqwest::header::HeaderMap::new())
                .await
            {
                Ok(t) => {
                    return Ok(GetAuthUrlResponse {
                        success: true,
                        url: t.value,
                        state: "direct".to_string(),
                        error_message: None,
                    })
                }
                Err(e) => {
                    return Ok(GetAuthUrlResponse {
                        success: false,
                        url: "".to_string(),
                        state: "".to_string(),
                        error_message: Some(e.to_string()),
                    })
                }
            }
        }

        match provider
            .generate_auth_url(
                &req.profile,
                &mut config,
                self.vault.clone(),
                &self.cfg_mgr,
                cowen_auth::provider::InitParams {
                    app_key: None,
                    app_secret: None,
                    certificate: None,
                    encrypt_key: None,
                    openapi_url: None,
                    stream_url: None,
                    webhook_target: None,
                    proxy_port: None,
                    auto_start: false,
                    is_new: false,
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
