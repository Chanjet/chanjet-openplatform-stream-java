use crate::auth::client::HttpSender;
use crate::auth::lifecycle::AuthSessionManager;
use crate::auth::models::{OAuth2TokenPair, Token};
use crate::auth::pool::TokenPool;
use crate::auth::provider::AuthProvider;
use crate::core::config::Config;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;

pub mod models;
pub mod diagnostics;
pub mod client;
pub mod token_logic;
pub mod storage;
#[cfg(test)]
pub mod tests;


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
    ) -> Result<Token> {
        client::refresh_token(self.pool.as_ref(), self.http_sender.as_ref(), profile, cfg, refresh_token).await
    }

    pub async fn intercept_exchange(
        &self,
        profile: &str,
        cfg: &Config,
        body_bytes: &[u8],
    ) -> Result<serde_json::Value> {
        client::intercept_exchange(self.pool.as_ref(), self.http_sender.as_ref(), profile, cfg, body_bytes).await
    }



    pub async fn exchange_permanent_code_by_temp_code(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
        temp_auth_code: &str,
    ) -> Result<String> {
        client::exchange_permanent_code_by_temp_code(self.pool.as_ref(), self.http_sender.as_ref(), profile, cfg, org_id, temp_auth_code).await
    }

    #[allow(dead_code)]
    pub async fn get_user_token(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
        user_id: &str,
    ) -> Result<Token> {
        token_logic::get_user_token(self.pool.as_ref(), self.http_sender.as_ref(), profile, cfg, org_id, user_id).await
    }

    #[allow(dead_code)]
    pub async fn get_org_token(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
    ) -> Result<Token> {
        token_logic::get_org_token(self.pool.as_ref(), self.http_sender.as_ref(), profile, cfg, org_id).await
    }

    async fn finalize_login(&self, profile: &str, cfg: &Config) -> Result<()> {
        tracing::info!(target: "sys", profile = %profile, "Finalizer started for StoreApp auth");
        
        let session_manager = AuthSessionManager::new(self.pool.as_ref());
        let session = session_manager.get_session(profile).await?;
        
        let (actual_port, rx) = crate::auth::lifecycle::listener::OAuth2CallbackListener::start(session.redirect_port, profile.to_string()).await?;
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
                            Err(e) => Err(anyhow::anyhow!("Authorization failed: {}", e))
                        }
                    }
                    Err(e) => Err(anyhow::anyhow!("Internal listener error: {}", e))
                }
            },
            _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
                Err(anyhow::anyhow!("Timeout waiting for authorization (5 mins)"))
            }
        };

        if res.is_err() {
            let _ = session_manager.clear(profile).await;
        }
        res
    }
}

#[async_trait]
impl AuthProvider for StoreAppProvider {
    async fn exchange_temp_code(&self, profile: &str, config: &Config, org_id: &str, temp_code: &str) -> Result<Token> {
        let _ = self.exchange_permanent_code_by_temp_code(profile, config, org_id, temp_code).await?;
        self.get_org_token(profile, config, org_id).await
    }

    async fn get_user_token(&self, profile: &str, config: &Config, org_id: &str, user_id: &str) -> Result<Token> {
        self.get_user_token(profile, config, org_id, user_id).await
    }

    async fn intercept_exchange(&self, profile: &str, config: &Config, body: &[u8]) -> Result<serde_json::Value> {
        self.intercept_exchange(profile, config, body).await
    }

    async fn get_app_access_token(&self, profile: &str, config: &Config) -> Result<Token> {
        client::get_app_access_token(self.pool.as_ref(), self.http_sender.as_ref(), profile, config).await
    }

    async fn get_token(
        &self,
        profile: &str,
        cfg: &Config,
        headers: &reqwest::header::HeaderMap,
    ) -> Result<Token> {
        token_logic::get_token(self.pool.as_ref(), self.http_sender.as_ref(), profile, cfg, headers).await
    }

