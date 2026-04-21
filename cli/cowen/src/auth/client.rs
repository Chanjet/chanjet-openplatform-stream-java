use crate::core::config::Config;
use crate::auth::pool::TokenPool;
use crate::auth::models::Token;
use anyhow::{Result, anyhow};
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

    pub async fn json<T: for<'de> serde::Deserialize<'de>>(&self) -> Result<T> {
        serde_json::from_str(&self.body).map_err(|e| anyhow!("JSON parse error: {}", e))
    }

    pub fn text(&self) -> String {
        self.body.clone()
    }
}

#[async_trait::async_trait]
pub trait HttpSender: Send + Sync {
    async fn post(&self, url: &str, headers: reqwest::header::HeaderMap, body: serde_json::Value) -> Result<SimpleResponse>;
    async fn post_form(&self, url: &str, headers: reqwest::header::HeaderMap, body: serde_json::Value) -> Result<SimpleResponse>;
    async fn get(&self, url: &str, headers: reqwest::header::HeaderMap) -> Result<SimpleResponse>;
}

pub struct ReqwestSender {
    client: HttpClient,
}

#[cfg(feature = "inte")]
pub struct MockHttpSender {
    pub real_sender: ReqwestSender,
}

#[cfg(feature = "inte")]
#[async_trait::async_trait]
impl HttpSender for MockHttpSender {
    async fn post(&self, url: &str, headers: reqwest::header::HeaderMap, body: serde_json::Value) -> Result<SimpleResponse> {
        if std::env::var("OWENC_ENV").unwrap_or_default() == "inte" {
            if url.contains("/oauth2/token") {
                return Ok(SimpleResponse {
                    status: 200,
                    body: serde_json::json!({
                        "access_token": "mock_access_token",
                        "refresh_token": "mock_refresh_token",
                        "expires_in": 7200,
                        "refresh_token_expires_in": 604800
                    }).to_string(),
                });
            }
        }
        self.real_sender.post(url, headers, body).await
    }

    async fn post_form(&self, url: &str, headers: reqwest::header::HeaderMap, body: serde_json::Value) -> Result<SimpleResponse> {
        if std::env::var("OWENC_ENV").unwrap_or_default() == "inte" {
            if url.contains("/oauth2/token") || url.contains("/user/v2/token") {
                return Ok(SimpleResponse {
                    status: 200,
                    body: serde_json::json!({
                        "access_token": "mock_access_token",
                        "refresh_token": "mock_refresh_token",
                        "expires_in": 7200,
                        "refresh_token_expires_in": 604800
                    }).to_string(),
                });
            }
        }
        self.real_sender.post_form(url, headers, body).await
    }

    async fn get(&self, url: &str, headers: reqwest::header::HeaderMap) -> Result<SimpleResponse> {
        if std::env::var("OWENC_ENV").unwrap_or_default() == "inte" {
            if url.contains("/v1/common/openapi/spec") {
                return Ok(SimpleResponse {
                    status: 200,
                    body: serde_json::json!({
                        "openapi": "3.0.0",
                        "paths": { "/mock-api": { "get": { "responses": { "200": { "description": "OK" } } } } }
                    }).to_string(),
                });
            }
        }
        self.real_sender.get(url, headers).await
    }
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

    async fn post_form(&self, url: &str, headers: reqwest::header::HeaderMap, body: serde_json::Value) -> Result<SimpleResponse> {
        let resp = self.client.post(url)
            .headers(headers)
            .form(&body)
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
    async fn trigger_push(&self, profile: &str, cfg: &Config, force: bool) -> Result<()>;
    async fn get_openapi_spec(&self, profile: &str, cfg: &Config, force_refresh: bool) -> Result<serde_json::Value>;
    async fn get_dynamic_interface_list(&self, profile: &str, cfg: &Config) -> Result<serde_json::Value>;
    async fn clear_token(&self, profile: &str) -> Result<()>;
}

use crate::auth::provider::self_built::SelfBuiltProvider;
use crate::auth::provider::AuthProvider;
use crate::auth::models::AuthMode;

use crate::auth::provider::oauth2::OAuth2Provider;

pub struct AuthClient<'a> {
    pool: &'a (dyn TokenPool + Send + Sync),
    http_sender: Arc<dyn HttpSender>,
    self_built: SelfBuiltProvider<'a>,
    oauth2: OAuth2Provider<'a>,
}

