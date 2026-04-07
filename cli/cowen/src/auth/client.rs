use crate::core::config::Config;
use crate::auth::pool::TokenPool;
use crate::auth::models::Token;
use anyhow::{Result, anyhow, Context};
use serde::Deserialize;
use chrono::{Utc, Duration};
use reqwest::Client as HttpClient;
use std::sync::Arc;

#[derive(Debug)]
pub struct SimpleResponse {
    pub status: u16,
    pub body: String,
}

impl SimpleResponse {
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    pub async fn json<T: for<'de> Deserialize<'de>>(&self) -> Result<T> {
        serde_json::from_str(&self.body).map_err(|e| anyhow!("JSON parse error: {}", e))
    }

    pub fn text(&self) -> String {
        self.body.clone()
    }
}

#[async_trait::async_trait]
pub trait HttpSender: Send + Sync {
    async fn post(&self, url: &str, headers: reqwest::header::HeaderMap, body: serde_json::Value) -> Result<SimpleResponse>;
    async fn get(&self, url: &str, headers: reqwest::header::HeaderMap) -> Result<SimpleResponse>;
}

pub struct ReqwestSender {
    client: HttpClient,
}

impl ReqwestSender {
    pub fn new() -> Self {
        Self {
            client: HttpClient::new(),
        }
    }
}

#[async_trait::async_trait]
impl HttpSender for ReqwestSender {
    async fn post(&self, url: &str, headers: reqwest::header::HeaderMap, body: serde_json::Value) -> Result<SimpleResponse> {
        let resp = self.client.post(url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow!("Network error: {}", e))?;
        
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        Ok(SimpleResponse { status, body })
    }

    async fn get(&self, url: &str, headers: reqwest::header::HeaderMap) -> Result<SimpleResponse> {
        let resp = self.client.get(url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| anyhow!("Network error: {}", e))?;
            
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        Ok(SimpleResponse { status, body })
    }
}

#[async_trait::async_trait]
pub trait Client: Send + Sync {
    async fn get_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token>;
    async fn refresh_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token>;
    async fn trigger_push(&self, profile: &str, cfg: &Config) -> Result<()>;
    async fn get_openapi_spec(&self, profile: &str, cfg: &Config, force_refresh: bool) -> Result<serde_json::Value>;
    async fn get_dynamic_interface_list(&self, profile: &str, cfg: &Config) -> Result<serde_json::Value>;
    async fn clear_token(&self, profile: &str) -> Result<()>;
}

pub struct AuthClient<'a> {
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

