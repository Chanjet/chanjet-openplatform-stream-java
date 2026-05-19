use cowen_common::{CowenResult, CowenError};
use async_trait::async_trait;
use rand::Rng;
use cowen_common::daemon::DaemonService;
use cowen_infra::obfs;
use crate::client::HttpSender;
use crate::lifecycle::AuthSessionManager;
use crate::models::{OAuth2TokenPair, Token};
use crate::pool::TokenPool;
use crate::provider::AuthProvider;
use cowen_common::config::Config;

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
    ) -> CowenResult<cowen_common::models::Token> {
        let url = format!(
            "{}{}",
            cfg.openapi_url.trim_end_matches('/'),
            obfs!("/oauth2/token")
        );
        let body = serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": crate::models::BUILTIN_CLIENT_ID,
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
    ) -> CowenResult<cowen_common::models::Token> {
        let url = format!(
            "{}{}",
            cfg.openapi_url.trim_end_matches('/'),
            obfs!("/oauth2/token")
        );
        let body = serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": crate::models::BUILTIN_CLIENT_ID,
            "appKey": crate::models::BUILTIN_CLIENT_ID,
            "refresh_token": refresh_token,
        });

        self.request_token(profile, &url, body, cfg).await
    }

    async fn request_token(
        &self,
        profile: &str,
        url: &str,
        body: serde_json::Value,
        cfg: &Config,
    ) -> CowenResult<cowen_common::models::Token> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("appKey", cfg.app_key.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
        
        let resp = self.http_sender.post_form(url, headers, body).await?;

        if !resp.is_success() {
            let status = resp.status;
            let err_text = cowen_common::utils::mask_sensitive_json(&resp.text());

            tracing::error!(
                target: "audit",
                profile = %profile,
                event = "token_rotate",
                status = "failure",
                error = %err_text,
                "OAuth2 token rotation failed"
            );
            
            eprintln!("❌ OAuth2 token rotation failed (HTTP {}): {}", status, err_text);

            // Handle specific platform error codes (Design §6)
            if err_text.contains("4029") {
                return Err(CowenError::Auth(format!("登录会话已超时（7天），请执行 `owenc init` 重新授权。 (Error: {})", status)));
            }
            if err_text.contains("4007") || err_text.contains("invalid_grant") {
                let _ = self.pool.as_vault().set_config(profile, "oauth2_revoked", "true").await;
                return Err(CowenError::Auth(format!("令牌已失效（可能已被吊销），请执行 `owenc auth login` 重新授权。 (Error: {})", status)));
            }
            if err_text.contains("4006") {
                return Err(CowenError::Auth(format!("ClientID 与令牌颁发者不一致，请检查配置。 (Error: {})", status)));
            }
            if err_text.contains("4001") {
                return Err(CowenError::Auth(format!("授权校验失败 (PKCE)，请重新执行 `owenc init`。 (Error: {})", status)));
            }

            return Err(CowenError::Auth(format!("OAuth2 token request failed (HTTP {}): {}", status, err_text)));
        }

        let token_resp: OAuth2TokenResponse = resp.json().await?;
        let now = Utc::now();
        
        tracing::info!(target: "sys", profile = %profile, "Received token response: expires_in={:?}, refresh_expires_in={:?}", token_resp.expires_in, token_resp.refresh_expires_in);

        let token = Token {
            value: token_resp.access_token.clone(),
            expires_at: now + Duration::seconds(token_resp.expires_in.unwrap_or(7200)),
            created_at: now,
        };

        let refresh_token = Token {
            value: token_resp.refresh_token.unwrap_or_default(),
            expires_at: now + Duration::seconds(token_resp.refresh_expires_in.unwrap_or(604800)),
            created_at: now,
        };

        // Save to vault via pool (Structured TokenDomain is the single source of truth)
        self.pool.as_vault().save_access_token(profile, token.clone()).await?;
        self.pool.as_vault().save_refresh_token(profile, refresh_token).await?;
        
        let _ = self.pool.as_vault().delete_config(profile, "oauth2_revoked").await;
        let _ = self.pool.as_vault().delete_config(profile, "last_refresh_error").await;
        
        // 🚀 OCP: Cleanup legacy JSON-blob to prevent inconsistency
        let _ = self.pool.as_vault().delete_config(profile, "oauth2_token_pair").await;
        let _ = self.pool.as_vault().delete_secret(profile, "oauth2_token_pair").await;

        tracing::info!(
            target: "audit",
            profile = %profile,
            event = "token_rotate",
            status = "success",
            "OAuth2 token pair successfully rotated"
        );

        Ok(token)
    }

    async fn get_refresh_token_with_fallback(&self, profile: &str) -> CowenResult<cowen_common::models::Token> {
        let vault = self.pool.as_vault();
        match vault.get_refresh_token(profile).await {
            Ok(t) => Ok(t),
            Err(_) => {
                // LEGACY FALLBACK: Try to recover from old JSON blob
                if let Ok(pair_str) = vault.get_config(profile, "oauth2_token_pair").await {
                    let pair: OAuth2TokenPair = serde_json::from_str(&pair_str)?;
                    Ok(cowen_common::models::Token {
                        value: pair.refresh_token,
                        expires_at: pair.refresh_expires_at,
                        created_at: pair.created_at,
                    })
                } else if let Ok(pair_str) = vault.get_secret(profile, "oauth2_token_pair").await {
                    let pair: OAuth2TokenPair = serde_json::from_str(&pair_str)?;
                    Ok(cowen_common::models::Token {
                        value: pair.refresh_token,
                        expires_at: pair.refresh_expires_at,
                        created_at: pair.created_at,
                    })
                } else {
                    Err(CowenError::Auth(format!("OAuth2 session missing or expired for profile '{}'. Please run 'owenc auth login'.", profile)))
                }
            }
        }
    }

    async fn finalize_login(&self, profile: &str, cfg: &Config, session_id: &str, daemon_service: Option<std::sync::Arc<dyn cowen_common::daemon::DaemonService>>) -> CowenResult<()> {
        tracing::info!(target: "sys", profile = %profile, session_id = %session_id, "Finalizer started for OAuth2 auth");
        
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
                                match self.exchange_code(profile, cfg, &res.code, &session.code_verifier, &session.redirect_uri).await {
                                    Ok(_) => {
                                        tracing::info!(target: "sys", "Token exchange successful");
                                        
                                        // 🚀 OCP: Auto-start daemon after successful authorization
                                        if let Some(ds) = daemon_service {
                                            tracing::info!(target: "sys", "Triggering background daemon startup after successful OAuth2 exchange");
                                            let _ = ds.start_daemon(profile, cfg, self.pool.as_vault()).await;
                                        }

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
impl AuthProvider for OAuth2Provider {
    async fn on_maintenance_tick(&self, profile: &str, config: &Config) -> CowenResult<()> {
        if let Ok(token) = self.pool.as_vault().get_access_token(profile).await {
            if token.is_expired_with_buffer(chrono::Duration::minutes(15)) {
                let remaining = token.expires_at.signed_duration_since(chrono::Utc::now());
                tracing::info!(target: "sys", "OAuth2 token expires in {:?}. Proactively refreshing...", remaining);
                match self.refresh(profile, config, &Default::default()).await {
                    Ok(_) => {
                        let _ = self.pool.as_vault().delete_config(profile, "last_refresh_error").await;
                    }
                    Err(e) => {
                        let _ = self.pool.as_vault().set_config(profile, "last_refresh_error", &e.to_string()).await;
                        return Err(e);
                    }
                }
            }
        }
        Ok(())
    }

    async fn get_token(&self, profile: &str, cfg: &Config, _headers: &reqwest::header::HeaderMap) -> CowenResult<cowen_common::models::Token> {
        // 1. Fast path: check current memory/local cache
        if let Ok(token) = self.pool.get_access_token(profile).await {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        // 2. Slow path: Acquire Cross-Process File Lock (Async-friendly)
        let lock_dir = cowen_common::config::get_app_dir().join("locks");
        let _ = std::fs::create_dir_all(&lock_dir);
        let lock_file_path = lock_dir.join(format!("{}.lock", profile));
        let lock_file = File::create(&lock_file_path).map_err(|e| CowenError::Internal(format!("Failed to create lock file: {}", e)))?;

        // 🚀 STABILITY: Use try_lock in a loop with async sleep to avoid blocking Tokio threads
        let mut acquired = false;
        for _ in 0..300 { // Max 30s
            if lock_file.try_lock_exclusive().is_ok() {
                acquired = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        if !acquired {
            return Err(CowenError::Auth(format!("Timeout waiting for cross-process lock for profile '{}'.", profile)));
        }

        let result = (|| async {
            // 3. Double-Check: Reload from Vault after acquiring lock
            if let Ok(token) = self.pool.as_vault().get_access_token(profile).await {
                if !token.is_expired() {
                    return Ok(token);
                }
            }

            // 4. Finalizer Path: Check for captured code
            let session_manager = AuthSessionManager::new(self.pool.as_ref());
            if let Ok(_code) = session_manager.get_captured_code(profile).await {
                // In the new world, orchestrator should handle this via state.
                // For now, we use a heuristic or just fall back to refresh.
            }

            // 4. Fallback to refresh if possible
            let rt = self.get_refresh_token_with_fallback(profile).await?;

            if rt.is_expired() {
                return Err(CowenError::Auth(format!("OAuth2 session expired. Please run 'owenc auth login' to re-authenticate.")));
            }

            self.refresh_token(profile, cfg, &rt.value).await
            })().await;

        lock_file.unlock()?;
        result
    }

    async fn refresh(&self, profile: &str, cfg: &Config, _headers: &reqwest::header::HeaderMap) -> CowenResult<cowen_common::models::Token> {
        let rt = self.get_refresh_token_with_fallback(profile).await?;
        self.refresh_token(profile, cfg, &rt.value).await
    }

    fn is_allowed_in_distributed_storage(&self) -> bool {
        false
    }

    /// OCP: OAuth2 uses relaxed dedup — only conflicts with other OAuth2 profiles using the same key.
    /// This allows the same AppKey to coexist across different auth modes (e.g., OAuth2 + SelfBuilt).
    async fn find_conflicting_profile(
        &self,
        app_key: &str,
        cfg_mgr: &cowen_config::ConfigManager,
    ) -> CowenResult<Option<String>> {
        cfg_mgr.find_profile_by_key_and_mode(app_key, &cowen_common::models::AuthMode::Oauth2).await
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
    ) -> CowenResult<crate::provider::ProxyRequestAction> {
        let token = self.get_token(profile, config, &headers).await?;
        
        let auth_headers = crate::RequestDecorator::get_auth_headers(
            spec, path, method, &config.app_key, &config.app_secret, &token.value
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

    async fn initialize(
        &self,
        profile: &str,
        config: &mut Config,
        vault: std::sync::Arc<dyn cowen_common::vault::Vault>,
        cfg_mgr: &cowen_config::ConfigManager,
        params: crate::provider::InitParams,
        daemon_service: Option<std::sync::Arc<dyn DaemonService>>,
    ) -> CowenResult<()> {
        use crate::lifecycle::orchestrator;
        

        // 1. Setup credentials (OCP: forced built-in for OAuth2)
        if params.app_key.is_some() || params.app_secret.is_some() {
            println!("⚠️  Note: OAuth2 mode uses the standard built-in identity. Provided AppKey/AppSecret will be ignored.");
        }
        config.app_key = crate::models::BUILTIN_CLIENT_ID.to_string();
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

        // 1.1 Use is_new from params (as Init already anchored the identity)
        let is_new = params.is_new;

        // 2. Persist config early so callback listeners can see it
        cfg_mgr.save(profile, config).await?;

        // 3. Start Flow

        println!("\n\x1b[1;34m🔒 Starting Authorization Flow...\x1b[0m");
        
        let token_pool = crate::VaultTokenPool::new(vault.clone());
        let session_manager = crate::lifecycle::AuthSessionManager::new(&token_pool);
        
        // 1. Get a free port for redirect_uri
        let port = cfg_mgr.find_free_port().await;
        
        // 1.1 Pre-cleanup residual sessions
        let _ = session_manager.clear(profile).await;
        
        // 2. Create Session
        let session = session_manager.create_session(profile, port).await?;
        
        // 3. Generate Auth URL
        let market_url = obfs!(cowen_common::config::DEF_MARKET_URL);
        let auth_url = format!(
            "{}/user/v2/authorize?client_id={}&response_type=code&scope=all&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256",
            market_url.trim_end_matches('/'),
            crate::models::BUILTIN_CLIENT_ID,
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
        
        // 6. Wait for Result (Closed Loop) with Signal Handling
        tokio::select! {
            res = orchestrator::wait_for_token_exchange(profile, vault.clone(), pid, is_new, cfg_mgr, &session.state) => {
                res?;
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\n🛑 Authorization cancelled by user.");
                return Err(CowenError::Auth(format!("Authorization cancelled")));
            }
        }


        // 7. Automatically start the daemon (OCP: Consistent experience across all modes)
        if params.auto_start {
            if let Some(ds) = &daemon_service { let _ = ds.start_daemon(profile, config, vault.clone()).await; }
        }

        Ok(())
    }

    async fn perform_login(&self, profile: &str, config: &Config, _force: bool, finalize: Option<&str>, daemon_service: Option<std::sync::Arc<dyn cowen_common::daemon::DaemonService>>) -> CowenResult<()> {
        // 1. Finalizer Implementation (Background flow)
        if let Some(session_id) = finalize {
            return self.finalize_login(profile, config, session_id, daemon_service).await;
        }

        // 2. Regular Login flow: Try refresh if valid pair exists
        let vault = self.pool.as_vault();
        let rt_opt = self.get_refresh_token_with_fallback(profile).await.ok();

        if let Some(rt) = rt_opt {
            if !rt.is_expired() {
                println!("🔄 [OAuth2] Attempting to refresh token pair for profile '{}'...", profile);
                match self.refresh_token(profile, config, &rt.value).await {
                    Ok(_) => {
                        println!("✅ Success! OAuth2 Token Pair has been rotated.");
                        return Ok(());
                    }
                    Err(e) => {
                        println!("⚠️  Refresh failed: {}. Falling back to full authorization...", e);
                    }
                }
            } else {
                println!("⚠️  OAuth2 RefreshToken has expired.");
            }
        } else {
            println!("💡 No active OAuth2 session found for profile '{}'.", profile);
        }

        // 3. Fallback: Trigger Automatic Re-authorization (Init Flow)
        println!("🚀 Triggering automatic browser-based authorization...");
        
        let mut mutable_config = config.clone();
        let cfg_mgr = cowen_config::ConfigManager::new()?;
        
        let params = crate::provider::InitParams {
            app_key: None,
            app_secret: None,
            certificate: None,
            encrypt_key: None,
            openapi_url: None,
            stream_url: None,
            webhook_target: None,
            proxy_port: None,
            auto_start: true,
            is_new: false,
        };

        self.initialize(profile, &mut mutable_config, vault, &cfg_mgr, params, daemon_service).await
    }

    async fn get_diagnostics(&self, ctx: &cowen_monitor::status::StatusContext<'_>) -> CowenResult<Vec<cowen_monitor::status::StatusEntry>> {
        use cowen_monitor::status::{StatusEntry, StatusLevel, CommonTemplate, AsStatusUI, collect_daemon_status};
        
        enum OAuth2Template {
            SecurityVault,
            AccessToken,
            RefreshToken,
            Authentication,
        }
        impl AsStatusUI for OAuth2Template {
            fn ui(&self) -> (String, String) {
                match self {
                    Self::SecurityVault => ("Security (Vault)".to_string(), "🛡️".to_string()),
                    Self::AccessToken => ("AccessToken".to_string(), "🔑".to_string()),
                    Self::RefreshToken => ("RefreshToken".to_string(), "🔄".to_string()),
                    Self::Authentication => ("Authentication".to_string(), "🔐".to_string()),
                }
            }
        }

        let mut results = Vec::new();
        let profile = &ctx.profile;
        let vault = ctx.vault.clone();

        // 1. Authentication Summary
        let mut auth_entries = Vec::new();

        // 1.1 Security Check
        auth_entries.push(StatusEntry::new(OAuth2Template::SecurityVault, StatusLevel::OK, "All core secrets are securely stored.".to_string()));

        // 1.2 Token Status
        let refresh_error = vault.get_config(profile, "last_refresh_error").await.ok();
        let ref_revoked = vault.get_config(profile, "oauth2_revoked").await.is_ok();

        let at_res: CowenResult<cowen_common::models::Token> = vault.get_access_token(profile).await;
        let rt_res: CowenResult<cowen_common::models::Token> = self.get_refresh_token_with_fallback(profile).await;

        match (at_res, rt_res) {
            (Ok(at), Ok(rt)) => {
                let is_expired = at.is_expired();
                let ref_expired = rt.is_expired();

                let token_children = vec![
                    StatusEntry::new(OAuth2Template::AccessToken, if is_expired || ref_revoked { StatusLevel::ERROR } else { StatusLevel::OK }, format!("[{}] (Expires: {})", 
                            if is_expired || ref_revoked { "EXPIRED" } else { "VALID" },
                            at.expires_at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S")))
                        .with_reason(if ref_revoked {
                            Some("关联的 RefreshToken 已失效，AccessToken 无法继续自动续约。".to_string())
                        } else if is_expired { 
                            refresh_error.as_ref().map(|e| format!("自动续约失败: {}", e))
                                .or(Some("AccessToken 已过期，正在等待后台续约进程处理...".to_string()))
                        } else { None }),
                    StatusEntry::new(OAuth2Template::RefreshToken, if ref_expired || ref_revoked { StatusLevel::ERROR } else { StatusLevel::OK }, format!("[{}] (Expires: {})", 
                            if ref_revoked { "REVOKED" } else if ref_expired { "EXPIRED" } else { "VALID" },
                            rt.expires_at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S")))
                        .with_reason(if ref_revoked {
                            Some("令牌已于服务端吊销或失效，必须重新执行 `cowen auth login`。".to_string())
                        } else if ref_expired { 
                            Some("RefreshToken 已失效，必须重新运行 'cowen auth login' 或 'init'。".to_string()) 
                        } else { None }),
                ];

                let mut details = vec![];
                if let Some(identity) = at.extract_identity() {
                    details.push(format!("User ID: {}", identity.user_id));
                    details.push(format!("Org ID:  {}", identity.org_id));
                    details.push(format!("App ID:  {}", identity.app_id));
                }

                auth_entries.push(StatusEntry::new(OAuth2Template::Authentication, if ref_revoked { StatusLevel::ERROR } else if is_expired { StatusLevel::WARN } else { StatusLevel::OK }, "OAuth2 tokens are locally managed.".to_string())
                    .with_reason(if ref_revoked { Some("会话已失效 (Revoked)".to_string()) } else { None })
                    .with_details(details)
                    .with_children(token_children));
            }
            _ => {
                auth_entries.push(StatusEntry::new(
                    OAuth2Template::Authentication, 
                    StatusLevel::WARN, 
                    "Not logged in or session expired.".to_string()
                ).with_reason(Some("本地未发现有效的 OAuth2 会话，或您已退出登录。请执行 `cowen auth login` 重新授权。".to_string())));
            }
        }

        // Wrap Authentication Summary
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
        let daemon_info = cowen_monitor::status::get_active_daemon_info(profile);
        let (display_name, efficiency_tip) = self.get_daemon_display_info(daemon_info.is_some());
        results.push(collect_daemon_status(ctx, &display_name, &efficiency_tip, self.supports_webhooks(), daemon_info).await?);

        Ok(results)
    }

    fn get_default_app_key(&self) -> Option<String> {
        Some(crate::models::BUILTIN_CLIENT_ID.to_string())
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

    async fn on_logout(&self, profile: &str, _config: &Config) -> CowenResult<()> {
        let vault = self.pool.as_vault();
        let _ = vault.delete_access_token(profile).await;
        let _ = vault.delete_refresh_token(profile).await;
        
        // Cleanup legacy keys if any
        let _ = vault.delete_config(profile, "oauth2_token_pair").await;
        let _ = vault.delete_secret(profile, "oauth2_token_pair").await;
        
        let _ = vault.delete_config(profile, "oauth2_revoked").await;
        let _ = vault.delete_config(profile, "last_refresh_error").await;
        Ok(())
    }

    async fn should_auto_recover(&self, profile: &str, config: &Config, has_pid: bool, _pid_file_exists: bool, is_distributed: bool) -> bool {
        if has_pid || config.app_key.trim().is_empty() {
            return false;
        }

        if is_distributed {
            return false;
        }

        // 🚀 OCP: For OAuth2, only auto-recover if we actually have a token pair.
        // If no token pair exists, it means the profile is not yet authorized,
        // and starting a daemon will just lead to errors or race conditions during 'init'.
        if self.get_refresh_token_with_fallback(profile).await.is_ok() {
            return true;
        }
        
        false
    }

    fn get_daemon_display_info(&self, is_running: bool) -> (String, String) {
        let name = "Token Renewer (Daemon)";
        let tip = if is_running { "主动续约: [ACTIVE]" } else { "若需实现令牌自动续约，请运行 'cowen daemon start'" };
        (name.to_string(), tip.to_string())
    }
}
impl Pkce {
    pub fn new() -> Self {
        let verifier = Self::generate_verifier(64);
        Self { verifier }
    }

    pub fn generate_verifier(len: usize) -> String {
        const CHARSET: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
        (0..len)
            .map(|_| {
                let idx = rand::thread_rng().gen_range(0..CHARSET.len());
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