impl<'a> AuthClient<'a> {
    pub fn new(pool: &'a (dyn TokenPool + Send + Sync)) -> Self {
        #[cfg(not(feature = "inte"))]
        let http_sender: Arc<dyn HttpSender> = Arc::new(ReqwestSender::new());

        #[cfg(feature = "inte")]
        let http_sender: Arc<dyn HttpSender> = if std::env::var("OWENC_ENV").unwrap_or_default() == "inte" {
            Arc::new(MockHttpSender { real_sender: ReqwestSender::new() })
        } else {
            Arc::new(ReqwestSender::new())
        };

        Self {
            pool,
            http_sender: http_sender.clone(),
            self_built: SelfBuiltProvider::with_sender(pool, http_sender.clone()),
            oauth2: OAuth2Provider::new(pool, http_sender),
        }
    }

    #[cfg(test)]
    pub fn with_sender(pool: &'a (dyn TokenPool + Send + Sync), sender: Arc<dyn HttpSender>) -> Self {
        Self {
            pool,
            http_sender: sender.clone(),
            self_built: SelfBuiltProvider::with_sender(pool, sender.clone()),
            oauth2: OAuth2Provider::new(pool, sender),
        }
    }

    fn provider(&self, mode: &AuthMode) -> &dyn AuthProvider {
        match mode {
            AuthMode::SelfBuilt => &self.self_built,
            AuthMode::Oauth2 => &self.oauth2,
        }
    }
}

#[async_trait::async_trait]
impl<'a> Client for AuthClient<'a> {
    async fn get_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token> {
        self.provider(&cfg.app_mode).get_token(profile, cfg).await
    }

    async fn refresh_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token> {
        self.provider(&cfg.app_mode).refresh(profile, cfg).await
    }

    async fn clear_token(&self, profile: &str) -> Result<()> {
        // 1. Clear Access Tokens (Generic pool)
        self.pool.delete_access_token(profile)?;
        
        // 2. Clear AppTickets (Self-Built specific)
        let vault = self.pool.as_vault();
        let _ = vault.delete(profile, "app_ticket");
        let _ = vault.delete(profile, "app_ticket_created");
        
        // 3. Clear OAuth2 Session & Tokens (OAuth2 specific)
        let _ = vault.delete(profile, "oauth2_token_pair");
        let _ = vault.delete(profile, "pending_auth_session");
        let _ = vault.delete(profile, "captured_auth_code");
        
        // 4. Clear Auth related transient states
        let _ = vault.delete(profile, "push_backoff_level");
        let _ = vault.delete(profile, "push_last_attempt_ts");
        
        Ok(())
    }