    async fn refresh(
        &self,
        profile: &str,
        cfg: &Config,
        _headers: &reqwest::header::HeaderMap,
    ) -> Result<Token> {
        let pair_str = self
            .pool
            .as_vault()
            .get(profile, "oauth2_token_pair")
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
    ) -> Result<crate::auth::provider::ProxyRequestAction> {
        // 1. Check for short-circuit interception (OAuth2 Token Exchange)
        if path.ends_with("/oauth2/token") && method == "POST" {
            let json_resp = self.intercept_exchange(profile, config, body).await?;
            return Ok(crate::auth::provider::ProxyRequestAction::Respond(
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
                        self.handle_platform_event(profile, config, crate::auth::provider::PlatformEvent::AppTicket(ticket)).await?;
                    }
                }
                "TEMP_AUTH_CODE" => {
                    let code = data.get("temp_auth_code").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    if code.is_empty() {
                        tracing::warn!(target: "sys", "Received TEMP_AUTH_CODE event with empty code value, ignoring");
                    } else {
                        let org_id = data.get("org_id").and_then(|v| v.as_str()).map(|s| s.to_string());
                        self.handle_platform_event(profile, config, crate::auth::provider::PlatformEvent::TempAuthCode { code, org_id }).await?;
                    }
                }
                _ => {
                    tracing::debug!(target: "sys", "StoreApp Webhook received unknown event type: {}", event_type);
                }
            }
            return Ok(crate::auth::provider::ProxyRequestAction::Respond(
                serde_json::json!({"code": "200", "message": "success"})
            ));
        }

        // 2. Not intercepted, proceed with normal forwarding logic
        let token = self.get_token(profile, config, &headers).await?;

        // 3. Decorate headers
        let auth_headers = crate::auth::RequestDecorator::get_auth_headers(
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

        Ok(crate::auth::provider::ProxyRequestAction::Forward { headers })
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
    ) -> Result<Option<serde_json::Value>> {
        Ok(None)
    }

