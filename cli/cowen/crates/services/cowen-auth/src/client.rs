use crate::pool::TokenPool;
use cowen_common::config::Config;
use cowen_common::{CowenError, CowenResult};
use cowen_infra::obfs;

use async_trait::async_trait;
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

    pub async fn json<T: serde::de::DeserializeOwned>(self) -> CowenResult<T> {
        serde_json::from_str(&self.body)
            .map_err(|e| CowenError::Serialization(format!("Failed to parse response JSON: {}", e)))
    }

    pub fn text(self) -> String {
        self.body
    }
}

#[async_trait]
pub trait HttpSender: Send + Sync {
    async fn post(
        &self,
        url: &str,
        headers: reqwest::header::HeaderMap,
        body: serde_json::Value,
    ) -> CowenResult<SimpleResponse>;
    async fn post_form(
        &self,
        url: &str,
        headers: reqwest::header::HeaderMap,
        body: serde_json::Value,
    ) -> CowenResult<SimpleResponse>;
    async fn get(
        &self,
        url: &str,
        headers: reqwest::header::HeaderMap,
    ) -> CowenResult<SimpleResponse>;
}

pub struct ReqwestSender {
    client: HttpClient,
}

impl Default for ReqwestSender {
    fn default() -> Self {
        Self::new()
    }
}

impl ReqwestSender {
    pub fn new() -> Self {
        Self {
            client: HttpClient::new(),
        }
    }
}

#[cfg(feature = "inte")]
pub struct MockHttpSender {
    pub real_sender: ReqwestSender,
}

#[cfg(feature = "inte")]
#[async_trait]
impl HttpSender for MockHttpSender {
    async fn post(
        &self,
        url: &str,
        headers: reqwest::header::HeaderMap,
        body: serde_json::Value,
    ) -> CowenResult<SimpleResponse> {
        // 🚀 OCP: Integration tests can intercept or log platform calls here
        self.real_sender.post(url, headers, body).await
    }

    async fn post_form(
        &self,
        url: &str,
        headers: reqwest::header::HeaderMap,
        body: serde_json::Value,
    ) -> CowenResult<SimpleResponse> {
        self.real_sender.post_form(url, headers, body).await
    }

    async fn get(
        &self,
        url: &str,
        headers: reqwest::header::HeaderMap,
    ) -> CowenResult<SimpleResponse> {
        self.real_sender.get(url, headers).await
    }
}

#[async_trait]
impl HttpSender for ReqwestSender {
    async fn post(
        &self,
        url: &str,
        headers: reqwest::header::HeaderMap,
        body: serde_json::Value,
    ) -> CowenResult<SimpleResponse> {
        let resp = self
            .client
            .post(url)
            .headers(headers)
            .json(&body)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        Ok(SimpleResponse { status, body })
    }

    async fn post_form(
        &self,
        url: &str,
        headers: reqwest::header::HeaderMap,
        body: serde_json::Value,
    ) -> CowenResult<SimpleResponse> {
        let resp = self
            .client
            .post(url)
            .headers(headers)
            .form(&body)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        Ok(SimpleResponse { status, body })
    }

    async fn get(
        &self,
        url: &str,
        headers: reqwest::header::HeaderMap,
    ) -> CowenResult<SimpleResponse> {
        let resp = self.client.get(url).headers(headers).send().await?;
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        Ok(SimpleResponse { status, body })
    }
}

#[async_trait]
pub trait Client: Send + Sync {
    async fn get_token(
        &self,
        profile: &str,
        cfg: &Config,
        headers: &reqwest::header::HeaderMap,
    ) -> CowenResult<cowen_common::models::Token>;
    async fn refresh_token(
        &self,
        profile: &str,
        cfg: &Config,
        headers: &reqwest::header::HeaderMap,
    ) -> CowenResult<cowen_common::models::Token>;
    async fn trigger_push(&self, profile: &str, cfg: &Config, force: bool) -> CowenResult<()>;
    async fn get_openapi_spec(
        &self,
        profile: &str,
        cfg: &Config,
        force_refresh: bool,
    ) -> CowenResult<serde_json::Value>;
    async fn get_dynamic_interface_list(
        &self,
        profile: &str,
        cfg: &Config,
    ) -> CowenResult<serde_json::Value>;
    async fn clear_token(&self, profile: &str, cfg: &Config) -> CowenResult<()>;

