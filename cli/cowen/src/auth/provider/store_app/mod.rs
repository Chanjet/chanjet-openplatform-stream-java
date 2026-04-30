use crate::auth::client::HttpSender;
use crate::auth::lifecycle::AuthSessionManager;
use crate::auth::models::{OAuth2TokenPair, Token};
use crate::auth::pool::TokenPool;
use crate::auth::provider::AuthProvider;
use crate::core::config::Config;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use std::sync::Arc;

pub mod models;
pub mod diagnostics;
pub mod client;
pub mod token_logic;
pub mod storage;
#[cfg(test)]
pub mod tests;

use models::StoreAppTokenResponse;

pub struct StoreAppProvider<'a> {
    pool: &'a (dyn TokenPool + Send + Sync),
    http_sender: Arc<dyn HttpSender>,
}

impl<'a> StoreAppProvider<'a> {
    pub fn new(pool: &'a (dyn TokenPool + Send + Sync), http_sender: Arc<dyn HttpSender>) -> Self {
        Self { pool, http_sender }
    }

    async fn refresh_token(
        &self,
        profile: &str,
        cfg: &Config,
        refresh_token: &str,
    ) -> Result<Token> {
        client::refresh_token(self.pool, self.http_sender.as_ref(), profile, cfg, refresh_token).await
    }

    pub async fn intercept_exchange(
        &self,
        profile: &str,
        cfg: &Config,
        body_bytes: &[u8],
    ) -> Result<serde_json::Value> {
        client::intercept_exchange(self.pool, self.http_sender.as_ref(), profile, cfg, body_bytes).await
    }

    pub async fn get_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token> {
        client::get_app_access_token(self.pool, self.http_sender.as_ref(), profile, cfg).await
    }

    pub async fn exchange_permanent_code_by_temp_code(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
        temp_auth_code: &str,
    ) -> Result<String> {
        client::exchange_permanent_code_by_temp_code(self.pool, self.http_sender.as_ref(), profile, cfg, org_id, temp_auth_code).await
    }

    async fn request_token(
        &self,
        profile: &str,
        url: &str,
        body: serde_json::Value,
        cfg: &Config,
    ) -> Result<Token> {
        client::request_token(self.pool, self.http_sender.as_ref(), profile, url, body, cfg).await
    }

    pub async fn get_user_token(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
        user_id: &str,
    ) -> Result<Token> {
        token_logic::get_user_token(self.pool, self.http_sender.as_ref(), profile, cfg, org_id, user_id).await
    }

    pub async fn get_org_token(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
    ) -> Result<Token> {
        token_logic::get_org_token(self.pool, self.http_sender.as_ref(), profile, cfg, org_id).await
    }
}

#[async_trait]
impl<'a> AuthProvider for StoreAppProvider<'a> {
    async fn get_token(
        &self,
        profile: &str,
        cfg: &Config,
        headers: &reqwest::header::HeaderMap,
    ) -> Result<Token> {
        token_logic::get_token(self.pool, self.http_sender.as_ref(), profile, cfg, headers).await
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
        // 1. Check for short-circuit interception
        if path.ends_with("/oauth2/token") && method == "POST" {
            let json_resp = self.intercept_exchange(profile, config, body).await?;
            return Ok(crate::auth::provider::ProxyRequestAction::Respond(
                json_resp,
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
    ) -> Result<()> {
        Ok(())
    }

    async fn initialize(
        &self,
        profile: &str,
        config: &Config,
        vault: std::sync::Arc<dyn crate::core::vault::Vault>,
        cfg_mgr: &crate::core::config::ConfigManager,
    ) -> Result<()> {
        // Validation: appKey, appSecret and encryptKey are required for sidecar
        if config.app_key.trim().is_empty()
            || config.app_secret.trim().is_empty()
            || config.encrypt_key.trim().is_empty()
        {
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
        println!("🚀 Sidecar is ready. Starting background daemon...");

        let _ = crate::cmd::system::ensure_daemon_running(profile, config, cfg_mgr, vault).await;
        Ok(())
    }

    async fn on_maintenance_tick(&self, profile: &str, config: &Config) -> Result<()> {
        // Routine maintenance for Store App: Maintain the AppAccessToken (SuiteAccessToken)
        match self.get_app_access_token(profile, config).await {
            Ok(token) => {
                let remaining = token.expires_at.signed_duration_since(Utc::now());
                if remaining < Duration::minutes(15) {
                    tracing::info!(target: "sys", "Store App SuiteAccessToken expires in less than 15 mins. Proactively refreshing...");
                    let _ = self.get_app_access_token(profile, config).await;
                }
            }
            Err(e) => {
                tracing::warn!(target: "sys", error = %e, "Store App SuiteAccessToken is missing or invalid. Waiting for platform push.");
            }
        }
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
                    created_at: Utc::now(),
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
    async fn finalize_login(&self, profile: &str, cfg: &Config) -> Result<()> {
        tracing::info!(target: "sys", profile = %profile, "Finalizer started for StoreApp auth");
        
        let session_manager = AuthSessionManager::new(self.pool);
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

    async fn get_status_entries(&self, profile: &str, config: &Config) -> Result<Vec<crate::core::status::StatusEntry>> {
        diagnostics::get_status_entries(self.pool, profile, config).await
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
