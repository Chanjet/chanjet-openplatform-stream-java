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

pub struct SelfBuiltProvider<'a> {
    pool: &'a (dyn TokenPool + Send + Sync),
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

impl<'a> SelfBuiltProvider<'a> {
    #[allow(dead_code)]
    pub fn new(pool: &'a (dyn TokenPool + Send + Sync)) -> Self {
        Self {
            pool,
            http_sender: Arc::new(ReqwestSender::new()),
            refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub fn with_sender(pool: &'a (dyn TokenPool + Send + Sync), sender: Arc<dyn HttpSender>) -> Self {
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
            match self.pool.get_app_ticket(profile) {
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

        self.pool.set_access_token(profile, &new_token)?;
        Ok(new_token)
    }

    pub async fn trigger_push(&self, profile: &str, cfg: &Config, force: bool) -> Result<()> {
        let vault = self.pool.as_vault();
        
        if !force {
            let now = Utc::now();
            let last_attempt = if let Some(ts_str) = vault.get(profile, "push_last_attempt_ts").ok() {
                chrono::DateTime::parse_from_rfc3339(&ts_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now() - Duration::hours(1))
            } else {
                Utc::now() - Duration::hours(1)
            };
            
            let level: u32 = vault.get(profile, "push_backoff_level")
                .unwrap_or_else(|_| "0".to_string())
                .parse()
                .unwrap_or(0);
                
            let wait_secs = std::cmp::min(86400, 60 * (1 << std::cmp::min(level, 10)));
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
            
            let level: u32 = vault.get(profile, "push_backoff_level").unwrap_or_else(|_| "0".to_string()).parse().unwrap_or(0);
            if status == 409 {
                let _ = vault.set(profile, "push_backoff_level", &(level + 1).to_string());
            }
            let _ = vault.set(profile, "push_last_attempt_ts", &Utc::now().to_rfc3339());
            
            return Err(anyhow!("Failed to trigger push (HTTP {}): {}", status, err_text));
        }

        let _ = vault.set(profile, "push_backoff_level", "0");
        let _ = vault.set(profile, "push_last_attempt_ts", &Utc::now().to_rfc3339());

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
impl<'a> AuthProvider for SelfBuiltProvider<'a> {
    async fn get_token(&self, profile: &str, cfg: &Config) -> Result<Token> {
        if let Ok(token) = self.pool.get_access_token(profile) {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        let _guard = self.refresh_lock.lock().await;
        
        if let Ok(token) = self.pool.get_access_token(profile) {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        self.pool.clear_cache(profile);
        self.perform_network_refresh(profile, cfg).await
    }

    async fn refresh(&self, profile: &str, cfg: &Config) -> Result<Token> {
        let _guard = self.refresh_lock.lock().await;
        self.pool.clear_cache(profile);
        self.perform_network_refresh(profile, cfg).await
    }
}