impl<'a> AuthClient<'a> {
    pub fn new(pool: &'a (dyn TokenPool + Send + Sync)) -> Self {
        Self {
            pool,
            http_sender: Arc::new(ReqwestSender::new()),
            refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    #[cfg(test)]
    pub fn with_sender(pool: &'a (dyn TokenPool + Send + Sync), sender: Arc<dyn HttpSender>) -> Self {
        Self {
            pool,
            http_sender: sender,
            refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    async fn perform_network_refresh(&self, profile: &str, cfg: &Config) -> Result<Token> {
        let mut attempts = 0;
        let max_attempts = 30; // Wait up to 30 seconds for cloud push
        
        let ticket = loop {
            match self.pool.get_app_ticket(profile) {
                Ok(t) => break t,
                Err(e) => {
                    if attempts >= max_attempts {
                        return Err(e).context("Missing app_ticket. The background daemon is running but hasn't received a push from the platform yet. Please wait a moment or check your network/firewall.");
                    }
                    if attempts == 0 {
                        eprintln!("⏳ AppTicket missing. Proactively triggering a platform push...");
                        // Proactively trigger push on first failure
                        if let Err(push_err) = self.trigger_push(profile, cfg).await {
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

        tracing::info!(
            target: "sys", 
            url = %url, 
            app_key = %app_key,
            "Outgoing platform auth request"
        );

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

        // Save to pool (and vault)
        self.pool.set_access_token(profile, &new_token)?;
        Ok(new_token)
    }
}

#[async_trait::async_trait]
impl<'a> Client for AuthClient<'a> {
    async fn get_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token> {
        // 1. Fast path: check pool
        if let Ok(token) = self.pool.get_access_token(profile) {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        // 2. Slow path: barrier refresh
        let _guard = self.refresh_lock.lock().await;
        
        // Re-check after acquiring lock
        if let Ok(token) = self.pool.get_access_token(profile) {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        self.pool.clear_cache(profile);
        self.perform_network_refresh(profile, cfg).await
    }

    async fn refresh_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token> {
        let _guard = self.refresh_lock.lock().await;
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
        
        let body = serde_json::json!({});

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("appKey", app_key.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
        headers.insert("appSecret", app_secret.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));

        tracing::info!(
            target: "sys", 
            url = %url, 
            app_key = %app_key,
            "Outgoing platform auth request"
        );

        let resp = self.http_sender.post(&url, headers, body).await?;

        if !resp.is_success() {
            let status = resp.status;
            let err_text = resp.text();
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

    async fn get_openapi_spec(&self, profile: &str, cfg: &Config, force_refresh: bool) -> Result<serde_json::Value> {
        let app_dir = crate::core::config::get_app_dir();
        let cache_path = app_dir.join(format!("{}_openapi.yaml", profile));

        // 1. Try Load Cache ONLY if not forcing refresh
        if !force_refresh && cache_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&cache_path) {
                if let Ok(spec) = serde_yaml::from_str::<serde_json::Value>(&data) {
                    let metadata = std::fs::metadata(&cache_path)?;
                    let elapsed = metadata.modified()?.elapsed()?.as_secs();
                    
                    // Cache is fresh (less than 1 hour)
                    if elapsed < 3600 {
                        return Ok(spec);
                    }
                    
                    // Cache expired but we have it as fallback if network fails
                    tracing::info!(target: "sys", "Local spec cache expired ({}s), attempting refresh...", elapsed);
                }
            }
        }

        if force_refresh {
            tracing::info!(target: "sys", "Force refresh requested, bypassing local cache.");
        }

        // 2. Fetch fresh spec from Platform
        if !cfg.openapi_url.is_empty() {
            let spec_url = format!("{}/v1/common/openapi/spec", cfg.openapi_url.trim_end_matches('/'));
            if let Ok(token) = self.get_app_access_token(profile, cfg).await {
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert("openToken", token.value.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
                headers.insert("appKey", cfg.app_key.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));

                match self.http_sender.get(&spec_url, headers).await {
                    Ok(resp) if resp.is_success() => {
                        if let Ok(mut fresh_spec) = resp.json::<serde_json::Value>().await {
                            Self::clean_non_standard_fields(&mut fresh_spec);
                            if let Err(e) = self.save_spec_to_cache(&cache_path, &fresh_spec) {
                                tracing::warn!(target: "sys", "Failed to save spec cache: {}", e);
                            }
                            return Ok(fresh_spec);
                        }
                    }
                    _ => {}
                }
            }
        }

        // 3. Fallback: Dynamic Discovery
        tracing::info!(target: "sys", "Full OpenAPI spec unavailable or refresh needed. Fetching real authorized interface list...");
        match self.get_dynamic_interface_list(profile, cfg).await {
            Ok(dynamic_spec) => {
                if let Err(e) = self.save_spec_to_cache(&cache_path, &dynamic_spec) {
                    tracing::warn!(target: "sys", "Failed to save dynamic spec cache: {}", e);
                }
                Ok(dynamic_spec)
            }
            Err(e) => {
                // If refresh failed but we have OLD cache, return OLD cache as last resort
                if cache_path.exists() {
                    if let Ok(data) = std::fs::read_to_string(&cache_path) {
                        if let Ok(spec) = serde_yaml::from_str::<serde_json::Value>(&data) {
                            tracing::warn!(target: "sys", "Refresh failed, falling back to expired local cache: {}", e);
                            return Ok(spec);
                        }
                    }
                }
                tracing::error!(target: "sys", "API list refresh failed: {}", e);
                Err(anyhow!("Could not refresh API list: {}", e))
            }
        }
    }

    async fn get_dynamic_interface_list(&self, profile: &str, cfg: &Config) -> Result<serde_json::Value> {
        let token = self.get_app_access_token(profile, cfg).await?;
        let base_url = format!("{}/developer/api/apiPermissions/isv/open/getInterfaceList", cfg.openapi_url.trim_end_matches('/'));
        
        let mut current_page = 0;
        let mut total_pages = 1;
        let mut combined_paths = serde_json::Map::new();

        while current_page < total_pages {
            let url = format!("{}?currentPage={}&size=100", base_url, current_page);
            tracing::info!(target: "sys", "Fetching interface list page {}/{}", current_page + 1, total_pages);
            
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("openToken", token.value.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
            headers.insert("appKey", cfg.app_key.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));

            let resp = self.http_sender.get(&url, headers).await?;

            if !resp.is_success() {
                return Err(anyhow!("Failed to fetch interface list page {}: HTTP {}", current_page, resp.status));
            }

            let body: serde_json::Value = resp.json().await?;
            let value = body.get("value").ok_or_else(|| anyhow!("Invalid response structure"))?;
            
            // Update pagination info
            total_pages = value.get("totalPages").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            
            if let Some(list) = value.get("resultList").and_then(|l| l.as_array()) {
                for item in list {
                    let mut parsed_paths = None;

                    // 1. Try to extract paths from openApi field
                    if let Some(open_api_val) = item.get("openApi") {
                        if !open_api_val.is_null() {
                            let mut item_spec = if let Some(s) = open_api_val.as_str() {
                                serde_json::from_str(s).unwrap_or(serde_json::json!({}))
                            } else {
                                open_api_val.clone()
                            };

                            Self::clean_non_standard_fields(&mut item_spec);
                            if let Some(item_paths) = item_spec.get("paths").and_then(|p| p.as_object()) {
                                parsed_paths = Some(item_paths.clone());
                            } else {
                                tracing::debug!(target: "sys", "openApi field exists but no paths found for item");
                            }
                        }
                    }

                    // 2. Merge paths or use fallback
                    if let Some(item_paths) = parsed_paths {
                        for (path, methods) in item_paths {
                            if let Some(existing) = combined_paths.get_mut(&path) {
                                if let (Some(e_obj), Some(m_obj)) = (existing.as_object_mut(), methods.as_object()) {
                                    for (k, v) in m_obj {
                                        e_obj.insert(k.clone(), v.clone());
                                    }
                                }
                            } else {
                                combined_paths.insert(path, methods);
                            }
                        }
                    } else {
                        // Fallback: manually construct from requestPath and requestHttpMethod
                        let path = item.get("requestPath").and_then(|v| v.as_str()).unwrap_or("");
                        let name = item.get("interfaceName").and_then(|v| v.as_str()).unwrap_or("No Name");
                        let method = item.get("requestHttpMethod").and_then(|v| v.as_str()).unwrap_or("GET").to_lowercase();
                        
                        if !path.is_empty() {
                            let mut methods_obj = serde_json::Map::new();
                            methods_obj.insert(method, serde_json::json!({
                                "summary": name,
                                "description": format!("Authorized Interface: {}", name),
                                "responses": { "200": { "description": "OK" } }
                            }));

                            if let Some(existing) = combined_paths.get_mut(path) {
                                if let Some(e_obj) = existing.as_object_mut() {
                                    for (k, v) in methods_obj {
                                        e_obj.insert(k, v);
                                    }
                                }
                            } else {
                                combined_paths.insert(path.to_string(), serde_json::Value::Object(methods_obj));
                            }
                        }
                    }
                }
            }
            current_page += 1;
        }

        Ok(serde_json::json!({
            "openapi": "3.0.1",
            "info": {
                "title": "Authorized API Specification",
                "version": "1.0.0",
                "description": "DYNAMICALLY DISCOVERED FROM PLATFORM"
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
        let yaml_data = serde_yaml::to_string(spec)?;
        std::fs::write(path, yaml_data)?;
        Ok(())
    }

    fn clean_non_standard_fields(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                map.retain(|k, _| !k.starts_with("x-"));
                for (_, val) in map.iter_mut() {
                    Self::clean_non_standard_fields(val);
                }
            }
            serde_json::Value::Array(arr) => {
                arr.retain(|val| {
                    if let Some(obj) = val.as_object() {
                        let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let in_location = obj.get("in").and_then(|v| v.as_str()).unwrap_or("");
                        if name.eq_ignore_ascii_case("content-type") && in_location.eq_ignore_ascii_case("header") {
                            return false;
                        }
                    }
                    true
                });
                for val in arr.iter_mut() {
                    Self::clean_non_standard_fields(val);
                }
            }
            _ => {}
        }
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
                        continue; 
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
    use serde_json::json;
    use crate::auth::models::{Token, Ticket};
    use std::sync::Mutex;

    struct MockPool {
        ticket: Mutex<Option<Ticket>>,
        token: Mutex<Option<Token>>,
    }

    impl MockPool {
        fn new() -> Self {
            Self {
                ticket: Mutex::new(None),
                token: Mutex::new(None),
            }
        }
    }

    impl TokenPool for MockPool {
        fn get_app_ticket(&self, _profile: &str) -> Result<Ticket> {
            self.ticket.lock().unwrap().clone().ok_or_else(|| anyhow!("No ticket"))
        }
        fn set_app_ticket(&self, _profile: &str, ticket: &Ticket) -> Result<()> {
            *self.ticket.lock().unwrap() = Some(ticket.clone());
            Ok(())
        }
        fn get_access_token(&self, _profile: &str) -> Result<Token> {
            self.token.lock().unwrap().clone().ok_or_else(|| anyhow!("No token"))
        }
        fn set_access_token(&self, _profile: &str, token: &Token) -> Result<()> {
            *self.token.lock().unwrap() = Some(token.clone());
            Ok(())
        }
        fn delete_access_token(&self, _profile: &str) -> Result<()> {
            *self.token.lock().unwrap() = None;
            Ok(())
        }
        fn clear_cache(&self, _profile: &str) {}
    }

    struct MockHttpSender {
        response_body: String,
        status: u16,
    }

    #[async_trait::async_trait]
    impl HttpSender for MockHttpSender {
        async fn post(&self, _url: &str, _headers: reqwest::header::HeaderMap, _body: serde_json::Value) -> Result<SimpleResponse> {
            Ok(SimpleResponse {
                status: self.status,
                body: self.response_body.clone(),
            })
        }
        async fn get(&self, _url: &str, _headers: reqwest::header::HeaderMap) -> Result<SimpleResponse> {
            Ok(SimpleResponse {
                status: self.status,
                body: self.response_body.clone(),
            })
        }
    }

    #[tokio::test]
    async fn test_perform_network_refresh_success() {
        let pool = MockPool::new();
        pool.set_app_ticket("test", &Ticket {
            value: "valid_ticket".to_string(),
            created_at: Utc::now(),
        }).unwrap();

        let mock_http = Arc::new(MockHttpSender {
            status: 200,
            response_body: json!({
                "result": true,
                "value": {
                    "accessToken": "new_token",
                    "expiresIn": 3600
                }
            }).to_string(),
        });

        let client = AuthClient::with_sender(&pool, mock_http);
        let cfg = Config::default_with_profile("test");
        
        let token = client.perform_network_refresh("test", &cfg).await.unwrap();
        assert_eq!(token.value, "new_token");
        
        let saved_token = pool.get_access_token("test").unwrap();
        assert_eq!(saved_token.value, "new_token");
    }

    #[test]
    fn test_clean_non_standard_fields_removes_content_type_header() {
        let mut spec = json!({
            "openapi": "3.0.0",
            "paths": {
                "/test": {
                    "post": {
                        "parameters": [
                            {
                                "name": "Content-Type",
                                "in": "header",
                                "required": true,
                                "schema": {
                                    "type": "string",
                                    "default": "application/json"
                                }
                            },
                            {
                                "name": "Authorization",
                                "in": "header"
                            }
                        ]
                    }
                }
            }
        });

        AuthClient::clean_non_standard_fields(&mut spec);

        let parameters = spec["paths"]["/test"]["post"]["parameters"].as_array().unwrap();
        assert_eq!(parameters.len(), 1);
        assert_eq!(parameters[0]["name"], "Authorization");
    }
}