    async fn on_maintenance_tick(&self, profile: &str, config: &Config) -> Result<()> {
        // 1. Check AppAccessToken health
        match self.pool.get_app_access_token(&config.app_key).await {
            Ok(token) => {
                let remaining = token.expires_at.signed_duration_since(chrono::Utc::now());
                if remaining < chrono::Duration::minutes(15) {
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
        if let Ok(pair_str) = self.pool.as_vault().get(profile, "oauth2_token_pair").await {
            if let Ok(pair) = serde_json::from_str::<OAuth2TokenPair>(&pair_str) {
                let remaining = pair.expires_at.signed_duration_since(chrono::Utc::now());
                if remaining < chrono::Duration::minutes(15) {
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
        vault: std::sync::Arc<dyn crate::core::vault::Vault>,
        cfg_mgr: &crate::core::config::ConfigManager,
        params: crate::auth::provider::InitParams,
    ) -> Result<()> {
        // 1. Setup credentials
        if let Some(ak) = params.app_key {
            config.app_key = ak;
        }
        if let Some(as_val) = params.app_secret {
            vault.set_secret(profile, "app_secret", &as_val).await?;
            config.app_secret = as_val;
        }
        if let Some(ek) = params.encrypt_key {
            vault.set_secret(profile, "encrypt_key", &ek).await?;
            config.encrypt_key = ek;
        }
        if let Some(url) = params.openapi_url {
            config.openapi_url = url;
        }
        if let Some(url) = params.stream_url {
            config.stream_url = url;
        }
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
            let bin_name = crate::core::utils::get_bin_name();
            println!("Error: --app-key, --app-secret, and --encrypt-key are required for store-app (sidecar) mode.");
            println!(
                "Example: {} init --app-mode store-app --app-key X --app-secret Y --encrypt-key Z",
                bin_name
            );
            return Err(anyhow!("Missing required credentials for StoreApp mode"));
        }

        println!(
            "✅ Profile '{}' initialized successfully (Sidecar Mode).",
            profile
        );
        println!("💡 Please perform authorization through your main application.");
        if params.auto_start {
            println!("🚀 Sidecar is ready. Starting background daemon...");
            let _ = crate::cmd::daemon::start(
                profile, 
                config, 
                config.proxy_port, 
                config.proxy_enabled, 
                false, 
                false, 
                cfg_mgr, 
                vault
            ).await;
        }
        Ok(())
    }

    async fn trigger_push(&self, _profile: &str, config: &Config, _force: bool) -> Result<()> {
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

    async fn hydrate_config(&self, profile: &str, config: &mut Config, vault: std::sync::Arc<dyn crate::core::vault::Vault>) -> Result<()> {
        if let Ok(as_val) = vault.get_secret(profile, "app_secret").await { config.app_secret = as_val; }
        if let Ok(ek) = vault.get_secret(profile, "encrypt_key").await { config.encrypt_key = ek; }
        Ok(())
    }



    fn requires_initial_push(&self, _config: &Config) -> bool {
        false // Store App usually waits for platform events passively
    }

    async fn handle_platform_event(&self, profile: &str, config: &Config, event: crate::auth::provider::PlatformEvent) -> Result<()> {
        match event {
            crate::auth::provider::PlatformEvent::AppTicket(ticket_val) => {
                let ticket = crate::auth::models::Ticket {
                    value: ticket_val,
                    created_at: chrono::Utc::now(),
                };
                self.pool.set_app_ticket(&config.app_key, &ticket).await?;
                tracing::info!(target: "sys", "Store App AppTicket updated from platform push");
                Ok(())
            }
            crate::auth::provider::PlatformEvent::TempAuthCode { code, org_id } => {
                tracing::info!(target: "sys", "Store App TEMP_AUTH_CODE received. Exchanging...");
                let oid = org_id.ok_or_else(|| anyhow!("Missing org_id in TempAuthCode event for Store App"))?;
                if let Err(e) = self.exchange_permanent_code_by_temp_code(profile, config, &oid, &code).await {
                    tracing::error!(target: "sys", error = %e, orgId = %oid, "Failed to exchange TEMP_AUTH_CODE for Store App");
                    return Err(e);
                }
                Ok(())
            }
        }
    }

    async fn perform_login(&self, profile: &str, config: &Config, _force: bool, finalize: Option<&str>) -> Result<()> {
        // 1. Finalizer Implementation (Background flow)
        if let Some(_session_id) = finalize {
            return self.finalize_login(profile, config).await;
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

    async fn get_diagnostics(&self, ctx: &crate::core::status::StatusContext<'_>) -> Result<Vec<crate::core::status::StatusEntry>> {
        use crate::core::status::{collect_daemon_status, StatusEntry, StatusLevel, CommonTemplate};
        let mut results = Vec::new();
        
        // 1. Auth Status (from diagnostics module)
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
        let (found_pid, _) = crate::core::status::get_active_daemon_info(&ctx.profile).await;
        let is_running = found_pid.is_some();
        let (display_name, efficiency_tip) = self.get_daemon_display_info(is_running);
        results.push(collect_daemon_status(ctx, &display_name, &efficiency_tip, self.supports_webhooks()).await?);

        Ok(results)
    }

    fn get_default_app_key(&self) -> Option<String> {
        Some(crate::auth::models::BUILTIN_CLIENT_ID.to_string())
    }

    fn decorate_openapi_request(&self, _url: &mut String, headers: &mut reqwest::header::HeaderMap, token: &Token, config: &Config) {
        headers.insert("openToken", token.value.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
        headers.insert("appKey", config.app_key.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
    }

    async fn on_logout(&self, profile: &str, _config: &Config) -> Result<()> {
        let vault = self.pool.as_vault();
        let _ = vault.delete(profile, "oauth2_token_pair");
        let _ = vault.delete(profile, "pending_auth_session");
        let _ = vault.delete(profile, "captured_auth_code");
        let _ = vault.delete(profile, "oauth2_revoked");
        let _ = vault.delete(profile, "last_refresh_error");
        let _ = vault.delete(profile, "app_ticket");
        let _ = vault.delete(profile, "app_ticket_created");
        Ok(())
    }
}
