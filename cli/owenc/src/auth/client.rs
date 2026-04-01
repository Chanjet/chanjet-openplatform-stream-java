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
    async fn refresh_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token>;
    async fn trigger_push(&self, profile: &str, cfg: &Config) -> Result<()>;
    async fn get_openapi_spec(&self, profile: &str, cfg: &Config) -> Result<serde_json::Value>;
    async fn get_dynamic_interface_list(&self, profile: &str, cfg: &Config) -> Result<serde_json::Value>;
    async fn clear_token(&self, profile: &str) -> Result<()>;
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

    async fn perform_network_refresh(&self, profile: &str, cfg: &Config) -> Result<Token> {
        let ticket = self.pool.get_app_ticket(profile)
            .context("Missing app_ticket, please ensure daemon is running and app_ticket is received.")?;

        let url = format!("{}/v1/common/auth/selfBuiltApp/generateToken", cfg.openapi_url);
        let app_key = cfg.app_key.trim();
        let app_secret = cfg.app_secret.trim();
        
        let ticket_val = ticket.value.trim();
        let prefix = if ticket_val.len() > 8 { &ticket_val[..8] } else { ticket_val };
        tracing::info!(
            target: "sys", 
            profile = %profile, 
            "Performing network token refresh using AppTicket (prefix: {}, created_at: {})...", 
            prefix, 
            ticket.created_at
        );
        println!("[{}] 📡 [Auth] Token refresh request (with AppTicket: {}..., created: {})", 
            Utc::now(), prefix, ticket.created_at);
        
        let body = serde_json::json!({
            "appKey": app_key,
            "appSecret": app_secret,
            "appTicket": ticket.value,
            "certificate": cfg.certificate.trim(),
            "authCertificate": cfg.certificate.trim(),
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

        // Save to pool (and vault)
        self.pool.set_access_token(profile, &new_token)?;
        Ok(new_token)
    }
}

#[async_trait::async_trait]
impl<'a> Client for AuthClient<'a> {
    async fn get_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token> {
        // 1. Fast Path: Check pool
        if let Ok(token) = self.pool.get_access_token(profile) {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        // 2. Slow Path: Acquire Global Lock
        let _lock_guard = self.pool.lock(profile).context("Failed to acquire global refresh lock")?;

        // 3. Clear cache to ensure Double Check reads from Vault (disk)
        self.pool.clear_cache(profile);

        // 4. Double Check
        if let Ok(token) = self.pool.get_access_token(profile) {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        // 4. Perform network refresh
        self.perform_network_refresh(profile, cfg).await
    }

    async fn refresh_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token> {
        // Force refresh skips cache but still uses the lock to avoid races
        let _lock_guard = self.pool.lock(profile).context("Failed to acquire global refresh lock")?;
        self.pool.clear_cache(profile);
        self.perform_network_refresh(profile, cfg).await
    }

    async fn clear_token(&self, profile: &str) -> Result<()> {
        self.pool.delete_access_token(profile)
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

    async fn get_openapi_spec(&self, profile: &str, cfg: &Config) -> Result<serde_json::Value> {
        let app_dir = crate::core::config::get_app_dir();
        let cache_path = app_dir.join(format!("{}_openapi.json", profile));

        // 1. Load Cache if exists
        let cached_data = if cache_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&cache_path) {
                serde_json::from_str::<serde_json::Value>(&data).ok()
            } else { None }
        } else { None };

        // 2. Evaluation Logic (PRD v0.1.1 Cache-First Strategy)
        if let Some(spec) = cached_data {
            let metadata = std::fs::metadata(&cache_path)?;
            let elapsed = metadata.modified()?.elapsed()?.as_secs();

            // Soft TTL (1h): Use cache directly for high performance (Agent-First)
            if elapsed < 3600 {
                return Ok(spec);
            }

            // Hard TTL or Stale: Try Refresh (Pull)
            if !cfg.openapi_url.is_empty() {
                let spec_url = format!("{}/v1/common/openapi/spec", cfg.openapi_url.trim_end_matches('/'));
                if let Ok(token) = self.get_app_access_token(profile, cfg).await {
                    match self.http.get(&spec_url)
                        .header("openToken", token.value)
                        .header("appKey", &cfg.app_key)
                        .send()
                        .await {
                        Ok(resp) if resp.status().is_success() => {
                            if let Ok(fresh_spec) = resp.json::<serde_json::Value>().await {
                                let _ = self.save_spec_to_cache(&cache_path, &fresh_spec);
                                return Ok(fresh_spec);
                            }
                        }
                        _ => {
                            tracing::warn!(target: "sys", "Failed to refresh OpenAPI spec from {}, using stale cache (age: {}s)", spec_url, elapsed);
                        }
                    }
                }
            }

            // Fallback: If refresh fails but we have cache, always return it (Fail-Fallback)
            return Ok(spec);
        }

        // 3. No Cache: Must Pull Spec
        if !cfg.openapi_url.is_empty() {
            let spec_url = format!("{}/v1/common/openapi/spec", cfg.openapi_url.trim_end_matches('/'));
             if let Ok(token) = self.get_app_access_token(profile, cfg).await {
                if let Ok(resp) = self.http.get(&spec_url)
                    .header("openToken", token.value)
                    .header("appKey", &cfg.app_key)
                    .send()
                    .await {
                    if resp.status().is_success() {
                        if let Ok(spec) = resp.json::<serde_json::Value>().await {
                            let _ = self.save_spec_to_cache(&cache_path, &spec);
                            return Ok(spec);
                        }
                    }
                }
            }
        }

        // 4. Pull Spec Failed: Try Dynamic Interface List (SYKFPT-1067 Patch)
        println!("⚠️  Full OpenAPI spec unavailable. Attempting to fetch authorized interface list...");
        match self.get_dynamic_interface_list(profile, cfg).await {
            Ok(dynamic_spec) => {
                let _ = self.save_spec_to_cache(&cache_path, &dynamic_spec);
                return Ok(dynamic_spec);
            }
            Err(e) => {
                tracing::warn!(target: "sys", "Failed to fetch dynamic interface list: {}", e);
            }
        }

        // 5. Ultimate Fallback to Mock (Development Artifact)
        let spec = Self::generate_mock_spec();
        let _ = self.save_spec_to_cache(&cache_path, &spec);
        Ok(spec)
    }

    async fn get_dynamic_interface_list(&self, profile: &str, cfg: &Config) -> Result<serde_json::Value> {
        let token = self.get_app_access_token(profile, cfg).await?;
        let url = format!("{}/developer/api/apiPermissions/isv/open/getInterfaceList?size=100", cfg.openapi_url.trim_end_matches('/'));
        
        tracing::info!(target: "sys", "Fetching dynamic interface list with full OpenAPI fragments from {}", url);
        
        let resp = self.http.get(&url)
            .header("openToken", token.value)
            .header("appKey", &cfg.app_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to fetch interface list: HTTP {}", resp.status()));
        }

        let body: serde_json::Value = resp.json().await?;
        let mut combined_paths = serde_json::Map::new();
        
        if let Some(list) = body.get("value").and_then(|v| v.get("resultList")).and_then(|l| l.as_array()) {
            for item in list {
                // Each item contains its own small OpenAPI spec!
                if let Some(item_spec) = item.get("openApi") {
                    if let Some(item_paths) = item_spec.get("paths").and_then(|p| p.as_object()) {
                        for (path, methods) in item_paths {
                            combined_paths.insert(path.clone(), methods.clone());
                        }
                    }
                } else {
                    // Fallback to minimal info if openApi fragment is missing
                    let path = item.get("requestPath").and_then(|v| v.as_str()).unwrap_or("");
                    let name = item.get("interfaceName").and_then(|v| v.as_str()).unwrap_or("No Name");
                    let method = item.get("requestHttpMethod").and_then(|v| v.as_str()).unwrap_or("GET").to_lowercase();
                    
                    if !path.is_empty() {
                        let mut methods_obj = serde_json::Map::new();
                        methods_obj.insert(method, serde_json::json!({
                            "summary": name,
                            "description": format!("Authorized Interface (Basic): {}", name),
                            "responses": { "200": { "description": "OK" } }
                        }));
                        combined_paths.insert(path.to_string(), serde_json::Value::Object(methods_obj));
                    }
                }
            }
        }

        Ok(serde_json::json!({
            "openapi": "3.0.1",
            "info": {
                "title": "Authorized API Specification",
                "version": "1.0.0",
                "description": "This specification is dynamically reconstructed from authorized interface fragments."
            },
            "paths": combined_paths
        }))
    }
}

impl<'a> AuthClient<'a> {
    fn save_spec_to_cache(&self, path: &std::path::PathBuf, spec: &serde_json::Value) -> Result<()> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json_data = serde_json::to_string_pretty(spec)?;
        std::fs::write(path, json_data)?;
        Ok(())
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::pool::VaultTokenPool;
    use crate::auth::models::Token;
    use crate::core::vault::Vault;
    use axum::{routing::get, Router, Json};
    use tokio::net::TcpListener;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::collections::HashMap;
    use serde_json::json;

    pub struct MockVault {
        data: Mutex<HashMap<String, String>>,
    }
    impl MockVault {
        pub fn new() -> Self { Self { data: Mutex::new(HashMap::new()) } }
    }
    impl Vault for MockVault {
        fn get(&self, profile: &str, key: &str) -> Result<String> {
            let full_key = format!("{}:{}", profile, key);
            self.data.lock().unwrap().get(&full_key).cloned().ok_or(anyhow!("Not found"))
        }
        fn set(&self, profile: &str, key: &str, secret: &str) -> Result<()> {
            let full_key = format!("{}:{}", profile, key);
            self.data.lock().unwrap().insert(full_key, secret.to_string());
            Ok(())
        }
        fn delete(&self, _profile: &str, _key: &str) -> Result<()> { Ok(()) }
        fn clear(&self, _profile: &str) -> Result<()> { Ok(()) }
        fn lock(&self, _profile: &str) -> Result<Box<dyn std::any::Any + Send>> {
            Ok(Box::new(()))
        }
    }

    #[tokio::test]
    async fn test_spec_pull_then_fallback() -> Result<()> {
        // 1. Mock Server
        let app = Router::new()
            .route("/v1/common/openapi/spec", get(|| async {
                Json(json!({"openapi": "3.0.0", "info": {"title": "Fresh Spec"}}))
            }));
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        // 2. Setup
        let vault: Arc<dyn Vault> = Arc::new(MockVault::new());
        let pool = VaultTokenPool::new(vault.clone());
        pool.set_access_token("test", &Token { value: "test-token".into(), expires_at: Utc::now() + Duration::hours(1), created_at: Utc::now() })?;
        
        let client = AuthClient::new(&pool);
        let mut config = Config::default_with_profile("test");
        config.openapi_url = format!("http://{}", addr);

        // 3. Run (Will pull from mock server)
        let spec = client.get_openapi_spec("test", &config).await?;
        assert_eq!(spec["info"]["title"], "Fresh Spec");

        Ok(())
    }
}