    // 🚀 Store App (OAuth2) Extension Methods
    async fn get_app_access_token(
        &self,
        profile: &str,
        cfg: &Config,
    ) -> CowenResult<cowen_common::models::Token>;
    async fn refresh_app_access_token(
        &self,
        profile: &str,
        cfg: &Config,
    ) -> CowenResult<cowen_common::models::Token>;
    async fn exchange_temp_code(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
        temp_code: &str,
    ) -> CowenResult<cowen_common::models::Token>;
    async fn get_user_access_token(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
        user_id: &str,
    ) -> CowenResult<cowen_common::models::Token>;
    async fn intercept_exchange(
        &self,
        profile: &str,
        cfg: &Config,
        body_bytes: &[u8],
    ) -> CowenResult<serde_json::Value>;

    // 🚀 OCP Lifecycle Hooks
    async fn on_maintenance_tick(&self, profile: &str, cfg: &Config) -> CowenResult<()>;
    async fn requires_initial_push(&self, cfg: &Config) -> bool;
    async fn handle_platform_event(
        &self,
        profile: &str,
        cfg: &Config,
        event: crate::provider::PlatformEvent,
    ) -> CowenResult<()>;
    fn requires_ticket(&self, cfg: &Config) -> bool;
    fn supports_webhooks(&self, cfg: &Config) -> bool;
    fn supports_api_call(&self, cfg: &Config) -> bool;
    async fn perform_login(
        &self,
        profile: &str,
        cfg: &Config,
        force: bool,
        finalize: Option<&str>,
        daemon_service: Option<std::sync::Arc<dyn cowen_common::daemon::DaemonService>>,
    ) -> CowenResult<()>;

    /// 🚀 UI/诊断能力：获取该模式下的专属诊断条目（Auth、Daemon等）
    async fn get_diagnostics(
        &self,
        ctx: &cowen_common::status::StatusContext<'_>,
    ) -> CowenResult<Vec<cowen_common::status::StatusEntry>>;
    fn get_provider(
        &self,
        mode: &cowen_common::models::AuthMode,
    ) -> Option<Arc<dyn crate::provider::AuthProvider>>;
}

use crate::provider::AuthProvider;
use cowen_common::models::AuthMode;
use std::collections::HashMap;

#[derive(Clone)]
pub struct AuthClient {
    pool: Arc<dyn TokenPool + Send + Sync>,
    http_sender: Arc<dyn HttpSender>,
    providers: HashMap<AuthMode, Arc<dyn AuthProvider>>,
}

/// Builder for constructing AuthClient with registered providers.
/// Keeps `client.rs` free from concrete provider type knowledge.
pub struct AuthClientBuilder {
    pub(crate) pool: Arc<dyn TokenPool + Send + Sync>,
    pub(crate) http_sender: Arc<dyn HttpSender>,
    providers: HashMap<AuthMode, Arc<dyn AuthProvider>>,
}

impl AuthClientBuilder {
    pub fn register(mut self, mode: AuthMode, provider: Arc<dyn AuthProvider>) -> Self {
        self.providers.insert(mode, provider);
        self
    }

    pub fn build(self) -> AuthClient {
        AuthClient {
            pool: self.pool,
            http_sender: self.http_sender,
            providers: self.providers,
        }
    }
}

impl AuthClient {
    pub fn builder(pool: Arc<dyn TokenPool + Send + Sync>) -> AuthClientBuilder {
        #[cfg(not(feature = "inte"))]
        let http_sender: Arc<dyn HttpSender> = Arc::new(ReqwestSender::new());

        #[cfg(feature = "inte")]
        let http_sender: Arc<dyn HttpSender> =
            if std::env::var("OWENC_ENV").unwrap_or_default() == "inte" {
                Arc::new(MockHttpSender {
                    real_sender: ReqwestSender::new(),
                })
            } else {
                Arc::new(ReqwestSender::new())
            };

        AuthClientBuilder {
            pool,
            http_sender,
            providers: HashMap::new(),
        }
    }

