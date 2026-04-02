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
        if let Ok(token) = self.pool.get_access_token(profile) {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        let _lock_guard = self.pool.lock(profile).context("Failed to acquire global refresh lock")?;
        self.pool.clear_cache(profile);

        if let Ok(token) = self.pool.get_access_token(profile) {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        self.perform_network_refresh(profile, cfg).await
    }

    async fn refresh_app_access_token(&self, profile: &str, cfg: &Config) -> Result<Token> {
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
        // CHANGED: Use .yaml instead of .json
        let cache_path = app_dir.join(format!("{}_openapi.yaml", profile));

        // 1. Load Cache if exists
        let cached_data = if cache_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&cache_path) {
                serde_yaml::from_str::<serde_json::Value>(&data).ok()
            } else { None }
        } else { None };

        // 2. Evaluation Logic
        if let Some(spec) = cached_data {
            let metadata = std::fs::metadata(&cache_path)?;
            let elapsed = metadata.modified()?.elapsed()?.as_secs();

            if elapsed < 3600 {
                return Ok(spec);
            }

            if !cfg.openapi_url.is_empty() {
                let spec_url = format!("{}/v1/common/openapi/spec", cfg.openapi_url.trim_end_matches('/'));
                if let Ok(token) = self.get_app_access_token(profile, cfg).await {
                    match self.http.get(&spec_url)
                        .header("openToken", token.value)
                        .header("appKey", &cfg.app_key)
                        .send()
                        .await {
                        Ok(resp) if resp.status().is_success() => {
                            if let Ok(mut fresh_spec) = resp.json::<serde_json::Value>().await {
                                Self::clean_non_standard_fields(&mut fresh_spec);
                                let _ = self.save_spec_to_cache(&cache_path, &fresh_spec);
                                return Ok(fresh_spec);
                            }
                        }
                        _ => {}
                    }
                }
            }
            
            if let Ok(dynamic_spec) = self.get_dynamic_interface_list(profile, cfg).await {
                let _ = self.save_spec_to_cache(&cache_path, &dynamic_spec);
                return Ok(dynamic_spec);
            }

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
                        if let Ok(mut spec) = resp.json::<serde_json::Value>().await {
                            Self::clean_non_standard_fields(&mut spec);
                            let _ = self.save_spec_to_cache(&cache_path, &spec);
                            return Ok(spec);
                        }
                    }
                }
            }
        }

        // 4. Pull Spec Failed: Try Dynamic Interface List
        tracing::info!(target: "sys", "Full OpenAPI spec unavailable. Fetching real authorized interface list...");
        match self.get_dynamic_interface_list(profile, cfg).await {
            Ok(dynamic_spec) => {
                let _ = self.save_spec_to_cache(&cache_path, &dynamic_spec);
                return Ok(dynamic_spec);
            }
            Err(e) => {
                tracing::error!(target: "sys", "Live discovery failed: {}", e);
                Err(anyhow!("Could not fetch real API list. Fallback unavailable."))
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
            
            let resp = self.http.get(&url)
                .header("openToken", &token.value)
                .header("appKey", &cfg.app_key)
                .send()
                .await?;

            if !resp.status().is_success() {
                return Err(anyhow!("Failed to fetch interface list page {}: HTTP {}", current_page, resp.status()));
            }

            let body: serde_json::Value = resp.json().await?;
            let value = body.get("value").ok_or_else(|| anyhow!("Invalid response structure"))?;
            
            // Update pagination info
            total_pages = value.get("totalPages").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            
            if let Some(list) = value.get("resultList").and_then(|l| l.as_array()) {
                for item in list {
                    if let Some(mut item_spec) = item.get("openApi").cloned() {
                        // CLEAN EXTENSIONS from snippet
                        Self::clean_non_standard_fields(&mut item_spec);
                        
                        if let Some(item_paths) = item_spec.get("paths").and_then(|p| p.as_object()) {
                            for (path, methods) in item_paths {
                                combined_paths.insert(path.clone(), methods.clone());
                            }
                        }
                    } else {
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
                            combined_paths.insert(path.to_string(), serde_json::Value::Object(methods_obj));
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
        // CHANGED: Use YAML for persistence
        let yaml_data = serde_yaml::to_string(spec)?;
        std::fs::write(path, yaml_data)?;
        Ok(())
    }

    /// Recursively removes all keys starting with 'x-' (OpenAPI extensions)
    fn clean_non_standard_fields(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                // 1. Remove x- fields from this level
                map.retain(|k, _| !k.starts_with("x-"));
                // 2. Recursively clean children
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
