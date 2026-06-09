use extism_pdk::*;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[host_fn]
extern "ExtismHost" {
    fn host_get_resolved_token(headers_json: String) -> String;
    fn host_get_app_key() -> String;
    fn host_get_app_secret() -> String;
    fn host_get_required_auth_keys(path: String, method: String) -> String;
}

#[derive(Serialize, Deserialize)]
pub struct FilterHeadersReq {
    pub path: String,
    pub method: String,
    pub has_body: bool,
    pub headers: HashMap<String, String>,
}

#[derive(Serialize, Deserialize)]
pub struct Token {
    pub value: String,
    pub created_at: String, 
}

#[plugin_fn]
pub fn filter_headers(Json(mut req): Json<FilterHeadersReq>) -> FnResult<Json<HashMap<String, String>>> {
    // 1. Ask Host what auth headers are required for this path/method according to the OpenAPI Spec
    let required_keys_json = unsafe { host_get_required_auth_keys(req.path.clone(), req.method.clone()) }.unwrap_or_default();
    
    let required_keys: Vec<String> = if !required_keys_json.is_empty() {
        serde_json::from_str(&required_keys_json).unwrap_or_else(|_| vec!["appKey".to_string(), "openToken".to_string()])
    } else {
        vec!["appKey".to_string(), "openToken".to_string()]
    };

    // 2. Inject appKey if required
    if required_keys.contains(&"appKey".to_string()) {
        let app_key = unsafe { host_get_app_key() }.unwrap_or_default();
        if !app_key.is_empty() {
            req.headers.insert("appKey".to_string(), app_key);
        }
    }

    // 3. Inject appSecret if required
    if required_keys.contains(&"appSecret".to_string()) {
        let app_secret = unsafe { host_get_app_secret() }.unwrap_or_default();
        if !app_secret.is_empty() {
            req.headers.insert("appSecret".to_string(), app_secret);
        }
    }

    // 4. Inject openToken if required
    if required_keys.contains(&"openToken".to_string()) {
        // Send the current incoming headers to the host so it can execute store_app arbitration logic (x-org-id / x-user-id)
        let headers_json = serde_json::to_string(&req.headers).unwrap_or_default();
        let token_json_str = unsafe { host_get_resolved_token(headers_json) }.unwrap_or_default();
        
        let mut open_token = "".to_string();
        if !token_json_str.is_empty() {
            if let Ok(token_val) = serde_json::from_str::<serde_json::Value>(&token_json_str) {
                if let Some(val) = token_val.get("access_token") {
                    if let Some(s) = val.as_str() {
                        open_token = s.to_string();
                    }
                } else if let Some(val) = token_val.get("value") {
                    if let Some(s) = val.as_str() {
                        open_token = s.to_string();
                    }
                }
            }
        }
        
        if !open_token.is_empty() {
            req.headers.insert("openToken".to_string(), open_token);
        }
    }

    Ok(Json(req.headers))
}