    pub fn provider(&self, mode: &AuthMode) -> &dyn AuthProvider {
        self.providers
            .get(mode)
            .map(|p| p.as_ref())
            .unwrap_or_else(|| {
                panic!("No provider registered for mode: {:?}", mode);
            })
    }
}

#[async_trait]
impl Client for AuthClient {
    async fn get_token(
        &self,
        profile: &str,
        cfg: &Config,
        headers: &reqwest::header::HeaderMap,
    ) -> CowenResult<cowen_common::models::Token> {
        self.provider(&cfg.app_mode)
            .get_token(profile, cfg, headers)
            .await
    }

    async fn refresh_token(
        &self,
        profile: &str,
        cfg: &Config,
        headers: &reqwest::header::HeaderMap,
    ) -> CowenResult<cowen_common::models::Token> {
        self.provider(&cfg.app_mode)
            .refresh(profile, cfg, headers)
            .await
    }

    async fn handle_platform_event(
        &self,
        profile: &str,
        cfg: &Config,
        event: crate::provider::PlatformEvent,
    ) -> CowenResult<()> {
        self.provider(&cfg.app_mode)
            .handle_platform_event(profile, cfg, event)
            .await
    }

    async fn perform_login(
        &self,
        profile: &str,
        cfg: &Config,
        force: bool,
        finalize: Option<&str>,
        daemon_service: Option<std::sync::Arc<dyn cowen_common::daemon::DaemonService>>,
    ) -> CowenResult<()> {
        self.provider(&cfg.app_mode)
            .perform_login(profile, cfg, force, finalize, daemon_service)
            .await
    }

    async fn on_maintenance_tick(&self, profile: &str, cfg: &Config) -> CowenResult<()> {
        self.provider(&cfg.app_mode)
            .on_maintenance_tick(profile, cfg)
            .await
    }

    async fn requires_initial_push(&self, cfg: &Config) -> bool {
        self.provider(&cfg.app_mode)
            .requires_initial_push(cfg)
            .await
    }

    fn requires_ticket(&self, cfg: &Config) -> bool {
        self.provider(&cfg.app_mode).requires_ticket()
    }

    fn supports_webhooks(&self, cfg: &Config) -> bool {
        self.provider(&cfg.app_mode).supports_webhooks()
    }

    fn supports_api_call(&self, cfg: &Config) -> bool {
        self.provider(&cfg.app_mode).supports_api_call()
    }

    async fn clear_token(&self, profile: &str, cfg: &Config) -> CowenResult<()> {
        // 1. Generic cleanup (Access token pool)
        self.pool.delete_access_token(profile).await?;
        let _ = self.pool.delete_app_access_token(&cfg.app_key).await;

        // 2. Mode-specific cleanup
        self.provider(&cfg.app_mode).on_logout(profile, cfg).await
    }

    async fn trigger_push(&self, profile: &str, cfg: &Config, force: bool) -> CowenResult<()> {
        self.provider(&cfg.app_mode)
            .trigger_push(profile, cfg, force)
            .await
    }

    async fn get_app_access_token(
        &self,
        profile: &str,
        cfg: &Config,
    ) -> CowenResult<cowen_common::models::Token> {
        let token = self
            .provider(&cfg.app_mode)
            .get_app_access_token(profile, cfg)
            .await?;
        // 🚀 归档到持久化池
        let _ = self.pool.set_app_access_token(&cfg.app_key, &token).await;
        Ok(token)
    }

    async fn refresh_app_access_token(
        &self,
        profile: &str,
        cfg: &Config,
    ) -> CowenResult<cowen_common::models::Token> {
        let token = self
            .provider(&cfg.app_mode)
            .refresh(profile, cfg, &reqwest::header::HeaderMap::new())
            .await?;
        let _ = self.pool.set_app_access_token(&cfg.app_key, &token).await;
        Ok(token)
    }

    async fn exchange_temp_code(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
        temp_code: &str,
    ) -> CowenResult<cowen_common::models::Token> {
        self.provider(&cfg.app_mode)
            .exchange_temp_code(profile, cfg, org_id, temp_code)
            .await
    }

