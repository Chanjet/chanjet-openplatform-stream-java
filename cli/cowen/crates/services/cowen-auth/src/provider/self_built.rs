use crate::client::HttpSender;
use crate::pool::TokenPool;
use crate::provider::AuthProvider;
use cowen_common::config::Config;
use cowen_common::{CowenError, CowenResult};
use cowen_infra::obfs;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
struct PlatformTokenResponse {
    pub result: Option<bool>,
    pub value: Option<PlatformTokenValue>,
    pub code: Option<String>,
    pub message: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlatformTokenValue {
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: Option<i64>,
    #[serde(rename = "expiresIn")]
    pub expires_in: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlatformResendResponse {
    pub code: String,
    pub message: Option<serde_json::Value>,
}

pub struct SelfBuiltProvider {
    pool: Arc<dyn TokenPool + Send + Sync>,
    http_sender: Arc<dyn HttpSender>,
    refresh_lock: Arc<tokio::sync::Mutex<()>>,
}

impl SelfBuiltProvider {
    pub fn new(pool: Arc<dyn TokenPool + Send + Sync>, sender: Arc<dyn HttpSender>) -> Self {
        Self {
            pool,
            http_sender: sender,
            refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    async fn perform_network_refresh(
        &self,
        profile: &str,
        cfg: &Config,
    ) -> CowenResult<cowen_common::models::Token> {
        if cfg.app_key.trim().is_empty() || cfg.app_secret.trim().is_empty() {
            return Err(CowenError::Config(format!("Credential Missing: AppKey or AppSecret is empty for profile '{}'. Please run 'cowen init' to configure your environment.", profile)));
        }

        tracing::info!(target: "sys", profile = %profile, "Proceeding with Self-Built token exchange...");

        let mut retry_count = 0;
        loop {
            use rand::Rng;
            let jitter_ms = rand::thread_rng().gen_range(50..500);
            tokio::time::sleep(std::time::Duration::from_millis(jitter_ms)).await;

            if let Ok(token) = self.pool.get_app_access_token(&cfg.app_key).await {
                if !token.is_expired() {
                    tracing::info!(target: "sys", profile = %profile, "Token was refreshed by another process while waiting.");
                    return Ok(token);
                }
            }

            let app_ticket = match self.pool.as_vault().get_app_ticket(cfg.app_key.trim()).await {
                Ok(t) => t.value,
                Err(_) => {
                    match self.pool.as_vault().get_secret(profile, "app_ticket").await {
                        Ok(v) => v,
                        Err(_) => {
                            if retry_count < 3 {
                                tracing::warn!(target: "sys", profile = %profile, "Missing appTicket. Triggering push... (Attempt {}/3)", retry_count + 1);
                                let _ = self.trigger_push_internal(profile, cfg, true).await;
                                tracing::info!(target: "sys", profile = %profile, "Waiting for new AppTicket dispatch (up to 35s)...");
                                let mut waited = 0;
                                while waited < 35 {
                                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                    if self.pool.as_vault().get_app_ticket(cfg.app_key.trim()).await.is_ok() {
                                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                        break;
                                    }
                                    waited += 1;
                                }
                                retry_count += 1;
                                continue;
                            } else {
                                return Err(CowenError::Auth("Missing appTicket. The platform has not pushed an AppTicket yet. Please ensure daemon is running.".to_string()));
                            }
                        }
                    }
                }
            };

            // 1. Build Request
            let app_cfg = cowen_config::ConfigManager::load_app_config_sync(&cowen_common::config::get_app_dir())?;
            let path = obfs!("/v1/common/auth/selfBuiltApp/generateToken");
            let url = format!(
                "{}{}{}appTicket={}",
                app_cfg.openapi_url.trim_end_matches('/'),
                path,
                if path.contains('?') { "&" } else { "?" },
                urlencoding::encode(&app_ticket)
            );
            let mut headers = HeaderMap::new();
            headers.insert(
                "appKey",
                cfg.app_key
                    .trim()
                    .parse()
                    .unwrap_or(reqwest::header::HeaderValue::from_static("")),
            );
            headers.insert(
                "appSecret",
                cfg.app_secret
                    .trim()
                    .parse()
                    .unwrap_or(reqwest::header::HeaderValue::from_static("")),
            );

            let mut body_map = serde_json::Map::new();

            // 🚀 OCP: Include certificate if provided (Required for some platform versions)
            if !cfg.certificate.trim().is_empty() {
                body_map.insert(
                    "certificate".to_string(),
                    serde_json::Value::String(cfg.certificate.clone()),
                );
            }
            // 🚀 E2E FIX: Ensure appTicket is also passed in the body so strict validation passes
            if !app_ticket.is_empty() {
                body_map.insert(
                    "appTicket".to_string(),
                    serde_json::Value::String(app_ticket.clone()),
                );
            }

            tracing::info!(target: "sys", profile = %profile, "URL for generateToken: {}", url);

            // 2. Execute
            let resp = self
                .http_sender
                .post(&url, headers, serde_json::Value::Object(body_map))
                .await?;
            if !resp.is_success() {
                let status = resp.status;
                let safe_err = cowen_common::utils::mask_sensitive_json(&resp.body);
                return Err(CowenError::Auth(format!(
                    "Platform auth failed (HTTP {}): {}",
                    status, safe_err
                )));
            }

            let token_resp: PlatformTokenResponse = resp.json().await?;

            // Success if result is true OR code is 200
            let is_success =
                token_resp.result.unwrap_or(false) || token_resp.code.as_deref() == Some("200");
            if !is_success || token_resp.value.is_none() {
                let code_str = token_resp.code.as_deref().unwrap_or("");
                let err_val = token_resp.error.clone().or(token_resp.message.clone());
                let err_msg = match err_val {
                    Some(serde_json::Value::String(s)) => s,
                    Some(v) => v.to_string(),
                    None => "Unknown platform error".to_string(),
                };

                if retry_count < 3
                    && (code_str == "4041"
                        || code_str == "4031"
                        || code_str == "4019"
                        || err_msg.contains("4041")
                        || err_msg.contains("4031")
                        || err_msg.contains("4019"))
                {
                    tracing::warn!(target: "sys", profile = %profile, "AppTicket expired/invalid ({}). Clearing and triggering push... (Attempt {}/3)", err_msg, retry_count + 1);
                    let _ = self
                        .pool
                        .as_vault()
                        .delete_app_ticket(cfg.app_key.trim())
                        .await;
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                    let _ = self.trigger_push_internal(profile, cfg, true).await;

                    tracing::info!(target: "sys", profile = %profile, "Waiting for new AppTicket dispatch (up to 20s)...");
                    let mut waited = 0;
                    while waited < 35 {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        if self
                            .pool
                            .as_vault()
                            .get_app_ticket(cfg.app_key.trim())
                            .await
                            .is_ok()
                        {
                            tracing::info!(target: "sys", profile = %profile, "Received new AppTicket.");
                            // Additional brief sleep to ensure ticket is fully propagated or avoid hitting platform replication delay
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            break;
                        }
                        waited += 1;
                    }
                    retry_count += 1;
                    continue;
                }

                return Err(CowenError::Auth(format!("Platform error: {}", err_msg)));
            }

            let val = token_resp.value.unwrap();
            let expires_at = if let Some(ts) = val.expires_at {
                DateTime::from_timestamp(ts / 1000, 0)
                    .unwrap_or_else(|| Utc::now() + chrono::Duration::hours(2))
            } else if let Some(secs) = val.expires_in {
                Utc::now() + chrono::Duration::seconds(secs as i64)
            } else {
                Utc::now() + chrono::Duration::hours(2)
            };

            let token = cowen_common::models::Token {
                value: val.access_token,
                expires_at,
                created_at: Utc::now(),
            };

            // 3. Persist
            self.pool.set_app_access_token(&cfg.app_key, &token).await?;
            tracing::info!(target: "sys", profile = %profile, "AccessToken successfully rotated from network");

            return Ok(token);
        }
    }

    async fn trigger_push_internal(
        &self,
        profile: &str,
        cfg: &Config,
        force: bool,
    ) -> CowenResult<()> {
        if cfg.app_key.trim().is_empty() || cfg.app_secret.trim().is_empty() {
            return Err(CowenError::Config(format!(
                "Missing AppKey or AppSecret for profile '{}'. Please run 'cowen init' first.",
                profile
            )));
        }

        // 1. Process-level mutex to avoid local concurrent race condition
        static RESEND_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> =
            std::sync::OnceLock::new();
        let mutex = RESEND_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
        let _lock = mutex.lock().await;

        // 2. Distributed de-duplication lock (using primary_store set_token)
        let store = self.pool.as_vault().primary_store();
        let lock_key = format!("lock:resend:{}", cfg.app_key);

        if let Ok(expire_val) = store.get_token(profile, &lock_key).await {
            if let Ok(expire_ts) = expire_val.parse::<i64>() {
                if Utc::now().timestamp() < expire_ts {
                    tracing::info!(target: "sys", profile = %profile, app_key = %cfg.app_key, "Resend push is locked by a concurrent task. Skipping resend request.");
                    return Ok(());
                }
            }
        }

        // Acquire distributed lock for 15 seconds
        let new_expire_ts = Utc::now().timestamp() + 15;
        let _ = store
            .set_token(profile, &lock_key, &new_expire_ts.to_string(), 15)
            .await;

        let app_cfg = cowen_config::ConfigManager::load_app_config_sync(&cowen_common::config::get_app_dir())?;
        let url = format!(
            "{}{}",
            app_cfg.openapi_url.trim_end_matches('/'),
            obfs!("/auth/appTicket/resend")
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            "appKey",
            cfg.app_key
                .trim()
                .parse()
                .unwrap_or(reqwest::header::HeaderValue::from_static("")),
        );
        headers.insert(
            "appSecret",
            cfg.app_secret
                .trim()
                .parse()
                .unwrap_or(reqwest::header::HeaderValue::from_static("")),
        );

        let mut body_map = serde_json::Map::new();
        if force {
            body_map.insert("force".to_string(), serde_json::Value::Bool(true));
        }

        let resp = match self
            .http_sender
            .post(&url, headers, serde_json::Value::Object(body_map))
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let _ = store.delete_token(profile, &lock_key).await;
                return Err(e);
            }
        };

        if !resp.is_success() {
            let _ = store.delete_token(profile, &lock_key).await;
            let status = resp.status;
            let err_text = cowen_common::utils::mask_sensitive_json(&resp.body);
            return Err(CowenError::Auth(format!(
                "Failed to trigger push (HTTP {}): {}",
                status, err_text
            )));
        }

        let resend_resp: PlatformResendResponse = match resp.json().await {
            Ok(r) => r,
            Err(e) => {
                let _ = store.delete_token(profile, &lock_key).await;
                return Err(CowenError::Auth(format!(
                    "Failed to parse resend response: {}",
                    e
                )));
            }
        };

        if resend_resp.code != "200" {
            let _ = store.delete_token(profile, &lock_key).await;
            let err_msg = match resend_resp.message {
                Some(serde_json::Value::String(s)) => s,
                Some(v) => v.to_string(),
                None => "Unknown error".to_string(),
            };
            return Err(CowenError::Auth(format!(
                "Platform error: {} - {}",
                resend_resp.code, err_msg
            )));
        }

        tracing::info!(target: "sys", profile = %profile, "Proactive AppTicket push triggered");
        Ok(())
    }

    fn decorate_openapi_request_internal(
        &self,
        _url: &mut String,
        headers: &mut HeaderMap,
        token: &cowen_common::models::Token,
        config: &Config,
    ) {
        if let Ok(val) = token.value.parse() {
            headers.insert("openToken", val);
        }
        if let Ok(val) = config.app_key.trim().parse() {
            headers.insert("appKey", val);
        }
    }
}

#[async_trait]
impl AuthProvider for SelfBuiltProvider {
    async fn hydrate_config(
        &self,
        profile: &str,
        config: &mut Config,
        vault: std::sync::Arc<dyn cowen_common::vault::Vault>,
    ) -> CowenResult<()> {
        if let Ok(as_val) = vault.get_secret(profile, "app_secret").await {
            config.app_secret = as_val;
        }
        if let Ok(cert) = vault.get_secret(profile, "certificate").await {
            config.certificate = cert;
        }
        if let Ok(ek) = vault.get_secret(profile, "encrypt_key").await {
            config.encrypt_key = ek;
        }
        Ok(())
    }

    async fn on_logout(&self, profile: &str, config: &Config) -> CowenResult<()> {
        let vault = self.pool.as_vault();
        let _ = vault.delete_access_token(profile).await;
        let _ = vault.delete_refresh_token(profile).await;

        let app_key = config.app_key.trim();
        if !app_key.is_empty() {
            let _ = vault.delete_app_access_token(app_key).await;
            let _ = vault.delete_app_ticket(app_key).await;
        }
        Ok(())
    }

    async fn get_token(
        &self,
        profile: &str,
        config: &Config,
        _headers: &HeaderMap,
    ) -> CowenResult<cowen_common::models::Token> {
        
        tracing::debug!(target: "sys", profile = %profile, app_key = %config.app_key, "Attempting to retrieve token");
        // 1. Try Cache
        match self.pool.get_app_access_token(&config.app_key).await {
            Ok(token) => {
                if !token.is_expired() {
                    tracing::debug!(target: "sys", profile = %profile, app_key = %config.app_key, token = "<ACCESS_TOKEN>", "Found valid cached token");
                    return Ok(token);
                }
                tracing::debug!(target: "sys", profile = %profile, "Cached token expired");
            }
            Err(e) => {
                tracing::debug!(target: "sys", profile = %profile, error = %e, app_key = %config.app_key, "Cache miss or error");
            }
        }

        // 2. Lock & Refresh
        let _lock = self.refresh_lock.lock().await;

        // Re-check after lock
        if let Ok(token) = self.pool.get_app_access_token(&config.app_key).await {
            if !token.is_expired() {
                tracing::debug!(target: "sys", profile = %profile, token = "<ACCESS_TOKEN>", "Found valid cached token after lock");
                return Ok(token);
            }
        }

        self.perform_network_refresh(profile, config).await
    }

    async fn refresh(
        &self,
        profile: &str,
        config: &Config,
        _headers: &HeaderMap,
    ) -> CowenResult<cowen_common::models::Token> {
        let _lock = self.refresh_lock.lock().await;
        self.perform_network_refresh(profile, config).await
    }

    async fn trigger_push(&self, profile: &str, cfg: &Config, force: bool) -> CowenResult<()> {
        self.trigger_push_internal(profile, cfg, force).await
    }

    async fn initialize(
        &self,
        profile: &str,
        config: &mut Config,
        vault: std::sync::Arc<dyn cowen_common::vault::Vault>,
        cfg_mgr: &cowen_config::ConfigManager,
        params: crate::provider::InitParams,
        daemon_service: Option<std::sync::Arc<dyn cowen_common::daemon::DaemonService>>,
    ) -> CowenResult<()> {
        if let Some(ak) = params.app_key {
            config.app_key = cowen_common::utils::sanitize_credential(&ak);
        }
        if let Some(as_val) = params.app_secret {
            config.app_secret = cowen_common::utils::sanitize_credential(&as_val);
        }
        if let Some(cert) = params.certificate {
            config.certificate = cowen_common::utils::sanitize_credential(&cert);
        }
        if let Some(ek) = params.encrypt_key {
            config.encrypt_key = cowen_common::utils::sanitize_credential(&ek);
        }
        if let Some(wt) = params.webhook_target {
            config.webhook_target = cowen_common::utils::sanitize_credential(&wt);
        }

        if let Some(pp) = params.proxy_port {
            config.proxy_port = pp;
        }

        config.app_mode = cowen_common::models::AuthMode::SelfBuilt;

        // 🚀 Validation: SelfBuilt mode REQUIRES all core credentials
        if config.app_key.trim().is_empty() {
            return Err(CowenError::Config(
                "Missing mandatory parameter: --app-key".to_string(),
            ));
        }
        if config.app_secret.trim().is_empty() {
            return Err(CowenError::Config(
                "Missing mandatory parameter: --app-secret".to_string(),
            ));
        }
        if config.certificate.trim().is_empty() {
            return Err(CowenError::Config(
                "Missing mandatory parameter: --certificate".to_string(),
            ));
        }
        if config.encrypt_key.trim().is_empty() {
            return Err(CowenError::Config(
                "Missing mandatory parameter: --encrypt-key".to_string(),
            ));
        }

        // Persist non-sensitive to app.yaml via ConfigManager
        cfg_mgr.save(profile, config).await?;

        // Persist sensitive to Vault
        vault
            .set_secret(profile, "app_secret", &config.app_secret)
            .await?;
        vault
            .set_secret(profile, "certificate", &config.certificate)
            .await?;
        vault
            .set_secret(profile, "encrypt_key", &config.encrypt_key)
            .await?;

        println!(
            "✅ Configuration saved for profile: \x1b[1;32m{}\x1b[0m",
            profile
        );

        if params.auto_start {
            if let Some(svc) = daemon_service {
                println!("📡 Starting background daemon to maintain AppTicket...");
                let _ = svc.start_daemon(profile).await;

                if config.proxy_enabled && config.proxy_port != 0 {
                    let mut retries = 20;
                    while retries > 0 {
                        if cowen_common::status::is_port_responsive(config.proxy_port).await {
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        retries -= 1;
                    }
                } else {
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                }

                println!("🚀 Mode is 'SelfBuilt': Triggering proactive AppTicket push...");
                let _ = self.trigger_push_internal(profile, config, false).await;
            }
        } else {
            println!("💡 \x1b[1m提示\x1b[0m: 'SelfBuilt' 模式依赖平台主动推送凭证。");
            println!("   建议运行 \x1b[33mcowen daemon start\x1b[0m 以保持后台监听。");
        }

        Ok(())
    }

    async fn requires_initial_push(&self, _cfg: &Config) -> bool {
        // Check if ticket is missing or older than 50 minutes
        match self.pool.as_vault().get_app_ticket(&_cfg.app_key).await { Ok(ticket) => {
            let age = chrono::Utc::now()
                .signed_duration_since(ticket.created_at)
                .num_minutes();
            age > 50
        } _ => {
            true
        }}
    }

    async fn handle_platform_event(
        &self,
        profile: &str,
        cfg: &Config,
        event: crate::provider::PlatformEvent,
    ) -> CowenResult<()> {
        match event {
            crate::provider::PlatformEvent::AppTicket(ticket_val) => {
                let ticket = cowen_common::models::Ticket {
                    value: ticket_val,
                    created_at: Utc::now(),
                };
                self.pool
                    .as_vault()
                    .save_app_ticket(&cfg.app_key, ticket)
                    .await?;
                tracing::info!(target: "sys", profile = %profile, "✅ AppTicket updated via PlatformEvent and saved to vault");
                eprintln!(
                    "✨ 收到平台推送的新 AppTicket，已同步至 Vault (Profile: {})",
                    profile
                );

                // Proactively refresh token if it's about to expire or missing
                let pool = self.pool.clone();
                let profile = profile.to_string();
                let cfg = cfg.clone();
                let provider_clone = Self::new(pool, self.http_sender.clone());

                tokio::spawn(async move {
                    // Delay proactive refresh to allow any active CLI login commands to complete and cache the token, avoiding a race condition.
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                    
                    let should_refresh =
                        match provider_clone.pool.get_app_access_token(&cfg.app_key).await {
                            Ok(t) => t.is_expired(),
                            Err(_) => true,
                        };
                    if should_refresh {
                        tracing::info!(target: "sys", profile = %profile, "Missing or expired token, triggering proactive refresh using new AppTicket");
                        let _ = provider_clone.perform_network_refresh(&profile, &cfg).await;
                    }
                });
            }
            _ => {
                tracing::debug!(target: "sys", "SelfBuiltProvider ignored non-relevant PlatformEvent");
            }
        }
        Ok(())
    }

    async fn perform_login(
        &self,
        profile: &str,
        cfg: &Config,
        force: bool,
        _finalize: Option<&str>,
        _daemon_service: Option<std::sync::Arc<dyn cowen_common::daemon::DaemonService>>,
    ) -> CowenResult<()> {
        if force {
            self.refresh(profile, cfg, &HeaderMap::new()).await?;
            println!("✅ Token forcefully refreshed from network.");
        } else {
            self.get_token(profile, cfg, &HeaderMap::new()).await?;
            println!("✅ Token is active and ready.");
        }
        Ok(())
    }

    async fn get_diagnostics(
        &self,
        ctx: &cowen_common::status::StatusContext<'_>,
    ) -> CowenResult<Vec<cowen_common::status::StatusEntry>> {
        use cowen_common::status::{CommonTemplate, StatusEntry, StatusLevel};
        let mut entries = Vec::new();
        let vault = self.pool.as_vault();

        // 1. Security Check
        let mut missing = Vec::new();
        let has_secret = vault.get_secret(&ctx.profile, "app_secret").await.is_ok()
            || !ctx.config.app_secret.is_empty();
        let has_encrypt_key = vault.get_secret(&ctx.profile, "encrypt_key").await.is_ok()
            || !ctx.config.encrypt_key.is_empty();

        if !has_secret {
            missing.push("app_secret".to_string());
        }
        if !has_encrypt_key {
            missing.push("encrypt_key".to_string());
        }

        let (sec_level, sec_msg) = if missing.is_empty() {
            (
                StatusLevel::OK,
                "All core secrets are securely stored.".to_string(),
            )
        } else {
            (
                StatusLevel::WARN,
                format!("Missing: {}", missing.join(", ")),
            )
        };
        entries.push(StatusEntry::new(
            CommonTemplate::Custom("Security (Vault)".to_string(), "🛡️".to_string()),
            sec_level,
            sec_msg,
        ));

        // 1.5 Decryption Key Check
        let app_secret_val = vault.get_secret(&ctx.profile, "app_secret").await.unwrap_or_else(|_| ctx.config.app_secret.clone());
        let encrypt_key_val = vault.get_secret(&ctx.profile, "encrypt_key").await.unwrap_or_else(|_| ctx.config.encrypt_key.clone());
        
        let decrypt_key_raw = if !encrypt_key_val.is_empty() {
            &encrypt_key_val
        } else {
            &app_secret_val
        };
        let decrypt_key = cowen_common::utils::sanitize_credential(decrypt_key_raw);

        let (dk_level, dk_msg) = if decrypt_key.is_empty() {
            (
                StatusLevel::ERROR,
                "Decryption key is missing (both encrypt_key and app_secret are empty)".to_string(),
            )
        } else {
            let key_len = if decrypt_key.len() == 32 {
                if decrypt_key.len().is_multiple_of(2) && decrypt_key.chars().all(|c| c.is_ascii_hexdigit()) {
                    16
                } else {
                    32
                }
            } else {
                decrypt_key.len()
            };

            if key_len != 16 {
                (
                    StatusLevel::ERROR,
                    format!("Decryption key trimmed length {} is invalid. Must be 16 bytes or 32-character hex", decrypt_key.len()),
                )
            } else {
                (
                    StatusLevel::OK,
                    "Decryption key format is valid (16 bytes or 32-character hex)".to_string(),
                )
            }
        };

        entries.push(StatusEntry::new(
            CommonTemplate::Custom("Decryption Key".to_string(), "🔑".to_string()),
            dk_level,
            dk_msg,
        ));

        // 2. AppTicket Status (Optional for Self-Built)
        // 2. AppTicket Status (Optional for Self-Built)
        let ticket_res = match self
            .pool
            .as_vault()
            .get_app_ticket(&ctx.config.app_key)
            .await
        {
            Ok(t) => Ok(t),
            Err(_) => {
                // FALLBACK: Try profile-level app_ticket secret
                let vault = self.pool.as_vault();
                let val = vault.get_secret(&ctx.profile, "app_ticket").await;
                match val {
                    Ok(v) => {
                        let created_str = vault
                            .get_secret(&ctx.profile, "app_ticket_created")
                            .await
                            .unwrap_or_default();
                        let created_at = chrono::DateTime::parse_from_rfc3339(&created_str)
                            .map(|d| d.with_timezone(&chrono::Utc))
                            .unwrap_or_else(|_| chrono::Utc::now());
                        Ok(cowen_common::models::Ticket {
                            value: v,
                            created_at,
                        })
                    }
                    Err(e) => Err(e),
                }
            }
        };

        if let Ok(t) = ticket_res {
            let age = Utc::now().signed_duration_since(t.created_at).num_minutes();
            let level = if age > 1440 {
                StatusLevel::WARN
            } else {
                StatusLevel::OK
            };
            entries.push(StatusEntry::new(
                CommonTemplate::Custom("AppTicket".to_string(), "🎫".to_string()),
                level,
                format!(
                    "[CACHED] (Received: {})",
                    t.created_at
                        .with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M:%S")
                ),
            ));
        }

        // 2. Token Pool Status
        let token_res = match self.pool.get_app_access_token(&ctx.config.app_key).await {
            Ok(t) if !t.is_expired() => Ok(t),
            _ => {
                // FALLBACK: Try profile-level access_token
                self.pool.get_access_token(&ctx.profile).await
            }
        };

        match token_res {
            Ok(t) => {
                let level = if t.is_expired() {
                    StatusLevel::ERROR
                } else {
                    StatusLevel::OK
                };
                let mut details = vec![];
                if let Some(ident) = t.extract_identity() {
                    details.push(format!("User ID: {}", ident.user_id));
                    details.push(format!("Org ID:  {}", ident.org_id));
                    details.push(format!("App ID:  {}", ident.app_id));
                }

                entries.push(
                    StatusEntry::new(
                        CommonTemplate::Custom("AccessToken".to_string(), "🔑".to_string()),
                        level,
                        format!(
                            "{} (Expires at: {})",
                            if t.is_expired() {
                                "[EXPIRED]"
                            } else {
                                "[VALID]"
                            },
                            t.real_expires_at()
                                .with_timezone(&chrono::Local)
                                .format("%Y-%m-%d %H:%M:%S")
                        ),
                    )
                    .with_details(details),
                );
            }
            Err(_) => {
                entries.push(StatusEntry::new(
                    CommonTemplate::Custom("AccessToken".to_string(), "🔑".to_string()),
                    StatusLevel::WARN,
                    "Not initialized".to_string(),
                ));
            }
        }

        // 3. Daemon Status
        let daemon_info = cowen_common::status::get_active_daemon_info(&ctx.profile);
        let (display_name, efficiency_tip) = self.get_daemon_display_info(daemon_info.is_some());
        entries.push(
            cowen_common::status::collect_daemon_status(
                ctx,
                &display_name,
                &efficiency_tip,
                self.supports_webhooks(),
                daemon_info,
            )
            .await?,
        );

        Ok(entries)
    }

    async fn on_maintenance_tick(&self, profile: &str, config: &Config) -> CowenResult<()> {
        let should_refresh = match self.pool.get_app_access_token(&config.app_key).await {
            Ok(t) => t.is_expired(),
            Err(_) => true,
        };

        if should_refresh {
            tracing::info!(target: "sys", profile = %profile, "Token expired or missing during maintenance tick, refreshing...");
            match self.perform_network_refresh(profile, config).await {
                Ok(_) => {
                    tracing::info!(target: "sys", profile = %profile, "Maintenance refresh successful")
                }
                Err(e) => {
                    tracing::error!(target: "sys", profile = %profile, error = %e, "Maintenance refresh failed")
                }
            }
        }
        Ok(())
    }

    fn requires_ticket(&self) -> bool {
        false
    }

    fn supports_webhooks(&self) -> bool {
        true
    }

    fn supports_api_call(&self) -> bool {
        true
    }

    async fn intercept_request(
        &self,
        profile: &str,
        config: &Config,
        path: &str,
        _method: &str,
        headers: reqwest::header::HeaderMap,
        _body: &[u8],
        _spec: &serde_json::Value,
    ) -> CowenResult<crate::provider::ProxyRequestAction> {
        let mut headers = headers;

        // Inject token
        match self.get_token(profile, config, &headers).await {
            Ok(token) => {
                headers.insert("openToken", token.value.parse().unwrap());
                headers.insert("appKey", config.app_key.trim().parse().unwrap());
                Ok(crate::provider::ProxyRequestAction::Forward { headers })
            }
            Err(e) => {
                // If it's a known "Missing appTicket" error, return a clear 401
                if e.to_string().contains("Missing app_ticket") {
                    return Err(CowenError::Auth(format!("Unauthorized: Profile '{}' is waiting for AppTicket push. Please ensure daemon is running. (Path: {})", profile, path)));
                }
                Err(e)
            }
        }
    }

    fn decorate_openapi_request(
        &self,
        url: &mut String,
        headers: &mut HeaderMap,
        token: &cowen_common::models::Token,
        config: &Config,
    ) {
        self.decorate_openapi_request_internal(url, headers, token, config);
    }
}
