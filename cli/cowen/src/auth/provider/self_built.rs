use crate::auth::models::Token;
use crate::auth::pool::TokenPool;
use crate::auth::provider::AuthProvider;
use crate::auth::client::{HttpSender, ReqwestSender};
use crate::core::config::Config;
use anyhow::{Result, anyhow, Context};
use async_trait::async_trait;
use chrono::{Utc, Duration};
use serde::Deserialize;
use std::sync::Arc;

pub struct SelfBuiltProvider {
    pool: Arc<dyn TokenPool>,
    http_sender: Arc<dyn HttpSender>,
    refresh_lock: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Debug, Deserialize)]
struct PlatformTokenResponse {
    result: bool,
    error: Option<serde_json::Value>,
    value: Option<TokenValue>,
}

#[derive(Debug, Deserialize)]
struct TokenValue {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "expiresIn")]
    expires_in: i64,
}

impl SelfBuiltProvider {
    #[allow(dead_code)]
    pub fn new(pool: Arc<dyn TokenPool>) -> Self {
        Self {
            pool,
            http_sender: Arc::new(ReqwestSender::new()),
            refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub fn with_sender(pool: Arc<dyn TokenPool>, sender: Arc<dyn HttpSender>) -> Self {
        Self {
            pool,
            http_sender: sender,
            refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    async fn perform_network_refresh(&self, profile: &str, cfg: &Config) -> Result<Token> {
        if cfg.app_key.trim().is_empty() || cfg.app_secret.trim().is_empty() {
            return Err(anyhow!("Credential Missing: AppKey or AppSecret is empty for profile '{}'. Please run 'cowen init' to configure your environment.", profile));
        }

        let mut attempts = 0;
        let max_attempts = 65; 
        
        let ticket = loop {
            match self.pool.as_vault().get_app_ticket(&cfg.app_key).await {
                Ok(t) => break t,
                Err(e) => {
                    if attempts >= max_attempts {
                        return Err(e).context("Missing app_ticket. The background daemon is running but hasn't received a push from the platform yet. Please wait a moment or check your network/firewall.");
                    }
                    if attempts == 0 {
                        eprintln!("⏳ AppTicket missing. Proactively triggering a platform push...");
                        if let Err(push_err) = self.trigger_push(profile, cfg, false).await {
                            let err_str = push_err.to_string();
                            if err_str.contains("HTTP 401") || err_str.contains("50003") {
                                return Err(push_err).context("Fatal configuration error from platform. Please check your AppKey, AppSecret, and OpenAPI URL settings.");
                            }
                            tracing::warn!(target: "sys", error = %push_err, "Failed to trigger proactive push");
                        }
                    }
                    if attempts % 5 == 0 {
                        eprintln!("⏳ Waiting for security handshake (AppTicket) from platform ({}s)...", attempts);
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    attempts += 1;
                }
            }
        };

        let url = format!("{}{}", cfg.openapi_url, obfs!("/v1/common/auth/selfBuiltApp/generateToken"));
        let app_key = cfg.app_key.trim();
        let app_secret = cfg.app_secret.trim();
        
        let body = serde_json::json!({
            "appKey": app_key,
            "appSecret": app_secret,
            "appTicket": ticket.value,
            "certificate": cfg.certificate.trim(),
            "authCertificate": cfg.certificate.trim(),
        });

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("appKey", app_key.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
        headers.insert("appSecret", app_secret.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));

        let resp = self.http_sender.post(&url, headers, body).await?;

        if !resp.is_success() {
            let status = resp.status;
            let err_text = resp.text();
            let safe_err = crate::core::utils::mask_sensitive_json(&err_text);
            return Err(anyhow!("Platform auth failed (HTTP {}): {}", status, safe_err));
        }

        let token_resp: PlatformTokenResponse = resp.json().await?;
        
        if !token_resp.result {
            return Err(anyhow!("Platform error: {:?}", token_resp.error));
        }

        let val = token_resp.value.context("Platform returned success but value is empty")?;
        
        let now = Utc::now();
        let new_token = Token {
            value: val.access_token,
            expires_at: now + Duration::seconds(val.expires_in),
            created_at: now,
        };

        self.pool.set_app_access_token(&cfg.app_key, &new_token).await?;
        Ok(new_token)
    }

    pub async fn trigger_push(&self, profile: &str, cfg: &Config, force: bool) -> Result<()> {
        let vault = self.pool.as_vault();
        
        if !force {
            let now = Utc::now();
            let last_attempt = if let Some(ts_str) = vault.get_config(profile, "push_last_attempt_ts").await.ok() {
                chrono::DateTime::parse_from_rfc3339(&ts_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now() - Duration::hours(1))
            } else {
                Utc::now() - Duration::hours(1)
            };
            
            let level: u32 = vault.get_config(profile, "push_backoff_level").await
                .unwrap_or_else(|_| "0".to_string())
                .parse()
                .unwrap_or(0);
                
            let wait_secs = if level == 0 { 1 } else { 60 * (1 << std::cmp::min(level, 10)) };
            let elapsed = now.signed_duration_since(last_attempt).num_seconds();
            
            if elapsed < wait_secs as i64 {
                tracing::info!(target: "sys", "Proactive push throttled for profile '{}'. Level: {}, Needs wait: {}s, Elapsed: {}s. Skipping.", profile, level, wait_secs, elapsed);
                return Ok(());
            }
        }

        let app_key = cfg.app_key.trim();
        let app_secret = cfg.app_secret.trim();

        if app_key.is_empty() || app_secret.is_empty() {
            return Err(anyhow!("Missing AppKey or AppSecret for profile '{}'. Please run 'cowen init' first.", profile));
        }

        let url = format!("{}{}", cfg.openapi_url, obfs!("/auth/appTicket/resend"));
        let body = serde_json::json!({});

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("appKey", app_key.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
        headers.insert("appSecret", app_secret.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));

        let resp = self.http_sender.post(&url, headers, body).await?;

        if !resp.is_success() {
            let status = resp.status;
            let err_text = resp.text();
            
            let level: u32 = vault.get_config(profile, "push_backoff_level").await.unwrap_or_else(|_| "0".to_string()).parse().unwrap_or(0);
            if status == 409 {
                let _ = vault.set_config(profile, "push_backoff_level", &(level + 1).to_string()).await;
            }
            let _ = vault.set_config(profile, "push_last_attempt_ts", &Utc::now().to_rfc3339()).await;
            
            return Err(anyhow!("Failed to trigger push (HTTP {}): {}", status, err_text));
        }

        let _ = vault.set_config(profile, "push_backoff_level", "0").await;
        let _ = vault.set_config(profile, "push_last_attempt_ts", &Utc::now().to_rfc3339()).await;

        #[derive(Deserialize)]
        struct ResendResp {
            code: String,
            message: Option<String>,
        }

        let resend_resp: ResendResp = resp.json().await?;
        if resend_resp.code != "200" {
            return Err(anyhow!("Platform error: {} - {:?}", resend_resp.code, resend_resp.message));
        }

        Ok(())
    }
}

#[async_trait]
impl AuthProvider for SelfBuiltProvider {
    async fn trigger_push(&self, profile: &str, config: &Config, force: bool) -> Result<()> {
        self.trigger_push(profile, config, force).await
    }

    async fn get_token(&self, profile: &str, cfg: &Config, _headers: &reqwest::header::HeaderMap) -> Result<Token> {
        // 1. First check: Fast path (Pool)
        if let Ok(token) = self.pool.get_app_access_token(&cfg.app_key).await {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        // 2. Local process lock (Still useful for multi-threaded within same pod)
        let _guard = self.refresh_lock.lock().await;
        
        // 3. Second check: After local lock
        if let Ok(token) = self.pool.get_app_access_token(&cfg.app_key).await {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        // 4. Distributed coordination: Jitter + Third check
        // In multi-process environments (Case 31), multiple pods might reach here.
        // We add a small random delay to allow one pod to "win" the refresh race.
        let delay_ms = rand::random_range(50..500);
        tracing::debug!(target: "sys", profile = %profile, delay = %delay_ms, "Token missing, waiting jitter before refresh...");
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;

        if let Ok(token) = self.pool.get_app_access_token(&cfg.app_key).await {
            if !token.is_expired() {
                tracing::info!(target: "sys", profile = %profile, "Token became available after jitter. Skipping network refresh.");
                return Ok(token);
            }
        }

        // 5. Network refresh (Only one pod should ideally reach here)
        self.pool.clear_cache(profile);
        self.perform_network_refresh(profile, cfg).await
    }

    async fn refresh(&self, profile: &str, cfg: &Config, _headers: &reqwest::header::HeaderMap) -> Result<Token> {
        let _guard = self.refresh_lock.lock().await;
        self.pool.clear_cache(profile);
        self.perform_network_refresh(profile, cfg).await
    }

    async fn hydrate_config(&self, profile: &str, config: &mut Config, vault: std::sync::Arc<dyn crate::core::vault::Vault>) -> Result<()> {
        if let Ok(as_val) = vault.get_secret(profile, "app_secret").await { config.app_secret = as_val; }
        if let Ok(cert) = vault.get_secret(profile, "certificate").await { config.certificate = cert; }
        if let Ok(ek) = vault.get_secret(profile, "encrypt_key").await { config.encrypt_key = ek; }
        Ok(())
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
        // 1. Setup credentials
        if let Some(ak) = params.app_key {
            config.app_key = ak;
        }

        let app_key = config.app_key.trim();
        let global_profile = format!("app:{}", app_key);

        if let Some(as_val) = params.app_secret {
            vault.set_secret(&global_profile, "app_secret", &as_val).await?;
            config.app_secret = as_val;
        }
        if let Some(cert) = params.certificate {
            vault.set_secret(&global_profile, "certificate", &cert).await?;
            config.certificate = cert;
        }
        if let Some(ek) = params.encrypt_key {
            vault.set_secret(&global_profile, "encrypt_key", &ek).await?;
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
        if config.app_key.trim().is_empty() 
            || config.app_secret.trim().is_empty() 
            || config.certificate.trim().is_empty()
            || config.encrypt_key.trim().is_empty() 
        {
            if is_new {
                let _ = cfg_mgr.delete(profile).await;
            }
            let bin_name = crate::core::utils::get_bin_name();
            println!("Error: --app-key, --app-secret, --certificate, and --encrypt-key are required for self-built mode.");
            println!("Example: {} init --app-mode self-built --app-key X --app-secret Y --certificate Z --encrypt-key E", bin_name);
            return Err(anyhow!("Missing required credentials for SelfBuilt mode"));
        }

        if params.auto_start {
            println!("✅ Profile '{}' initialized successfully.", profile);
            // Automatically start the daemon for Self-Built mode to avoid OFFLINE status on first check
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
        } else {
            println!("✅ Profile '{}' initialized successfully (Auto-start disabled).", profile);
        }
        Ok(())
    }

    async fn on_maintenance_tick(&self, profile: &str, config: &Config) -> Result<()> {
        let token_res = if let Ok(t) = self.pool.get_app_access_token(&config.app_key).await {
            Ok(t)
        } else {
            self.pool.get_access_token(profile).await
        };

        match token_res {
            Ok(token) => {
                let remaining = token.expires_at.signed_duration_since(Utc::now());
                if remaining < Duration::minutes(15) {
                    tracing::info!(target: "sys", "Self-Built token expires in less than 15 mins. Proactively refreshing...");
                    let _ = self.refresh(profile, config, &reqwest::header::HeaderMap::new()).await;
                }
            }
            Err(_) => {
                tracing::info!(target: "sys", "Self-Built token missing. Triggering proactive push...");
                let _ = self.trigger_push(profile, config, false).await;
            }
        }
        Ok(())
    }

    fn requires_initial_push(&self, config: &Config) -> bool {
        // If AppTicket is missing, we need an initial push
        let _pool = self.pool.clone();
        let _app_key = config.app_key.clone();
        
        // Note: In a real sync context this might be tricky, but here we can check synchronously if the pool allows or just return true to be safe
        // For simplicity and to match bridge.rs logic:
        true 
    }

    async fn handle_platform_event(&self, _profile: &str, config: &Config, event: crate::auth::provider::PlatformEvent) -> Result<()> {
        match event {
            crate::auth::provider::PlatformEvent::AppTicket(ticket_val) => {
                let ticket = crate::auth::models::Ticket {
                    value: ticket_val,
                    created_at: Utc::now(),
                };
                self.pool.set_app_ticket(&config.app_key, &ticket).await?;
                tracing::info!(target: "sys", "Self-Built AppTicket updated from platform push");
                Ok(())
            }
            _ => Ok(()) // Self-built doesn't care about TempAuthCode
        }
    }

    fn get_daemon_display_info(&self, is_running: bool) -> (String, String) {
        let name = "Stream Bridge (Daemon)";
        let tip = if is_running { "同步状态: [ACTIVE]" } else { "若需实现实时消息同步，请运行 'cowen daemon start'" };
        (name.to_string(), tip.to_string())
    }

    fn requires_ticket(&self) -> bool {
        true
    }

    async fn perform_login(&self, profile: &str, config: &Config, force: bool, _finalize: Option<&str>) -> Result<()> {
        if force {
            println!("🔄 Force refresh requested. Attempting immediate Token refresh using existing Ticket...");
        } else {
            println!("📡 Checking current credentials for profile '{}'...", profile);
        }

        // Attempt immediate refresh
        match self.refresh(profile, config, &reqwest::header::HeaderMap::new()).await {
            Ok(_) => {
                println!("✅ Success! AccessToken has been refreshed.");
                Ok(())
            }
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("Missing app_ticket") {
                    println!("⚠️  Local AppTicket missing or expired. Requesting a new one...");
                } else {
                    println!("⚠️  Refresh failed: {}", err_msg);
                }
                println!("📡 Triggering a fresh platform push...");
                self.trigger_push(profile, config, force).await?;
                
                println!("⏳ Waiting for platform (AppTicket) push...");
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                match self.refresh(profile, config, &reqwest::header::HeaderMap::new()).await {
                    Ok(_) => {
                        println!("✅ Success! AccessToken obtained.");
                        Ok(())
                    }
                    Err(e) => Err(anyhow::anyhow!("Failed to obtain token: {}", e))
                }
            }
        }
    }

    async fn get_diagnostics(&self, ctx: &crate::core::status::StatusContext<'_>) -> Result<Vec<crate::core::status::StatusEntry>> {
        use crate::core::status::{StatusEntry, StatusLevel, AsStatusUI, collect_daemon_status};
        
        enum SelfBuiltTemplate {
            SecurityVault,
            #[allow(dead_code)]
            AccessToken,
            AppTicket,
            AuthSummary,
        }
        impl AsStatusUI for SelfBuiltTemplate {
            fn ui(&self) -> (String, String) {
                match self {
                    Self::SecurityVault => ("Security (Vault)".to_string(), "🛡️".to_string()),
                    Self::AccessToken => ("AccessToken".to_string(), "🔑".to_string()),
                    Self::AppTicket => ("AppTicket".to_string(), "🎫".to_string()),
                    Self::AuthSummary => ("AccessToken Status".to_string(), "🔑".to_string()),
                }
            }
        }

        let mut results = Vec::new();
        let profile = &ctx.profile;
        let config = ctx.config;
        let vault = ctx.vault.clone();

        // 1. Authentication Summary (Legacy get_status_entries logic)
        let mut auth_entries = Vec::new();
        
        // 1.1 Security Check
        let mut missing: Vec<String> = Vec::new();
        let mut insecure: Vec<String> = Vec::new();
        if vault.get_secret(profile, "app_secret").await.is_err() {
            if config.app_secret.trim().is_empty() { missing.push("app_secret".to_string()); }
            else { insecure.push("app_secret".to_string()); }
        }
        if vault.get_secret(profile, "certificate").await.is_err() {
            if config.certificate.trim().is_empty() { missing.push("certificate".to_string()); }
            else { insecure.push("certificate".to_string()); }
        }
        if vault.get_secret(profile, "encrypt_key").await.is_err() {
            if config.encrypt_key.trim().is_empty() { missing.push("encrypt_key".to_string()); }
            else { insecure.push("encrypt_key".to_string()); }
        }

        let (sec_level, sec_msg, sec_reason) = if !missing.is_empty() {
            (StatusLevel::ERROR, format!("Missing: {}", missing.join(", ")), Some("缺少必要凭据，可能导致 API 调用或解密失败。".to_string()))
        } else if !insecure.is_empty() {
            (StatusLevel::OK, "Credentials found in local config (Legacy).".to_string(), Some("提示: 凭据当前以明文存储在 YAML 中，建议重新运行 'init' 迁移至 Vault。".to_string()))
        } else {
            (StatusLevel::OK, "All core secrets are securely stored.".to_string(), None)
        };

        auth_entries.push(StatusEntry::new(SelfBuiltTemplate::SecurityVault, sec_level, sec_msg)
            .with_reason(sec_reason));

        // 1.3 AppTicket Check
        if let Ok(ticket) = vault.get_app_ticket(&config.app_key).await {
            let created_at = ticket.created_at;
            auth_entries.push(StatusEntry::new(SelfBuiltTemplate::AppTicket, StatusLevel::OK, format!("[CACHED] (Received: {})", created_at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S"))));
        } else {
            auth_entries.push(StatusEntry::new(SelfBuiltTemplate::AppTicket, StatusLevel::NONE, "[NONE] (等待 Daemon 接收推送)".to_string()));
        }

        // Wrap Authentication Summary
        if !auth_entries.is_empty() {
            let max_level = auth_entries.iter().map(|e| e.level).max_by_key(|l| match l {
                StatusLevel::ERROR => 3,
                StatusLevel::WARN => 2,
                StatusLevel::OK => 1,
                _ => 0,
            }).unwrap_or(StatusLevel::OK);

            results.push(StatusEntry::new(SelfBuiltTemplate::AuthSummary, max_level, format!("Collected {} status indicators", auth_entries.len()))
                .with_children(auth_entries));
        }

        // 2. Daemon Status
        let (found_pid, _) = crate::core::status::get_active_daemon_info(profile).await;
        let is_running = found_pid.is_some();
        let (display_name, efficiency_tip) = self.get_daemon_display_info(is_running);
        results.push(collect_daemon_status(ctx, &display_name, &efficiency_tip, self.supports_webhooks()).await?);

        Ok(results)
    }

    fn decorate_openapi_request(&self, _url: &mut String, headers: &mut reqwest::header::HeaderMap, token: &Token, config: &Config) {
        headers.insert("openToken", token.value.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
        headers.insert("appKey", config.app_key.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
    }

    async fn on_logout(&self, profile: &str, config: &Config) -> Result<()> {
        let vault = self.pool.as_vault();
        let _ = vault.delete_access_token(profile).await;
        // Also cleanup app-scoped token if it exists
        let _ = vault.delete_access_token(&format!("app:{}", config.app_key)).await;
        Ok(())
    }
}