    async fn get_user_access_token(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
        user_id: &str,
    ) -> CowenResult<cowen_common::models::Token> {
        self.provider(&cfg.app_mode)
            .get_user_token(profile, cfg, org_id, user_id)
            .await
    }

    async fn intercept_exchange(
        &self,
        profile: &str,
        cfg: &Config,
        body_bytes: &[u8],
    ) -> CowenResult<serde_json::Value> {
        self.provider(&cfg.app_mode)
            .intercept_exchange(profile, cfg, body_bytes)
            .await
    }

    async fn get_diagnostics(
        &self,
        ctx: &cowen_common::status::StatusContext<'_>,
    ) -> CowenResult<Vec<cowen_common::status::StatusEntry>> {
        self.provider(&ctx.config.app_mode)
            .get_diagnostics(ctx)
            .await
    }

    fn get_provider(
        &self,
        mode: &cowen_common::models::AuthMode,
    ) -> Option<Arc<dyn crate::provider::AuthProvider>> {
        self.providers.get(mode).cloned()
    }

    async fn get_openapi_spec(
        &self,
        profile: &str,
        cfg: &Config,
        force_refresh: bool,
    ) -> CowenResult<serde_json::Value> {
        let app_dir = cowen_common::config::get_app_dir();
        let cache_path = app_dir.join(format!("{}_openapi.yaml", profile));

        // 1. Try Load Cache ONLY if not forcing refresh
        if !force_refresh && cache_path.exists() {
            if let Ok(data) = std::fs::read_to_string(&cache_path) {
                if let Ok(spec) = serde_yaml::from_str::<serde_json::Value>(&data) {
                    let metadata = std::fs::metadata(&cache_path).map_err(CowenError::from)?;
                    let elapsed = metadata
                        .modified()
                        .map_err(CowenError::from)?
                        .elapsed()
                        .map_err(|e| CowenError::Internal(e.to_string()))?
                        .as_secs();

                    // Cache is fresh (less than 1 hour)
                    if elapsed < 3600 {
                        return Ok(spec);
                    }

                    // Cache expired but we have it as fallback if network fails
                    tracing::debug!(target: "sys", "Local spec cache expired ({}s), attempting refresh...", elapsed);
                }
            }
        }

        if force_refresh {
            tracing::debug!(target: "sys", "Force refresh requested, bypassing local cache.");
        }

        let app_cfg = cowen_config::ConfigManager::new()?.load_app_config().await?;
        // 2. Fetch fresh spec from Platform
        if !app_cfg.openapi_url.is_empty() {
            let mut spec_url = format!(
                "{}{}",
                app_cfg.openapi_url.trim_end_matches('/'),
                obfs!("/v1/common/openapi/spec")
            );
            if let Ok(token) = self
                .get_token(profile, cfg, &reqwest::header::HeaderMap::new())
                .await
            {
                let mut headers = reqwest::header::HeaderMap::new();

                // OCP: Delegate URL and Header decoration to provider
                self.provider(&cfg.app_mode).decorate_openapi_request(
                    &mut spec_url,
                    &mut headers,
                    &token,
                    cfg,
                );

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
        tracing::debug!(target: "sys", "Full OpenAPI spec unavailable or refresh needed. Fetching real authorized interface list...");
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
                Err(CowenError::Api(format!(
                    "Could not refresh API list: {}",
                    e
                )))
            }
        }
    }

