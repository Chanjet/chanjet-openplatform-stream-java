use cowen_common::{CowenResult, CowenError};
use async_trait::async_trait;
use cowen_common::daemon::DaemonService;
use crate::client::HttpSender;
use crate::lifecycle::AuthSessionManager;
use crate::models::{OAuth2TokenPair, Token};
use crate::pool::TokenPool;
use crate::provider::AuthProvider;
use cowen_common::config::Config;

use std::sync::Arc;

pub mod models;
pub mod diagnostics;
pub mod client;
pub mod token_logic;
pub mod storage;


pub struct StoreAppProvider {
    pool: Arc<dyn TokenPool>,
    http_sender: Arc<dyn HttpSender>,
}

impl StoreAppProvider {
    pub fn new(pool: Arc<dyn TokenPool>, http_sender: Arc<dyn HttpSender>) -> Self {
        Self { pool, http_sender }
    }

    async fn refresh_token(
        &self,
        profile: &str,
        cfg: &Config,
        refresh_token: &str,
    ) -> CowenResult<cowen_common::models::Token> {
        client::refresh_token(self.pool.as_ref(), self.http_sender.as_ref(), profile, cfg, refresh_token).await
    }

    pub async fn intercept_exchange(
        &self,
        profile: &str,
        cfg: &Config,
        body_bytes: &[u8],
    ) -> CowenResult<serde_json::Value> {
        client::intercept_exchange(self.pool.as_ref(), self.http_sender.as_ref(), profile, cfg, body_bytes).await
    }



    pub async fn exchange_permanent_code_by_temp_code(
        &self,
        profile: &str,
        cfg: &Config,
        temp_auth_code: &str,
    ) -> CowenResult<String> {
        client::exchange_permanent_code_by_temp_code(self.pool.as_ref(), self.http_sender.as_ref(), profile, cfg, temp_auth_code).await
    }

    #[allow(dead_code)]
    pub async fn get_user_token(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
        user_id: &str,
    ) -> CowenResult<cowen_common::models::Token> {
        token_logic::get_user_token(self.pool.as_ref(), self.http_sender.as_ref(), profile, cfg, org_id, user_id).await
    }

    #[allow(dead_code)]
    pub async fn get_org_token(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
    ) -> CowenResult<cowen_common::models::Token> {
        token_logic::get_org_token(self.pool.as_ref(), self.http_sender.as_ref(), profile, cfg, org_id).await
    }

    async fn finalize_login(&self, profile: &str, cfg: &Config, session_id: &str) -> CowenResult<()> {
        tracing::info!(target: "sys", profile = %profile, session_id = %session_id, "Finalizer started for StoreApp auth");
        
        let session_manager = AuthSessionManager::new(self.pool.as_ref());
        let session = session_manager.get_session(session_id).await?;
        
        let (actual_port, rx) = crate::lifecycle::listener::OAuth2CallbackListener::start(session.redirect_port, profile.to_string()).await?;
        tracing::info!(target: "sys", port = %actual_port, "Finalizer listening for callback");

        let res = tokio::select! {
            result = rx => {
                match result {
                    Ok(inner_res) => {
                        match inner_res {
                            Ok(res) => {
                                tracing::info!(target: "sys", "Callback received, saving code...");
                                session_manager.save_code(profile, &res.code, &res.state).await?;
                                
                                // Trigger exchange
                                match self.get_app_access_token(profile, cfg).await {
                                    Ok(_) => {
                                        tracing::info!(target: "sys", "Token exchange successful");
                                        Ok(())
                                    }
                                    Err(e) => {
                                        tracing::error!(target: "sys", error = %e, "Token exchange failed");
                                        Err(e)
                                    }
                                }
                            }
                            Err(e) => Err(CowenError::Auth(format!("Authorization failed: {}", e)))
                        }
                    }
                    Err(e) => Err(CowenError::Auth(format!("Internal listener error: {}", e)))
                }
            },
            _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
                Err(CowenError::Auth("Timeout waiting for authorization (5 mins)".to_string()))
            }
        };

        if res.is_err() {
            let _ = session_manager.clear(profile).await;
        }
        res
    }
}

