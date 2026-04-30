use crate::auth::client::HttpSender;
use crate::auth::lifecycle::AuthSessionManager;
use crate::auth::models::{OAuth2TokenPair, Token};
use crate::auth::pool::TokenPool;
use crate::auth::provider::AuthProvider;
use crate::core::config::Config;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{Duration, Utc};
use fs2::FileExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::sync::Arc;

pub struct Pkce {
    pub verifier: String,
}

pub struct OAuth2Provider {
    pool: Arc<dyn TokenPool>,
    http_sender: Arc<dyn HttpSender>,
}

#[derive(Debug, Deserialize)]
struct OAuth2TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    #[serde(alias = "refresh_token_expires_in")]
    refresh_expires_in: Option<i64>,
}

impl OAuth2Provider {
    pub fn new(pool: Arc<dyn TokenPool>, http_sender: Arc<dyn HttpSender>) -> Self {
        Self { pool, http_sender }
    }

    async fn exchange_code(
        &self,
        profile: &str,
        cfg: &Config,
        code: &str,
        verifier: &str,
        redirect_uri: &str,
    ) -> Result<Token> {
        let url = format!(
            "{}{}",
            cfg.openapi_url.trim_end_matches('/'),
            obfs!("/oauth2/token")
        );
        let body = serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": crate::auth::models::BUILTIN_CLIENT_ID,
            "code": code,
            "redirect_uri": redirect_uri,
            "code_verifier": verifier,
        });

        self.request_token(profile, &url, body, cfg).await
    }

    async fn refresh_token(
        &self,
        profile: &str,
        cfg: &Config,
        refresh_token: &str,
    ) -> Result<Token> {
        let url = format!(
            "{}{}",
            cfg.openapi_url.trim_end_matches('/'),
            obfs!("/oauth2/token")
        );
        let body = serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": crate::auth::models::BUILTIN_CLIENT_ID,
            "refresh_token": refresh_token,
        });

        self.request_token(profile, &url, body, cfg).await
    }

    async fn request_token(
        &self,
        profile: &str,
        url: &str,
        body: serde_json::Value,
        _cfg: &Config,
    ) -> Result<Token> {
        let headers = reqwest::header::HeaderMap::new();
        let resp = self.http_sender.post_form(url, headers, body).await?;

        if !resp.is_success() {
            let status = resp.status;
            let err_text = resp.text();

            tracing::error!(
                target: "audit",
                profile = %profile,
                event = "token_rotate",
                status = "failure",
                error = %err_text,
                "OAuth2 token rotation failed"
            );

            // Handle specific platform error codes (Design §6)
            if err_text.contains("4029") {
                return Err(anyhow!(
                    "登录会话已超时（7天），请执行 `owenc init` 重新授权。 (Error: {})",
                    status
                ));
            }
            if err_text.contains("4007") || err_text.contains("invalid_grant") {
                let _ = self.pool.as_vault().set(profile, "oauth2_revoked", "true").await;
                return Err(anyhow!(
                    "令牌已失效（可能已被吊销），请执行 `owenc auth login` 重新授权。 (Error: {})",
                    status
                ));
            }
            if err_text.contains("4006") {
                return Err(anyhow!(
                    "ClientID 与令牌颁发者不一致，请检查配置。 (Error: {})",
                    status
                ));
            }
            if err_text.contains("4001") {
                return Err(anyhow!(
                    "授权校验失败 (PKCE)，请重新执行 `owenc init`。 (Error: {})",
                    status
                ));
            }

            return Err(anyhow!(
                "OAuth2 token request failed (HTTP {}): {}",
                status,
                err_text
            ));
        }

        let token_resp: OAuth2TokenResponse = resp.json().await?;
        let now = Utc::now();

        let token = Token {
            value: token_resp.access_token.clone(),
            expires_at: now + Duration::seconds(token_resp.expires_in.unwrap_or(7200)),
            created_at: now,
        };

        let pair = OAuth2TokenPair {
            access_token: token_resp.access_token,
            refresh_token: token_resp.refresh_token.unwrap_or_default(),
            expires_at: token.expires_at,
            refresh_expires_at: now
                + Duration::seconds(token_resp.refresh_expires_in.unwrap_or(604800)),
            created_at: now,
        };

        // Save to vault via pool
        self.pool
            .as_vault()
            .set(profile, "oauth2_token_pair", &serde_json::to_string(&pair)?).await?;
        let _ = self.pool.as_vault().delete(profile, "oauth2_revoked").await;
        self.pool.set_access_token(profile, &token).await?;

        // Deduplication and permanent code logic have been moved to StoreAppProvider.


        tracing::info!(
            target: "audit",
            profile = %profile,
            event = "token_rotate",
            status = "success",
            "OAuth2 token pair successfully rotated"
        );

        Ok(token)
    }

    async fn finalize_login(&self, profile: &str, cfg: &Config) -> Result<()> {
        tracing::info!(target: "sys", profile = %profile, "Finalizer started for OAuth2 auth");
        
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
                                match self.exchange_code(profile, cfg, &res.code, &session.code_verifier, &session.redirect_uri).await {
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
impl AuthProvider for OAuth2Provider {
    async fn on_maintenance_tick(&self, profile: &str, config: &Config) -> Result<()> {
        if let Ok(pair_str) = self.pool.as_vault().get(profile, "oauth2_token_pair").await {
            if let Ok(pair) = serde_json::from_str::<OAuth2TokenPair>(&pair_str) {
                let remaining = pair.expires_at.signed_duration_since(chrono::Utc::now());
                if remaining < chrono::Duration::minutes(15) {
                    tracing::info!(target: "sys", "OAuth2 token expires in {:?}. Proactively refreshing...", remaining);
                    let _ = self.refresh(profile, config, &Default::default()).await;
                }
            }
        }
        Ok(())
    }

    async fn get_token(&self, profile: &str, cfg: &Config, _headers: &reqwest::header::HeaderMap) -> Result<Token> {
        // 1. Fast path: check current memory/local cache
        if let Ok(token) = self.pool.get_access_token(profile).await {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        // 2. Slow path: Acquire Cross-Process File Lock
        let lock_dir = crate::core::config::get_app_dir().join("locks");
        std::fs::create_dir_all(&lock_dir)?;
        let lock_file_path = lock_dir.join(format!("{}.lock", profile));
        let lock_file = File::create(&lock_file_path)?;

        // Blocking lock (wait for other processes)
        lock_file.lock_exclusive()?;

        let result = (|| async {
            // 3. Double-Check: Reload from Vault after acquiring lock
            // Another process might have refreshed the token while we were waiting
            if let Ok(token) = self.pool.get_access_token(profile).await {
                if !token.is_expired() {
                    return Ok(token);
                }
            }

            // 4. Finalizer Path: Check for captured code
            let session_manager = AuthSessionManager::new(self.pool.as_ref());
            if let Ok(code) = session_manager.get_captured_code(profile).await {
                if let Ok(session) = session_manager.get_session(profile).await {
                    tracing::info!(target: "sys", "Captured auth code found for profile '{}'. Finalizing exchange...", profile);
                    let token = self.exchange_code(profile, cfg, &code, &session.code_verifier, &session.redirect_uri).await?;
                    let _ = session_manager.clear(profile).await;
                    return Ok(token);
                }
            }

            let pair_str = self.pool.as_vault().get(profile, "oauth2_token_pair").await?;
            let pair: OAuth2TokenPair = serde_json::from_str(&pair_str)?;

            // Re-check expiry in case it was updated
            if Utc::now() < pair.expires_at {
                let token = Token {
                    value: pair.access_token.clone(),
                    expires_at: pair.expires_at,
                    created_at: pair.created_at,
                };
                self.pool.set_access_token(profile, &token).await?;
                return Ok(token);
            }

            if Utc::now() >= pair.refresh_expires_at {
                return Err(anyhow!("OAuth2 session expired. Please run 'owenc init' to re-authenticate."));
            }

            self.refresh_token(profile, cfg, &pair.refresh_token).await
        })().await;

        lock_file.unlock()?;
        result
    }

    async fn refresh(&self, profile: &str, cfg: &Config, _headers: &reqwest::header::HeaderMap) -> Result<Token> {
        let pair_str = self.pool.as_vault().get(profile, "oauth2_token_pair").await?;
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
        _body: &[u8],
        spec: &serde_json::Value,
    ) -> Result<crate::auth::provider::ProxyRequestAction> {
        let token = self.get_token(profile, config, &headers).await?;
        
        let auth_headers = crate::auth::RequestDecorator::get_auth_headers(
            spec, path, method, &config.app_key, &config.app_secret, &token.value
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

    async fn initialize(
        &self,
        profile: &str,
        config: &mut Config,
        vault: std::sync::Arc<dyn crate::core::vault::Vault>,
        cfg_mgr: &crate::core::config::ConfigManager,
        params: crate::auth::provider::InitParams,
    ) -> Result<()> {
        use crate::auth::lifecycle::orchestrator;
        

        // 1. Setup credentials (OCP: forced built-in for OAuth2)
        if params.app_key.is_some() || params.app_secret.is_some() {
            println!("⚠️  Note: OAuth2 mode uses the standard built-in identity. Provided AppKey/AppSecret will be ignored.");
        }
        config.app_key = crate::auth::models::BUILTIN_CLIENT_ID.to_string();
        config.app_secret = "".to_string();

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

        // 2. Persist config early so callback listeners can see it
        cfg_mgr.save(profile, config).await?;

        // 3. Start Flow

        println!("\n\x1b[1;34m🔒 Starting Authorization Flow...\x1b[0m");
        
        let token_pool = crate::auth::VaultTokenPool::new(vault.clone());
        let session_manager = crate::auth::lifecycle::AuthSessionManager::new(&token_pool);
        
        // 1. Get a free port for redirect_uri
        let port = cfg_mgr.find_free_port().await;
        
        // 1.1 Pre-cleanup residual sessions
        let _ = session_manager.clear(profile).await;
        
        // 2. Create Session
        let session = session_manager.create_session(profile, port).await?;
        
        // 3. Generate Auth URL
        let market_url = obfs!(env!("DEF_MARKET_URL"));
        let auth_url = format!(
            "{}/user/v2/authorize?client_id={}&response_type=code&scope=all&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256",
            market_url.trim_end_matches('/'),
            crate::auth::models::BUILTIN_CLIENT_ID,
            urlencoding::encode(&session.redirect_uri),
            session.state,
            Pkce::generate_challenge(&session.code_verifier),
        );

        println!("\n\x1b[1mPlease authorize in the LOCAL browser of this machine. Opening URL...\x1b[0m");
        
        // 4. Automatically open browser
        if let Err(e) = open::that(&auth_url) {
            tracing::warn!(target: "sys", error = %e, "Failed to open browser automatically");
            println!("\x1b[33m(Failed to open browser automatically. Please copy the URL below manually to your LOCAL browser)\x1b[0m");
        }
        
        println!("\x1b[34m{}\x1b[0m", auth_url);
        
        // 5. Spawn Background Finalizer
        println!("\n\x1b[34m🚀 授权监听已在本机启动。请在浏览器中确认...\x1b[0m");

        let pid = orchestrator::spawn_finalizer(profile, &session.state)?;
        
        // 6. Wait for Result (Closed Loop)
        // Note: is_new check is slightly simplified here as we are in provider, but init.rs can pass it or we assume false for existing.
        // Actually init.rs knows if it's new. I should probably add is_new to initialize signature or handle it.
        // For now, let's assume init.rs handles the "new profile cleanup" inside initialize? No, initialize should handle it.
        // I'll add is_new to the signature.
        let is_new = !cfg_mgr.exists(profile).await; 
        orchestrator::wait_for_token_exchange(profile, vault.clone(), pid, is_new, cfg_mgr).await?;

        // 7. Automatically start the daemon (OCP: Consistent experience across all modes)
        let _ = crate::cmd::system::ensure_daemon_running(profile, config, cfg_mgr, vault).await;

        Ok(())
    }

    async fn perform_login(&self, profile: &str, config: &Config, _force: bool, finalize: Option<&str>) -> Result<()> {
        // 1. Finalizer Implementation (Background flow)
        if let Some(_session_id) = finalize {
            return self.finalize_login(profile, config).await;
        }

        // 2. Regular Login flow
        println!("🔄 [OAuth2] Attempting to refresh token pair for profile '{}'...", profile);
        match self.refresh(profile, config, &reqwest::header::HeaderMap::new()).await {
            Ok(_) => {
                println!("✅ Success! OAuth2 Token Pair has been rotated.");
                Ok(())
            }
            Err(e) => {
                println!("❌ Refresh failed: {}", e);
                println!("💡 Suggestion: If the session has expired, please run \x1b[33mcowen init\x1b[0m to re-authorize.");
                Err(e)
            }
        }
    }

    async fn get_status_entries(&self, profile: &str, _config: &Config) -> Result<Vec<crate::core::status::StatusEntry>> {
        use crate::core::status::{StatusEntry, StatusLevel};
        let mut entries = Vec::new();
        let vault = self.pool.as_vault();

        let refresh_error = vault.get(profile, "last_refresh_error").await.ok();
        let ref_revoked = vault.get(profile, "oauth2_revoked").await.is_ok();

        if let Ok(pair_raw) = vault.get(profile, "oauth2_token_pair").await {
            let pair: crate::auth::models::OAuth2TokenPair = serde_json::from_str(&pair_raw)?;
            let is_expired = Utc::now() > pair.expires_at;
            let ref_expired = Utc::now() > pair.refresh_expires_at;

            let children = vec![
                StatusEntry {
                    name: "AccessToken".to_string(),
                    icon: "🔑".to_string(),
                    level: if is_expired || ref_revoked { StatusLevel::ERROR } else { StatusLevel::OK },
                    message: format!("[{}] (Expires: {})", 
                        if is_expired || ref_revoked { "EXPIRED" } else { "VALID" },
                        pair.expires_at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S")),
                    reason: if ref_revoked {
                        Some("关联的 RefreshToken 已失效，AccessToken 无法继续自动续约。".to_string())
                    } else if is_expired { 
                        refresh_error.map(|e| format!("自动续约失败: {}", e))
                            .or(Some("AccessToken 已过期，正在等待后台续约进程处理...".to_string()))
                    } else { None },
                    details: vec![],
                    children: vec![],
                },
                StatusEntry {
                    name: "RefreshToken".to_string(),
                    icon: "🔄".to_string(),
                    level: if ref_expired || ref_revoked { StatusLevel::ERROR } else { StatusLevel::OK },
                    message: format!("[{}] (Expires: {})", 
                        if ref_revoked { "REVOKED" } else if ref_expired { "EXPIRED" } else { "VALID" },
                        pair.refresh_expires_at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S")),
                    reason: if ref_revoked {
                        Some("令牌已于服务端吊销或失效，必须重新执行 `cowen auth login`。".to_string())
                    } else if ref_expired { 
                        Some("RefreshToken 已失效，必须重新运行 'cowen auth login' 或 'init'。".to_string()) 
                    } else { None },
                    details: vec![],
                    children: vec![],
                }
            ];

            let mut details = vec![];
            let token_inner = Token {
                value: pair.access_token.clone(),
                expires_at: pair.expires_at,
                created_at: pair.created_at,
            };
            
            if let Some(identity) = token_inner.extract_identity() {
                details.push(format!("User ID: {}", identity.user_id));
                details.push(format!("Org ID:  {}", identity.org_id));
                details.push(format!("App ID:  {}", identity.app_id));
            }

            entries.push(StatusEntry {
                name: "Authentication".to_string(),
                icon: "🔐".to_string(),
                level: if ref_revoked { StatusLevel::ERROR } else if is_expired { StatusLevel::WARN } else { StatusLevel::OK },
                message: "OAuth2 tokens are locally managed.".to_string(),
                reason: if ref_revoked { Some("会话已失效 (Revoked)".to_string()) } else { None },
                details,
                children,
            });
        }

        Ok(entries)
    }

    fn get_default_app_key(&self) -> Option<String> {
        Some(crate::auth::models::BUILTIN_CLIENT_ID.to_string())
    }

    fn supports_webhooks(&self) -> bool {
        false
    }

    fn decorate_openapi_request(&self, url: &mut String, headers: &mut reqwest::header::HeaderMap, token: &Token, config: &Config) {
        if !url.contains("checkPermission=") {
            if url.contains('?') {
                url.push_str("&checkPermission=false");
            } else {
                url.push_str("?checkPermission=false");
            }
        }
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
        Ok(())
    }
}
impl Pkce {
    pub fn new() -> Self {
        let verifier = Self::generate_verifier(64);
        Self { verifier }
    }

    fn generate_verifier(len: usize) -> String {
        const CHARSET: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
        (0..len)
            .map(|_| {
                let idx = rand::random_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    pub fn generate_challenge(verifier: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let result = hasher.finalize();
        URL_SAFE_NO_PAD.encode(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_generation() {
        let pkce = Pkce::new();
        assert_eq!(pkce.verifier.len(), 64);

        // Verify challenge can be computed from verifier
        let challenge = Pkce::generate_challenge(&pkce.verifier);
        assert!(!challenge.is_empty());

        // Manual verification of challenge
        let mut hasher = Sha256::new();
        hasher.update(pkce.verifier.as_bytes());
        let expected_challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
        assert_eq!(challenge, expected_challenge);
    }

    #[test]
    fn test_oauth2_capabilities() {
        struct MockVault {}
        #[async_trait::async_trait]
        impl crate::core::vault::Vault for MockVault {
            async fn get_config(&self, _: &str, _: &str) -> Result<String> { Ok("".to_string()) }
            async fn get_config_full(&self, _: &str, _: &str) -> Result<crate::core::store::Item> { Err(anyhow!("not found")) }
            async fn set_config(&self, _: &str, _: &str, _: &str) -> Result<()> { Ok(()) }
            async fn set_config_conditional(&self, _: &str, _: &str, _: &str, _: u64) -> Result<()> { Ok(()) }
            async fn list_configs(&self, _: &str) -> Result<Vec<String>> { Ok(vec![]) }
            async fn delete_config(&self, _: &str, _: &str) -> Result<()> { Ok(()) }
            async fn get_secret(&self, _: &str, _: &str) -> Result<String> { Ok("".to_string()) }
            async fn set_secret(&self, _: &str, _: &str, _: &str) -> Result<()> { Ok(()) }
            async fn get_token(&self, _: &str, _: &str) -> Result<String> { Ok("".to_string()) }
            async fn set_token(&self, _: &str, _: &str, _: &str, _: u64) -> Result<()> { Ok(()) }
            async fn save_audit(&self, _: &crate::core::store::AuditEntry) -> Result<()> { Ok(()) }
            async fn list_audit(&self, _: &str, _: usize) -> Result<Vec<crate::core::store::AuditEntry>> { Ok(vec![]) }
            async fn push_dlq(&self, _: &crate::core::store::DlqMessage) -> Result<()> { Ok(()) }
            async fn pop_dlq(&self, _: &str, _: &str) -> Result<Option<crate::core::store::DlqMessage>> { Ok(None) }
            async fn list_dlq(&self, _: &str, _: usize) -> Result<Vec<crate::core::store::DlqMessage>> { Ok(vec![]) }
            async fn get_cache(&self, _: &str, _: &str) -> Result<String> { Ok("".to_string()) }
            async fn set_cache(&self, _: &str, _: &str, _: &str, _: u64) -> Result<()> { Ok(()) }
            async fn get(&self, _: &str, _: &str) -> Result<String> { Ok("".to_string()) }
            async fn set(&self, _: &str, _: &str, _: &str) -> Result<()> { Ok(()) }
            async fn delete(&self, _: &str, _: &str) -> Result<()> { Ok(()) }
            async fn list_keys(&self, _: &str, _: &str) -> Result<Vec<String>> { Ok(vec![]) }
            async fn clear_profile(&self, _: &str) -> Result<()> { Ok(()) }
            async fn rename_profile(&self, _: &str, _: &str) -> Result<()> { Ok(()) }
            async fn list_all_profiles(&self) -> Result<Vec<String>> { Ok(vec![]) }
            async fn notify_config_changed(&self, _: &str, _: &str) -> Result<()> { Ok(()) }
            async fn watch_config(&self, _: &str) -> Result<std::pin::Pin<Box<dyn tokio_stream::Stream<Item = String> + Send>>> {
                    Ok(Box::pin(tokio_stream::iter(vec![])))
            }
            fn primary_store(&self) -> Arc<dyn crate::core::store::Store> { unimplemented!() }
        }

        struct MockHttpSender {}
        #[async_trait::async_trait]
        impl crate::auth::client::HttpSender for MockHttpSender {
            async fn post(&self, _: &str, _: reqwest::header::HeaderMap, _: serde_json::Value) -> Result<crate::auth::client::SimpleResponse> {
                Ok(crate::auth::client::SimpleResponse { status: 200, body: "{}".to_string() })
            }
            async fn post_form(&self, _: &str, _: reqwest::header::HeaderMap, _: serde_json::Value) -> Result<crate::auth::client::SimpleResponse> {
                Ok(crate::auth::client::SimpleResponse { status: 200, body: "{}".to_string() })
            }
            async fn get(&self, _: &str, _: reqwest::header::HeaderMap) -> Result<crate::auth::client::SimpleResponse> {
                Ok(crate::auth::client::SimpleResponse { status: 200, body: "{}".to_string() })
            }
        }

        let vault = Arc::new(MockVault {});
        let pool: Arc<dyn TokenPool> = Arc::new(crate::auth::VaultTokenPool::new(vault));
        let sender = Arc::new(MockHttpSender {});
        let provider = OAuth2Provider::new(pool, sender);
        assert!(!provider.supports_webhooks());
    }

    #[test]
    fn test_verifier_charset() {
        let verifier = Pkce::generate_verifier(1000);
        let allowed = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
        for c in verifier.chars() {
            assert!(allowed.contains(c));
        }
    }

}
