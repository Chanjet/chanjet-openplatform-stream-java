use cowen_auth::client::Client;
use cowen_common::vault::Vault;
use cowen_common::CowenError;
use cowen_config::ConfigManager;
use std::sync::Arc;
use tracing::info;

use cowen_macros::{rbac, rbac_controller};

// Domain DTOs
pub struct DomainApiSpecRequest {
    pub profile: String,
    pub method: String,
    pub path: String,
}

pub struct DomainApiSpecResponse {
    pub json: String,
    pub error_message: Option<String>,
}

pub struct DomainCallApiRequest {
    pub profile: String,
    pub method: String,
    pub path: String,
    pub data: Option<String>,
    pub force: bool,
}

pub struct DomainCallApiResponse {
    pub status: u32,
    pub headers: std::collections::HashMap<String, String>,
    pub body: String,
    pub error_message: Option<String>,
}

pub struct DomainApiListRequest {
    pub profile: String,
    pub search: Option<String>,
    pub page: u32,
    pub page_size: u32,
    pub refresh: bool,
}

pub struct DomainApiListResponse {
    pub total: u32,
    pub json: String,
    pub plugin_used: Option<String>,
    pub error_message: Option<String>,
}

#[tonic::async_trait]
pub trait NativeApiRegistryCapability: Send + Sync {
    async fn api_spec(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: DomainApiSpecRequest,
    ) -> Result<DomainApiSpecResponse, CowenError>;
    async fn call_api(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: DomainCallApiRequest,
    ) -> Result<DomainCallApiResponse, CowenError>;
    async fn api_list(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: DomainApiListRequest,
    ) -> Result<DomainApiListResponse, CowenError>;
}

pub struct DefaultApiRegistry {
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
    native_search: Arc<dyn crate::internal::native_search::NativeSearchCapability>,
}

impl DefaultApiRegistry {
    pub fn new(
        vault: Arc<dyn Vault>,
        cfg_mgr: ConfigManager,
        native_search: Arc<dyn crate::internal::native_search::NativeSearchCapability>,
    ) -> Self {
        Self {
            vault,
            cfg_mgr,
            native_search,
        }
    }

    async fn load_config(&self, profile: &str) -> Result<cowen_common::config::Config, CowenError> {
        self.cfg_mgr
            .load(profile)
            .await
            .map_err(|e| CowenError::NotFound(e.to_string()))
    }

    async fn get_openapi_spec(
        &self,
        profile: &str,
        config: &cowen_common::config::Config,
        refresh: bool,
    ) -> Result<serde_json::Value, CowenError> {
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        auth_cli
            .get_openapi_spec(profile, config, refresh)
            .await
            .map_err(|e| CowenError::Internal(e.to_string()))
    }

    async fn validate_api_call(
        &self,
        profile: &str,
        config: &cowen_common::config::Config,
        req: &DomainCallApiRequest,
        method_upper: &str,
        body_option: &Option<String>,
    ) -> Result<(), CowenError> {
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        if !auth_cli.supports_api_call(config) {
            return Err(CowenError::Validation(format!(
                "Auth mode {:?} does not support direct CLI API calls.",
                config.app_mode
            )));
        }

        if !req.force {
            let spec = self.get_openapi_spec(profile, config, false).await?;
            if let Err(e) =
                cowen_common::openapi::validate_request(&spec, method_upper, &req.path, body_option)
            {
                return Err(CowenError::Validation(format!(
                    "OpenAPI validation failed: {}",
                    e
                )));
            }
            let path_no_query = req.path.split('?').next().unwrap_or(&req.path);
            if !cowen_auth::client::is_path_in_whitelist(path_no_query, &spec) {
                return Err(CowenError::Auth(format!(
                    "CLI Rejected: Target path {} is not in the OpenAPI whitelist.",
                    path_no_query
                )));
            }
        }
        Ok(())
    }