#[async_trait]
#[async_trait]
impl AuthProvider for StoreAppProvider {
    async fn exchange_temp_code(&self, profile: &str, config: &Config, org_id: &str, temp_code: &str) -> CowenResult<cowen_common::models::Token> {
        let _ = self.exchange_permanent_code_by_temp_code(profile, config, temp_code).await?;
        self.get_org_token(profile, config, org_id).await
    }

    async fn get_user_token(&self, profile: &str, config: &Config, org_id: &str, user_id: &str) -> CowenResult<cowen_common::models::Token> {
        self.get_user_token(profile, config, org_id, user_id).await
    }

    async fn intercept_exchange(&self, profile: &str, config: &Config, body: &[u8]) -> CowenResult<serde_json::Value> {
        self.intercept_exchange(profile, config, body).await
    }

    async fn get_app_access_token(&self, profile: &str, config: &Config) -> CowenResult<cowen_common::models::Token> {
        client::get_app_access_token(self.pool.as_ref(), self.http_sender.as_ref(), profile, config).await
    }

    async fn get_token(
        &self,
        profile: &str,
        cfg: &Config,
        headers: &reqwest::header::HeaderMap,
    ) -> CowenResult<cowen_common::models::Token> {
        token_logic::get_token(self.pool.as_ref(), self.http_sender.as_ref(), profile, cfg, headers).await
    }

    async fn refresh(
        &self,
        profile: &str,
        cfg: &Config,
        _headers: &reqwest::header::HeaderMap,
    ) -> CowenResult<cowen_common::models::Token> {
        let pair_str = self
            .pool
            .as_vault()
            .get_secret(profile, "oauth2_token_pair")
            .await?;
        let pair: OAuth2TokenPair = serde_json::from_str(&pair_str)?;
        self.refresh_token(profile, cfg, &pair.refresh_token).await
    }

    async fn intercept_request(
        &self,
        profile: &str,
        config: &Config,
        path: &str,
        method: &str,
        mut headers: reqwest::header::HeaderMap,
        body: &[u8],
        spec: &serde_json::Value,
    ) -> CowenResult<crate::provider::ProxyRequestAction> {
        // 1. Check for short-circuit interception (OAuth2 Token Exchange)
        if path.ends_with("/oauth2/token") && method == "POST" {
            let json_resp = self.intercept_exchange(profile, config, body).await?;
            return Ok(crate::provider::ProxyRequestAction::Respond(
                json_resp,
            ));
        }

        // 1.5. Webhook Receiver Interception
        if path.ends_with("/webhook") && method == "POST" {
            tracing::info!(target: "sys", path = %path, "StoreApp Proxy intercepted webhook request");
            let data: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();
            let event_type = data.get("type").and_then(|v| v.as_str()).unwrap_or_default();
            tracing::info!(target: "sys", event_type = %event_type, "StoreApp Webhook event type identified");
            
            match event_type {
                "APP_TICKET" => {
                    let ticket = data.get("app_ticket").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    if ticket.is_empty() {
                        tracing::warn!(target: "sys", "Received APP_TICKET event with empty ticket value, ignoring");
                    } else {
                        self.handle_platform_event(profile, config, crate::provider::PlatformEvent::AppTicket(ticket)).await?;
                    }
                }
                "TEMP_AUTH_CODE" => {
                    let code = data.get("temp_auth_code").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    if code.is_empty() {
                        tracing::warn!(target: "sys", "Received TEMP_AUTH_CODE event with empty code value, ignoring");
                    } else {
                        let state = data.get("state").and_then(|v| v.as_str()).map(|s| s.to_string());
                        self.handle_platform_event(profile, config, crate::provider::PlatformEvent::TempAuthCode { code, state }).await?;
                    }
                }
                _ => {
                    tracing::debug!(target: "sys", "StoreApp Webhook received unknown event type: {}", event_type);
                }
            }
            return Ok(crate::provider::ProxyRequestAction::Respond(
                serde_json::json!({"code": "200", "message": "success"})
            ));
        }

        // 2. Not intercepted, proceed with normal forwarding logic
        let token = self.get_token(profile, config, &headers).await?;

        // 3. Decorate headers
        let auth_headers = crate::RequestDecorator::get_auth_headers(
            spec,
            path,
            method,
            &config.app_key,
            &config.app_secret,
            &token.value,
        );

        for (name, value) in auth_headers {
            if let Ok(name) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) {
                if let Ok(val) = reqwest::header::HeaderValue::from_bytes(value.as_bytes()) {
                    headers.insert(name, val);
                }
            }
        }

