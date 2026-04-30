use crate::auth::models::Token;
use crate::core::config::Config;
use anyhow::Result;
use async_trait::async_trait;

pub mod self_built;
pub mod store_app;
pub mod oauth2;

pub enum ProxyRequestAction {
    Forward {
        headers: reqwest::header::HeaderMap,
    },
    Respond(serde_json::Value),
}

#[derive(Debug, Default)]
pub struct InitParams {
    pub app_key: Option<String>,
    pub app_secret: Option<String>,
    pub certificate: Option<String>,
    pub encrypt_key: Option<String>,
    pub webhook_target: Option<String>,
    pub openapi_url: Option<String>,
    pub stream_url: Option<String>,
}

#[derive(Debug, Clone)]
pub enum PlatformEvent {
    AppTicket(String),
    TempAuthCode {
        code: String,
        org_id: Option<String>,
    },
}

#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// 获取当前可用令牌。若过期则触发刷新或网络整改。
    async fn get_token(&self, profile: &str, config: &Config, headers: &reqwest::header::HeaderMap) -> Result<Token>;
    
    /// 强制执行网络刷新逻辑（忽略内存或本地缓存）。
    async fn refresh(&self, profile: &str, config: &Config, headers: &reqwest::header::HeaderMap) -> Result<Token>;

    /// 🚀 维护周期回调：由守护进程定时触发，负责令牌“保鲜”或状态修复
    async fn on_maintenance_tick(&self, profile: &str, config: &Config) -> Result<()> {
        let _ = (profile, config);
        Ok(())
    }

    /// 🚀 初始推送检查：决定是否在启动时强制要求平台推送初始凭证 (如 AppTicket)
    fn requires_initial_push(&self, config: &Config) -> bool {
        let _ = config;
        false
    }

    /// 🚀 平台事件处理器：处理来自 WebSocket 流的异步事件 (如 APP_TICKET, TEMP_AUTH_CODE)
    async fn handle_platform_event(&self, profile: &str, config: &Config, event: PlatformEvent) -> Result<()> {
        let _ = (profile, config, event);
        Ok(())
    }

    /// 🚀 UI/诊断能力：返回该模式在状态列表中显示的图标与名称 (Auth 模块)
    fn get_auth_display_info(&self) -> (String, String) {
        ("Authentication".to_string(), "🔐".to_string())
    }

    /// 🚀 UI/诊断能力：返回该模式后台进程的显示名称与效率提示
    fn get_daemon_display_info(&self, is_running: bool) -> (String, String) {
        let name = if is_running { "Auth Renewer (Daemon)" } else { "Auth Bridge (Daemon)" };
        let tip = if is_running { "主动续约: [ACTIVE]" } else { "建议运行 'cowen daemon start' 以实现自动续约" };
        (name.to_string(), tip.to_string())
    }

    /// 🚀 能力检查：该模式是否需要 AppTicket (用于 Ticket 采集器显示)
    fn requires_ticket(&self) -> bool {
        false
    }

    /// 🚀 能力检查：该模式是否支持 Webhook/Streaming 推送能力
    fn supports_webhooks(&self) -> bool {
        true
    }

    /// OCP: Capability check for OpenAPI call support.
    fn supports_api_call(&self) -> bool {
        true
    }

    /// OCP: Unified Initialization Hook.
    async fn initialize(
        &self,
        profile: &str,
        config: &mut Config,
        vault: std::sync::Arc<dyn crate::core::vault::Vault>,
        cfg_mgr: &crate::core::config::ConfigManager,
        params: InitParams,
    ) -> Result<()>;

    /// 🚀 前置请求拦截器：负责请求修饰（Header/Token注入）或请求劫持短路
    async fn intercept_request(
        &self,
        profile: &str,
        config: &Config,
        _path: &str,
        _method: &str,
        headers: reqwest::header::HeaderMap,
        body: &[u8],
        spec: &serde_json::Value,
    ) -> Result<ProxyRequestAction> {
        let mut headers = headers;
        let token = self.get_token(profile, config, &headers).await?;
        headers.insert("Authorization", format!("Bearer {}", token.value).parse().unwrap());
        let _ = (body, spec);
        Ok(ProxyRequestAction::Forward { headers })
    }

    async fn intercept_response(
        &self,
        profile: &str,
        config: &Config,
        path: &str,
        method: &str,
        status: u16,
        headers: &reqwest::header::HeaderMap,
        body: &[u8],
    ) -> Result<Option<serde_json::Value>> {
        let _ = (profile, config, path, method, status, headers, body);
        Ok(None)
    }

    async fn on_login(&self, _profile: &str, _config: &Config, _headers: &mut reqwest::header::HeaderMap) -> Result<()> {
        Ok(())
    }

    async fn on_logout(&self, _profile: &str, _config: &Config) -> Result<()> {
        Ok(())
    }

    async fn perform_login(&self, _profile: &str, _config: &Config, _force: bool, _finalize: Option<&str>) -> Result<()> {
        Ok(())
    }

    async fn get_status_entries(&self, _profile: &str, _config: &Config) -> Result<Vec<crate::core::status::StatusEntry>> {
        Ok(vec![])
    }

    fn get_capabilities(&self) -> Vec<String> {
        vec![]
    }

    fn get_default_app_key(&self) -> Option<String> {
        None
    }

    fn decorate_openapi_request(&self, _url: &mut String, _headers: &mut reqwest::header::HeaderMap, _token: &Token, _config: &Config) {
    }
}