    async fn get_dynamic_interface_list(
        &self,
        profile: &str,
        cfg: &Config,
    ) -> CowenResult<serde_json::Value> {
        let token = self
            .get_token(profile, cfg, &reqwest::header::HeaderMap::new())
            .await?;
        let app_cfg = cowen_config::ConfigManager::new()?.load_app_config().await?;
        let base_url = format!(
            "{}{}",
            app_cfg.openapi_url.trim_end_matches('/'),
            obfs!("/developer/api/apiPermissions/isv/open/getInterfaceList")
        );

        let mut current_page = 1;
        let mut total_pages = 1;
        let mut combined_paths = serde_json::Map::new();

        while current_page <= total_pages {
            let mut url = base_url.clone();
            let mut headers = reqwest::header::HeaderMap::new();

            // OCP: Delegate URL and Header decoration to provider
            self.provider(&cfg.app_mode).decorate_openapi_request(
                &mut url,
                &mut headers,
                &token,
                cfg,
            );

            // Append standard pagination params
            if url.contains('?') {
                url.push_str(&format!("&page={}&size=100", current_page - 1));
            } else {
                url.push_str(&format!("?page={}&size=100", current_page - 1));
            }

            tracing::debug!(target: "sys", "Fetching interface list page {}/{}", current_page, total_pages);
            let resp = self.http_sender.get(&url, headers).await?;

            if !resp.is_success() {
                let status = resp.status;
                let mut err_msg = format!(
                    "Failed to fetch interface list page {}: HTTP {}",
                    current_page, status
                );
                
                if status == 500 {
                    err_msg.push_str("\n\n💡 提示 (Hint): 服务端返回 500 内部错误 (Internal Server Error)。\n如果您使用的是新创建的【自建应用 (Self-Built)】，此错误通常是因为该应用尚未在任何企业（账套）中完成实质性安装或启用，导致服务端查询不到关联的授权数据。\n请前往开放平台确保该应用已至少关联一个企业账套。");
                }
                
                return Err(CowenError::Api(err_msg));
            }

            let body: serde_json::Value = resp.json().await?;
            let value = body
                .get("value")
                .ok_or_else(|| CowenError::Api("Invalid response structure".to_string()))?;

            tracing::debug!(target: "sys", "Server reported currentPage: {:?}, totalPages: {:?}", value.get("currentPage"), value.get("totalPages"));

            // Update pagination info
            total_pages = value
                .get("totalPages")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as usize;

            if let Some(list) = value.get("resultList").and_then(|l| l.as_array()) {
                tracing::debug!(target: "sys", "Page {} resultList contains {} items", current_page, list.len());
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
                            if let Some(item_paths) =
                                item_spec.get("paths").and_then(|p| p.as_object())
                            {
                                parsed_paths = Some(item_paths.clone());
                            }
                        }
                    }

                    // 2. Merge paths or use fallback
                    if let Some(item_paths) = parsed_paths {
                        for (path, methods) in item_paths {
                            if let Some(existing) = combined_paths.get_mut(&path) {
                                if let (Some(e_obj), Some(m_obj)) =
                                    (existing.as_object_mut(), methods.as_object())
                                {
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
                        let path = item
                            .get("requestPath")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let name = item
                            .get("interfaceName")
                            .and_then(|v| v.as_str())
                            .unwrap_or("No Name");
                        let method = item
                            .get("requestHttpMethod")
                            .and_then(|v| v.as_str())
                            .unwrap_or("GET")
                            .to_lowercase();

                        if !path.is_empty() {
                            let mut methods_obj = serde_json::Map::new();
                            methods_obj.insert(
                                method.clone(),
                                serde_json::json!({
                                    "summary": name,
                                    "description": format!("Authorized Interface: {}", name),
                                    "responses": { "200": { "description": "OK" } }
                                }),
                            );

                            if let Some(existing) = combined_paths.get_mut(path) {
                                if let Some(e_obj) = existing.as_object_mut() {
                                    e_obj.insert(
                                        method.clone(),
                                        methods_obj.get(&method).unwrap().clone(),
                                    );
                                }
                            } else {
                                combined_paths.insert(
                                    path.to_string(),
                                    serde_json::Value::Object(methods_obj),
                                );
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

impl AuthClient {
    fn save_spec_to_cache(
        &self,
        path: &std::path::PathBuf,
        spec: &serde_json::Value,
    ) -> CowenResult<()> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let yaml_data = serde_yaml::to_string(spec)?;
        cowen_common::utils::secure_write(path, yaml_data)?;
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
                        if name.eq_ignore_ascii_case("content-type")
                            && in_location.eq_ignore_ascii_case("header")
                        {
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

pub use cowen_common::openapi::{find_matching_spec_path, get_operation, is_path_in_whitelist};
