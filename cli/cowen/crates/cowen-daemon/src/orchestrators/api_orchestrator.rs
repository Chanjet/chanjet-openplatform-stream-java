use std::sync::Arc;
use cowen_config::ConfigManager;
use cowen_common::vault::Vault;
use tonic::{Response, Status};
use cowen_common::grpc::proto::{ApiSpecRequest, ApiSpecResponse, CallApiRequest, CallApiResponse};
use tracing::info;
use cowen_auth::client::Client;

pub struct ApiOrchestrator {
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
}

impl ApiOrchestrator {
    pub fn new(vault: Arc<dyn Vault>, cfg_mgr: ConfigManager) -> Self {
        Self { vault, cfg_mgr }
    }

    pub async fn api_spec(&self, req: ApiSpecRequest) -> Result<Response<ApiSpecResponse>, Status> {
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(Status::not_found(e.to_string()))
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        match auth_cli.get_openapi_spec(&req.profile, &config, false).await {
            Ok(spec) => {
                let method_lower = req.method.to_lowercase();
                if let Some(op) = spec.get("paths").and_then(|p| p.as_object()).and_then(|p| p.get(&req.path)).and_then(|p| p.get(&method_lower)) {
                    let mut result = serde_json::Map::new();
                    result.insert("operation".to_string(), op.clone());
                    if let Some(components) = spec.get("components") {
                        result.insert("components".to_string(), components.clone());
                    }
                    let json = serde_json::to_string(&result).unwrap_or_default();
                    Ok(Response::new(ApiSpecResponse { json, error_message: None }))
                } else {
                    Ok(Response::new(ApiSpecResponse { json: "".to_string(), error_message: Some("Operation not found".to_string()) }))
                }
            }
            Err(e) => Ok(Response::new(ApiSpecResponse { json: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }

    pub async fn call_api(&self, req: CallApiRequest) -> Result<Response<CallApiResponse>, Status> {
        info!("CallApi requested for profile={} method={} path={}", req.profile, req.method, req.path);
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(Status::not_found(format!("Profile not found: {}", e)))
        };
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        if !auth_cli.supports_api_call(&config) {
            return Err(Status::invalid_argument(format!("Auth mode {:?} does not support direct CLI API calls.", config.app_mode)));
        }

        let app_cfg = match self.cfg_mgr.load_app_config().await {
            Ok(c) => c,
            Err(e) => return Err(Status::internal(e.to_string()))
        };

        let body_option = if req.data.is_none() || req.data.as_ref().unwrap().trim() == "{}" || req.data.as_ref().unwrap().trim().is_empty() {
            None
        } else {
            req.data.clone()
        };

        let method_upper = req.method.to_uppercase();

        if !req.force {
            let spec = match auth_cli.get_openapi_spec(&req.profile, &config, false).await {
                Ok(s) => s,
                Err(e) => return Err(Status::internal(e.to_string()))
            };
            if let Err(e) = cowen_common::openapi::validate_request(&spec, &method_upper, &req.path, &body_option) {
                return Err(Status::invalid_argument(format!("OpenAPI validation failed: {}", e)));
            }
            let path_no_query = req.path.split('?').next().unwrap_or(&req.path);
            if !cowen_auth::client::is_path_in_whitelist(path_no_query, &spec) {
                return Err(Status::permission_denied(format!("CLI Rejected: Target path {} is not in the OpenAPI whitelist.", path_no_query)));
            }
        }

        if req.path.starts_with("http") && !req.path.starts_with(&app_cfg.openapi_url) {
            return Err(Status::permission_denied("CLI Security Block: Absolute external URLs are not allowed.".to_string()));
        }

        let token = match auth_cli.get_token(&req.profile, &config, &reqwest::header::HeaderMap::new()).await {
            Ok(t) => t,
            Err(e) => return Err(Status::internal(format!("Failed to get token: {}", e)))
        };

        let ua = cowen_infra::get_user_agent("0.4.0");
        let client = match cowen_infra::create_client(&ua) {
            Ok(c) => c,
            Err(e) => return Err(Status::internal(e.to_string()))
        };
        let url = if req.path.starts_with("http") {
            req.path.clone()
        } else {
            let base = app_cfg.openapi_url.trim_end_matches('/');
            format!("{}{}", base, req.path)
        };

        let method_enum = match reqwest::Method::from_bytes(method_upper.as_bytes()) {
            Ok(m) => m,
            Err(_) => return Err(Status::invalid_argument(format!("Invalid HTTP method: {}", method_upper)))
        };

        let mut api_req = client.request(method_enum, &url)
            .header("openToken", token.value)
            .header("appKey", config.app_key.trim());

        if let Some(b) = body_option {
            let json_body: serde_json::Value = match serde_json::from_str(&b) {
                Ok(j) => j,
                Err(e) => return Err(Status::invalid_argument(format!("Invalid JSON payload: {}", e)))
            };
            api_req = api_req.json(&json_body);
        }

        match api_req.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let mut headers_map = std::collections::HashMap::new();
                for (k, v) in resp.headers().iter() {
                    if let Ok(v_str) = v.to_str() {
                        headers_map.insert(k.to_string(), v_str.to_string());
                    }
                }
                let body = resp.text().await.unwrap_or_default();
                Ok(Response::new(CallApiResponse { status: status as u32, headers: headers_map, body, error_message: None }))
            }
            Err(e) => Ok(Response::new(CallApiResponse { status: 520, headers: std::collections::HashMap::new(), body: "".to_string(), error_message: Some(format!("Request failed: {}", e)) }))
        }
    }
}