        Ok(crate::provider::ProxyRequestAction::Forward { headers })
    }

    async fn intercept_response(
        &self,
        _profile: &str,
        _config: &Config,
        _path: &str,
        _method: &str,
        _status: u16,
        _response_headers: &reqwest::header::HeaderMap,
        _response_body: &[u8],
    ) -> CowenResult<Option<serde_json::Value>> {
        Ok(None)
    }

    async fn on_maintenance_tick(&self, profile: &str, config: &Config) -> CowenResult<()> {
        // 1. Check AppAccessToken health
        match self.pool.get_app_access_token(&config.app_key).await {
            Ok(token) => {
                if token.is_expired_with_buffer(chrono::Duration::minutes(15)) {
                    let remaining = token.expires_at.signed_duration_since(chrono::Utc::now());
                    tracing::info!(target: "sys", "StoreApp AppAccessToken expires in {:?}. Proactively refreshing...", remaining);
                    let _ = self.get_app_access_token(profile, config).await;
                }
            }
            Err(_) => {
                // If missing, try to get it (this will fail if appTicket is missing, but that's handled inside)
                let _ = self.get_app_access_token(profile, config).await;
            }
        }

        // 2. Check archived OAuth2 tokens health (Multi-tenant support)
        // For simplicity, we just look at the 'main' pair if archived
        if let Ok(pair_str) = self.pool.as_vault().get_secret(profile, "oauth2_token_pair").await {
            if let Ok(pair) = serde_json::from_str::<OAuth2TokenPair>(&pair_str) {
                if pair.is_expired_with_buffer(chrono::Duration::minutes(15)) {
                    let remaining = pair.expires_at.signed_duration_since(chrono::Utc::now());
                    tracing::info!(target: "sys", "StoreApp archived OAuth2 token expires in {:?}. Proactively refreshing...", remaining);
                    let _ = self.refresh(profile, config, &Default::default()).await;
                }
            }
        }
        
        Ok(())
    }

    async fn initialize(
        &self,
        profile: &str,
        config: &mut Config,
        vault: std::sync::Arc<dyn cowen_common::vault::Vault>,
        cfg_mgr: &cowen_config::ConfigManager,
        params: crate::provider::InitParams,
        daemon_service: Option<std::sync::Arc<dyn DaemonService>>,
    ) -> CowenResult<()> {
        // 1. Setup credentials
        if let Some(ak) = params.app_key { config.app_key = ak; }
        if let Some(as_val) = params.app_secret { config.app_secret = as_val; }
        if let Some(ek) = params.encrypt_key { config.encrypt_key = ek; }
        if let Some(url) = params.openapi_url { config.openapi_url = url; }
        if let Some(url) = params.stream_url { config.stream_url = url; }

        if config.app_key.trim().is_empty() { return Err(CowenError::Config("Missing mandatory parameter: --app-key".to_string())); }
        if config.app_secret.trim().is_empty() { return Err(CowenError::Config("Missing mandatory parameter: --app-secret".to_string())); }
        if config.encrypt_key.trim().is_empty() { return Err(CowenError::Config("Missing mandatory parameter: --encrypt-key".to_string())); }

        let app_key = config.app_key.trim();
        let global_profile = format!("app:{}", app_key);
        
        vault.set_secret(&global_profile, "app_secret", &config.app_secret).await?;
        vault.set_secret(&global_profile, "encrypt_key", &config.encrypt_key).await?;
        if let Some(target) = params.webhook_target {
            config.webhook_target = target;
        }
        if let Some(port) = params.proxy_port {
            config.proxy_port = port;
        }

        // 1.1 Use is_new from params (as Init already anchored the identity)
        let is_new = params.is_new;

        // 2. Persist config so daemon can see it
        cfg_mgr.save(profile, config).await?;

        // 3. Validation and Startup
        // Validation: appKey, appSecret and encryptKey are required for sidecar
        if config.app_key.trim().is_empty()
            || config.app_secret.trim().is_empty()
            || config.encrypt_key.trim().is_empty()
        {
            if is_new {
                let _ = cfg_mgr.delete(profile).await;
            }
            let bin_name = cowen_common::utils::get_bin_name();
            println!("Error: --app-key, --app-secret, and --encrypt-key are required for store-app (sidecar) mode.");
            println!(
                "Example: {} init --app-mode store-app --app-key X --app-secret Y --encrypt-key Z",
                bin_name
            );
            return Err(CowenError::Auth(format!("Missing required credentials for StoreApp mode")));
        }

        println!(
            "✅ Profile '{}' initialized successfully (Sidecar Mode).",
            profile
        );
        println!("💡 Please perform authorization through your main application.");
        if params.auto_start {
            println!("🚀 Sidecar is ready. Starting background daemon...");
            if let Some(ds) = &daemon_service { let _ = ds.start_daemon(profile, config, vault.clone()).await; }
        }
        Ok(())
    }

    async fn trigger_push(&self, _profile: &str, config: &Config, _force: bool) -> CowenResult<()> {
        let url = format!(
            "{}/auth/appTicket/resend",
            config.openapi_url.trim_end_matches('/')
        );
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("appKey", config.app_key.trim().parse()?);
        headers.insert("appSecret", config.app_secret.trim().parse()?);
        
        let body = serde_json::json!({});
        let _ = self.http_sender.post(&url, headers, body).await?;
        Ok(())
    }

    async fn hydrate_config(&self, profile: &str, config: &mut Config, vault: std::sync::Arc<dyn cowen_common::vault::Vault>) -> CowenResult<()> {
        if let Ok(as_val) = vault.get_secret(profile, "app_secret").await { config.app_secret = as_val; }
        if let Ok(ek) = vault.get_secret(profile, "encrypt_key").await { config.encrypt_key = ek; }
        Ok(())
    }



    async fn requires_initial_push(&self, config: &Config) -> bool {
        // Check if ticket is missing or older than 50 minutes
        if let Ok(ticket) = self.pool.get_app_ticket(&config.app_key).await {
            let age = chrono::Utc::now().signed_duration_since(ticket.created_at).num_minutes();
            age > 50
        } else {
            true
        }
    }

    async fn handle_platform_event(&self, profile: &str, config: &Config, event: crate::provider::PlatformEvent) -> CowenResult<()> {
        match event {
            crate::provider::PlatformEvent::AppTicket(ticket_val) => {
                let ticket = cowen_common::models::Ticket {
                    value: ticket_val,
                    created_at: chrono::Utc::now(),
                };
                self.pool.set_app_ticket(&config.app_key, &ticket).await?;
                tracing::info!(target: "sys", "Store App AppTicket updated from platform push");
                Ok(())
            }
            crate::provider::PlatformEvent::TempAuthCode { code, state: _ } => {
                tracing::info!(target: "sys", "Store App TEMP_AUTH_CODE received. Exchanging...");
                if let Err(e) = self.exchange_permanent_code_by_temp_code(profile, config, &code).await {
                    tracing::error!(target: "sys", error = %e, "Failed to exchange TEMP_AUTH_CODE for Store App");
                    return Err(e);
                }
                Ok(())
            }
        }
    }

    async fn perform_login(&self, profile: &str, config: &Config, _force: bool, finalize: Option<&str>, _daemon_service: Option<std::sync::Arc<dyn cowen_common::daemon::DaemonService>>) -> CowenResult<()> {
        // 1. Finalizer Implementation (Background flow)
        if let Some(session_id) = finalize {
            return self.finalize_login(profile, config, session_id).await;
        }

        // 2. Regular Login flow
        println!("🔄 [StoreApp] Attempting to refresh token pair for profile '{}'...", profile);
        match self.refresh(profile, config, &reqwest::header::HeaderMap::new()).await {
            Ok(_) => {
                println!("✅ Success! OAuth2 Token Pair has been rotated.");
                Ok(())
            }
            Err(e) => {
                println!("❌ Refresh failed: {}", e);
                println!("💡 Suggestion: Sidecar session may have expired. Please re-authorize through your \x1b[33mMain Application\x1b[0m.");
                Err(e)
            }
        }
    }

    async fn get_diagnostics(&self, ctx: &cowen_monitor::status::StatusContext<'_>) -> CowenResult<Vec<cowen_monitor::status::StatusEntry>> {
        use cowen_monitor::status::{StatusEntry, StatusLevel, CommonTemplate, collect_daemon_status};
        
        let mut results = Vec::new();
        
        // 1. Mode Specific Diagnostics (Authentication, Vault, etc.)
        let auth_entries = diagnostics::get_diagnostics_entries(self.pool.as_ref(), &ctx.profile, ctx.config).await?;
        
        if !auth_entries.is_empty() {
            let max_level = auth_entries.iter().map(|e| e.level).max_by_key(|l| match l {
                StatusLevel::ERROR => 3,
                StatusLevel::WARN => 2,
                StatusLevel::OK => 1,
                _ => 0,
            }).unwrap_or(StatusLevel::OK);

            results.push(StatusEntry::new(CommonTemplate::ProviderSummary("Authentication Status".to_string(), "🔐".to_string()), max_level, format!("Collected {} status indicators", auth_entries.len()))
                .with_children(auth_entries));
        }

        // 2. Daemon Status
        let daemon_info = cowen_monitor::status::get_active_daemon_info(&ctx.profile);
        let (display_name, efficiency_tip) = self.get_daemon_display_info(daemon_info.is_some());
        results.push(collect_daemon_status(ctx, &display_name, &efficiency_tip, self.supports_webhooks(), daemon_info).await?);

        Ok(results)
    }

    fn get_daemon_display_info(&self, is_running: bool) -> (String, String) {
        let name = "Stream Bridge (Daemon)";
        let tip = if is_running {
            "同步状态: [ACTIVE]"
        } else {
            "若需实现多租户消息同步，请运行 'cowen daemon start'"
        };
        (name.to_string(), tip.to_string())
    }

    fn supports_api_call(&self) -> bool {
        false
    }


    fn get_default_app_key(&self) -> Option<String> {
        Some(crate::models::BUILTIN_CLIENT_ID.to_string())
    }

    fn decorate_openapi_request(&self, _url: &mut String, headers: &mut reqwest::header::HeaderMap, token: &Token, config: &Config) {
        headers.insert("openToken", token.value.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
        headers.insert("appKey", config.app_key.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
    }

    async fn on_logout(&self, profile: &str, config: &Config) -> CowenResult<()> {
        let vault = self.pool.as_vault();
        let _ = vault.delete_access_token(profile).await;
        let _ = vault.delete_refresh_token(profile).await;
        
        let _ = vault.delete_secret(profile, "oauth2_token_pair").await;
        let _ = vault.delete_config(profile, "oauth2_token_pair").await;
        
        let _ = vault.delete_config(profile, "oauth2_revoked").await;
        let _ = vault.delete_config(profile, "last_refresh_error").await;
        
        let app_key = config.app_key.trim();
        if !app_key.is_empty() {
             let _ = vault.delete_app_access_token(app_key).await;
             let _ = vault.delete_app_ticket(app_key).await;
        }
        Ok(())
    }

    async fn should_auto_recover(&self, profile: &str, config: &Config, has_pid: bool, _pid_file_exists: bool, is_distributed: bool) -> bool {
        if has_pid || config.app_key.trim().is_empty() {
            return false;
        }

        if is_distributed {
            return false;
        }

        // 🚀 OCP: For StoreApp, only auto-recover if we have the essential secrets.
        let vault = self.pool.as_vault();
        let app_key = config.app_key.trim();
        let global_profile = format!("app:{}", app_key);

        let has_secret = vault.get_secret(profile, "app_secret").await.is_ok() 
            || vault.get_secret(&global_profile, "app_secret").await.is_ok();
        
        let has_ek = vault.get_secret(profile, "encrypt_key").await.is_ok()
            || vault.get_secret(&global_profile, "encrypt_key").await.is_ok();

        has_secret && has_ek
    }
}
