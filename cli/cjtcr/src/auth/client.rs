use crate::core::config::Config;
use crate::auth::pool::TokenPool;
use crate::auth::models::Token;
use anyhow::{Result, anyhow, Context};
use serde::Deserialize;
use chrono::{Utc, Duration};
use reqwest::Client as HttpClient;

#[async_trait::async_trait]
pub trait Client {
    async fn get_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token>;
    async fn trigger_push(&self, profile: &str, cfg: &Config) -> Result<()>;
    async fn get_openapi_spec(&self, profile: &str, cfg: &Config) -> Result<serde_json::Value>;
}

pub struct AuthClient<'a> {
    pool: &'a (dyn TokenPool + Send + Sync),
    http: HttpClient,
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

impl<'a> AuthClient<'a> {
    pub fn new(pool: &'a (dyn TokenPool + Send + Sync)) -> Self {
        Self {
            pool,
            http: HttpClient::new(),
        }
    }
}

#[async_trait::async_trait]
impl<'a> Client for AuthClient<'a> {
    async fn get_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token> {
        // 1. Check pool
        if let Ok(token) = self.pool.get_access_token(profile) {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        // 2. Perform network refresh
        let ticket = self.pool.get_app_ticket(profile)
            .context("Missing app_ticket, please ensure daemon is running and app_ticket is received.")?;

        let url = format!("{}/v1/common/auth/selfBuiltApp/generateToken", cfg.openapi_url);
        let app_key = cfg.app_key.trim();
        let app_secret = cfg.app_secret.trim();
        
        println!("📡 [Auth] Fetching AccessToken from: {}", url);
        
        let body = serde_json::json!({
            "appTicket": ticket.value,
            "certificate": cfg.certificate.trim(),
        });

        let resp = self.http.post(&url)
            .header("appKey", app_key)
            .header("appSecret", app_secret)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await?;
            return Err(anyhow!("Platform auth failed (HTTP {}): {}", status, err_text));
        }

        let token_resp: PlatformTokenResponse = resp.json().await?;
        
        if !token_resp.result {
            return Err(anyhow!("Platform error: {:?}", token_resp.error));
        }

        let val = token_resp.value.context("Platform returned success but value is empty")?;
        
        let new_token = Token {
            value: val.access_token,
            expires_at: Utc::now() + Duration::seconds(val.expires_in),
        };

        // 3. Save to pool
        self.pool.set_access_token(profile, &new_token)?;

        Ok(new_token)
    }

    async fn trigger_push(&self, _profile: &str, cfg: &Config) -> Result<()> {
        let url = format!("{}/auth/appTicket/resend", cfg.openapi_url);
        let app_key = cfg.app_key.trim();
        let app_secret = cfg.app_secret.trim();
        
        println!("📡 [Auth] Triggering push to: {} (appKey: {}...)", url, &app_key[..std::cmp::min(app_key.len(), 5)]);
        
        let body = serde_json::json!({});

        let resp = self.http.post(&url)
            .header("appKey", app_key)
            .header("appSecret", app_secret)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await?;
            return Err(anyhow!("Failed to trigger push (HTTP {}): {}", status, err_text));
        }

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

    async fn get_openapi_spec(&self, profile: &str, _cfg: &Config) -> Result<serde_json::Value> {
        let app_dir = crate::core::config::get_app_dir();
        let cache_path = app_dir.join(format!("{}_openapi.json", profile));

        // 1. Try Cache with Staleness Check (1 hour TTL)
        if cache_path.exists() {
            let metadata = std::fs::metadata(&cache_path)?;
            let is_stale = metadata.modified()
                .map(|m| m.elapsed().map(|e| e.as_secs() > 3600).unwrap_or(true))
                .unwrap_or(true);

            if !is_stale {
                let data = std::fs::read_to_string(&cache_path)?;
                if let Ok(spec) = serde_json::from_str(&data) {
                    return Ok(spec);
                }
            }
        }

        // 2. Generate Mock Spec (Same as Go version for parity)
        let spec = Self::generate_mock_spec();

        // 3. Save Cache
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json_data = serde_json::to_string_pretty(&spec)?;
        let _ = std::fs::write(cache_path, json_data);

        Ok(spec)
    }
}

impl<'a> AuthClient<'a> {
    fn generate_mock_spec() -> serde_json::Value {
        let template_str = include_str!("mock_openapi.json");
        let mut spec: serde_json::Value = serde_json::from_str(template_str).unwrap_or_else(|_| serde_json::json!({}));
        
        crate::core::openapi::flatten(&mut spec);
        spec
    }
}

pub fn find_matching_spec_path(req_path: &str, spec: &serde_json::Value) -> Option<String> {
    if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
        if paths.contains_key(req_path) {
            return Some(req_path.to_string());
        }
        let req_segments: Vec<&str> = req_path.split('/').filter(|s| !s.is_empty()).collect();
        for spec_path in paths.keys() {
            let spec_segments: Vec<&str> = spec_path.split('/').filter(|s| !s.is_empty()).collect();
            if req_segments.len() == spec_segments.len() {
                let mut match_ok = true;
                for (req_seg, spec_seg) in req_segments.iter().zip(spec_segments.iter()) {
                    if spec_seg.starts_with('{') && spec_seg.ends_with('}') {
                        continue; // matches path variable
                    }
                    if req_seg != spec_seg {
                        match_ok = false;
                        break;
                    }
                }
                if match_ok {
                    return Some(spec_path.clone());
                }
            }
        }
    }
    None
}

pub fn get_operation(spec: &serde_json::Value, path: &str, method: &str) -> Option<serde_json::Value> {
    if let Some(matched_path) = find_matching_spec_path(path, spec) {
        spec.get("paths")?
            .get(&matched_path)?
            .get(method.to_lowercase())
            .cloned()
    } else {
        None
    }
}

pub fn is_path_in_whitelist(req_path: &str, spec: &serde_json::Value) -> bool {
    find_matching_spec_path(req_path, spec).is_some()
}