    async fn execute_http_api_call(
        &self,
        profile: &str,
        config: &cowen_common::config::Config,
        app_cfg: &cowen_common::config::AppConfig,
        req: &DomainCallApiRequest,
        method_upper: &str,
        body_option: &Option<String>,
    ) -> Result<DomainCallApiResponse, CowenError> {
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let token = match auth_cli
            .get_token(profile, config, &reqwest::header::HeaderMap::new())
            .await
        {
            Ok(t) => t,
            Err(e) => return Err(CowenError::Internal(format!("Failed to get token: {}", e))),
        };

        let ua = cowen_infra::get_user_agent("0.4.0");
        let client = match cowen_infra::create_client(&ua) {
            Ok(c) => c,
            Err(e) => return Err(CowenError::Internal(e.to_string())),
        };
        let url = if req.path.starts_with("http") {
            req.path.clone()
        } else {
            let base = app_cfg.openapi_url.trim_end_matches('/');
            format!("{}{}", base, req.path)
        };

        let method_enum = match reqwest::Method::from_bytes(method_upper.as_bytes()) {
            Ok(m) => m,
            Err(_) => {
                return Err(CowenError::Validation(format!(
                    "Invalid HTTP method: {}",
                    method_upper
                )))
            }
        };

        let mut api_req = client
            .request(method_enum, &url)
            .header("openToken", token.value)
            .header("appKey", config.app_key.trim());

        if let Some(b) = body_option {
            let json_body: serde_json::Value = match serde_json::from_str(b) {
                Ok(j) => j,
                Err(e) => {
                    return Err(CowenError::Validation(format!(
                        "Invalid JSON payload: {}",
                        e
                    )))
                }
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
                Ok(DomainCallApiResponse {
                    status: status as u32,
                    headers: headers_map,
                    body,
                    error_message: None,
                })
            }
            Err(e) => Ok(DomainCallApiResponse {
                status: 520,
                headers: std::collections::HashMap::new(),
                body: "".to_string(),
                error_message: Some(format!("Request failed: {}", e)),
            }),
        }
    }
}

#[rbac_controller(domain = "native.api.registry")]
#[tonic::async_trait]
impl NativeApiRegistryCapability for DefaultApiRegistry {
    #[rbac(action = "read")]
    async fn api_spec(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: DomainApiSpecRequest,
    ) -> Result<DomainApiSpecResponse, CowenError> {
        let config = self.load_config(&req.profile).await?;
        let spec = self.get_openapi_spec(&req.profile, &config, false).await?;
        let method_lower = req.method.to_lowercase();
        if let Some(op) = spec
            .get("paths")
            .and_then(|p| p.as_object())
            .and_then(|p| p.get(&req.path))
            .and_then(|p| p.get(&method_lower))
        {
            let mut result = serde_json::Map::new();
            result.insert("operation".to_string(), op.clone());
            if let Some(components) = spec.get("components") {
                result.insert("components".to_string(), components.clone());
            }
            let json = serde_json::to_string(&result).unwrap_or_default();
            Ok(DomainApiSpecResponse {
                json,
                error_message: None,
            })
        } else {
            Ok(DomainApiSpecResponse {
                json: "".to_string(),
                error_message: Some("Operation not found".to_string()),
            })
        }
    }

    #[rbac(action = "execute")]
    async fn call_api(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: DomainCallApiRequest,
    ) -> Result<DomainCallApiResponse, CowenError> {
        let profile = req.profile.clone();
        info!(
            "CallApi requested for profile={} method={} path={}",
            profile, req.method, req.path
        );

        let config = self.load_config(&profile).await?;

        let app_cfg = match self.cfg_mgr.load_app_config().await {
            Ok(c) => c,
            Err(e) => return Err(CowenError::Internal(e.to_string())),
        };

        let body_option = if req.data.is_none()
            || req.data.as_ref().unwrap().trim() == "{}"
            || req.data.as_ref().unwrap().trim().is_empty()
        {
            None
        } else {
            req.data.clone()
        };

        let method_upper = req.method.to_uppercase();

        self.validate_api_call(&profile, &config, &req, &method_upper, &body_option)
            .await?;

        if req.path.starts_with("http") && !req.path.starts_with(&app_cfg.openapi_url) {
            return Err(CowenError::Auth(
                "CLI Security Block: Absolute external URLs are not allowed.".to_string(),
            ));
        }

        self.execute_http_api_call(
            &profile,
            &config,
            &app_cfg,
            &req,
            &method_upper,
            &body_option,
        )
        .await
    }

    #[rbac(action = "search")]
    async fn api_list(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: DomainApiListRequest,
    ) -> Result<DomainApiListResponse, CowenError> {
        let profile = req.profile.clone();

        let config = self.load_config(&profile).await?;
        let spec = self
            .get_openapi_spec(&req.profile, &config, req.refresh)
            .await?;

        let mut ops = crate::internal::openapi_parser::OpenApiParser::parse_operations(&spec);

        let (filtered_ops, used_plugin_name) = self
            .native_search
            .search_if_needed(&req.profile, ops, &req.search)
            .await;

        ops = filtered_ops;

        let total = ops.len() as u32;

        let page = req.page.max(1) as usize;
        let page_size = req.page_size.max(1) as usize;
        let start = (page - 1) * page_size;
        let end = (start + page_size).min(ops.len());

        let paged_ops = if start < ops.len() {
            ops[start..end].to_vec()
        } else {
            Vec::new()
        };

        let json = serde_json::to_string(&paged_ops).unwrap_or_default();
        Ok(DomainApiListResponse {
            total,
            json,
            plugin_used: used_plugin_name,
            error_message: None,
        })
    }
}