    async fn trigger_push(&self, profile: &str, cfg: &Config, force: bool) -> Result<()> {
        self.self_built.trigger_push(profile, cfg, force).await
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
            let mut spec_url = format!("{}{}", cfg.openapi_url.trim_end_matches('/'), obfs!("/v1/common/openapi/spec"));
            if cfg.app_mode == AuthMode::Oauth2 {
                spec_url.push_str("?checkPermission=false");
            }
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
        let base_url = format!("{}{}", cfg.openapi_url.trim_end_matches('/'), obfs!("/developer/api/apiPermissions/isv/open/getInterfaceList"));
        
        let mut current_page = 1;
        let mut total_pages = 1;
        let mut combined_paths = serde_json::Map::new();

        while current_page <= total_pages {
            let url = if cfg.app_mode == AuthMode::Oauth2 {
                format!("{}?page={}&size=100&checkPermission=false", base_url, current_page-1)
            } else {
                format!("{}?page={}&size=100", base_url, current_page-1)
            };
            tracing::info!(target: "sys", "Fetching interface list page {}/{}", current_page, total_pages);
            
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("openToken", token.value.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
            headers.insert("appKey", cfg.app_key.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));

            let resp = self.http_sender.get(&url, headers).await?;

            if !resp.is_success() {
                return Err(anyhow!("Failed to fetch interface list page {}: HTTP {}", current_page, resp.status));
            }

            let body: serde_json::Value = resp.json().await?;
            let value = body.get("value").ok_or_else(|| anyhow!("Invalid response structure"))?;
            
            tracing::debug!(target: "sys", "Server reported currentPage: {:?}, totalPages: {:?}", value.get("currentPage"), value.get("totalPages"));

            // Update pagination info
            total_pages = value.get("totalPages").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            
            if let Some(list) = value.get("resultList").and_then(|l| l.as_array()) {
                tracing::debug!(target: "sys", "Page {} resultList contains {} items", current_page, list.len());
                if !list.is_empty() {
                    let first_item = &list[0];
                    tracing::debug!(target: "sys", "Page {} first item: path={:?}, name={:?}", current_page, first_item.get("requestPath"), first_item.get("interfaceName"));
                }
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
                                // tracing::debug!(target: "sys", "Extracted {} paths from openApi object", item_paths.len());
                            } else {
                                // tracing::debug!(target: "sys", "openApi field exists but no paths found for item");
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
                            methods_obj.insert(method.clone(), serde_json::json!({
                                "summary": name,
                                "description": format!("Authorized Interface: {}", name),
                                "responses": { "200": { "description": "OK" } }
                            }));

                            if let Some(existing) = combined_paths.get_mut(path) {
                                if let Some(e_obj) = existing.as_object_mut() {
                                    e_obj.insert(method.clone(), methods_obj.get(&method).unwrap().clone());
                                }
                            } else {
                                combined_paths.insert(path.to_string(), serde_json::Value::Object(methods_obj));
                            }
                        } else {
                            tracing::debug!(target: "sys", "Fallback failed: requestPath is empty for item {:?}", item.get("id"));
                        }
                    }
                }
            } else {
                tracing::debug!(target: "sys", "Page {} resultList is empty or invalid", current_page);
            }
            
            // Calculate total methods merged so far
            let mut current_total_methods = 0;
            for methods in combined_paths.values() {
                if let Some(m) = methods.as_object() {
                    current_total_methods += m.len();
                }
            }
            tracing::debug!(target: "sys", "After page {}, combined_paths has {} unique paths and {} total methods", current_page, combined_paths.len(), current_total_methods);

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
    use crate::core::vault::Vault;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use chrono::Utc;

    struct MockPool {
        ticket: Mutex<Option<Ticket>>,
        token: Mutex<Option<Token>>,
        vault: Arc<dyn Vault>,
    }

    struct MockVault {
        data: Mutex<HashMap<String, String>>,
    }

    impl Vault for MockVault {
        fn get(&self, _profile: &str, key: &str) -> Result<String> {
            self.data.lock().unwrap().get(key).cloned().ok_or_else(|| anyhow!("Not found"))
        }
        fn set(&self, _profile: &str, key: &str, value: &str) -> Result<()> {
            self.data.lock().unwrap().insert(key.to_string(), value.to_string());
            Ok(())
        }
        fn delete(&self, _profile: &str, key: &str) -> Result<()> {
            self.data.lock().unwrap().remove(key);
            Ok(())
        }
        fn clear_profile(&self, _profile: &str) -> Result<()> {
            self.data.lock().unwrap().clear();
            Ok(())
        }
        fn rename_profile(&self, _old: &str, _new: &str) -> Result<()> {
            Ok(())
        }
    }

    impl MockPool {
        fn new() -> Self {
            Self {
                ticket: Mutex::new(None),
                token: Mutex::new(None),
                vault: Arc::new(MockVault { data: Mutex::new(HashMap::new()) }),
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
        fn delete_access_token(&self, profile: &str) -> Result<()> {
            *self.token.lock().unwrap() = None;
            let _ = self.vault.delete(profile, "access_token");
            let _ = self.vault.delete(profile, "access_token_expires");
            let _ = self.vault.delete(profile, "access_token_created");
            Ok(())
        }
        fn clear_cache(&self, _profile: &str) {}
        fn as_vault(&self) -> Arc<dyn Vault> {
            self.vault.clone()
        }
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
        async fn post_form(&self, _url: &str, _headers: reqwest::header::HeaderMap, _body: serde_json::Value) -> Result<SimpleResponse> {
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
        let mut cfg = Config::default_with_profile("test");
        cfg.app_key = "test_app_key".to_string();
        cfg.app_secret = "test_app_secret".to_string();
        cfg.app_mode = AuthMode::SelfBuilt;
        
        let token = client.get_app_access_token("test", &cfg).await.unwrap();
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

    #[tokio::test]
    async fn test_clear_token_removes_all_dynamic_keys() {
        let pool = MockPool::new();
        let vault = pool.as_vault();
        let profile = "test";
        
        vault.set(profile, "access_token", "abc").unwrap();
        vault.set(profile, "app_ticket", "tkt").unwrap();
        vault.set(profile, "oauth2_token_pair", "{}").unwrap();
        vault.set(profile, "push_backoff_level", "1").unwrap();
        vault.set(profile, "app_secret", "STAY").unwrap();
        
        let client = AuthClient::new(&pool);
        client.clear_token(profile).await.unwrap();
        
        assert!(vault.get(profile, "access_token").is_err());
        assert!(vault.get(profile, "app_ticket").is_err());
        assert!(vault.get(profile, "oauth2_token_pair").is_err());
        assert!(vault.get(profile, "push_backoff_level").is_err());
        assert_eq!(vault.get(profile, "app_secret").unwrap(), "STAY");
    }
}
