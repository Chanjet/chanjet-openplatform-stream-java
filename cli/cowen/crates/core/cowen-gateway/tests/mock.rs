use async_trait::async_trait;
use cowen_auth::client::Client;
use cowen_common::models::Token;
use cowen_common::{config::Config, CowenError, CowenResult};

pub struct MockClient {
    pub should_auth_fail: bool,
}

impl MockClient {
    pub fn new() -> Self {
        Self {
            should_auth_fail: true,
        }
    }
}

impl Default for MockClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Client for MockClient {
    async fn get_token(
        &self,
        _profile: &str,
        _cfg: &Config,
        _headers: &reqwest::header::HeaderMap,
    ) -> CowenResult<Token> {
        if self.should_auth_fail {
            Err(CowenError::Auth("Mock Auth Failed".into()))
        } else {
            Ok(Token {
                value: "mock_token".into(),
                expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
                created_at: chrono::Utc::now(),
            })
        }
    }

    async fn refresh_token(
        &self,
        _profile: &str,
        _cfg: &Config,
        _headers: &reqwest::header::HeaderMap,
    ) -> CowenResult<Token> {
        Err(CowenError::Auth("Mock Auth Failed".into()))
    }

    async fn trigger_push(&self, _profile: &str, _cfg: &Config, _force: bool) -> CowenResult<()> {
        Ok(())
    }

    async fn get_openapi_spec(
        &self,
        _profile: &str,
        _cfg: &Config,
        _force_refresh: bool,
    ) -> CowenResult<serde_json::Value> {
        Ok(serde_json::json!({}))
    }

    async fn get_dynamic_interface_list(
        &self,
        _profile: &str,
        _cfg: &Config,
    ) -> CowenResult<serde_json::Value> {
        Ok(serde_json::json!([]))
    }

    async fn clear_token(&self, _profile: &str, _cfg: &Config) -> CowenResult<()> {
        Ok(())
    }

    async fn get_app_access_token(&self, _profile: &str, _cfg: &Config) -> CowenResult<Token> {
        Err(CowenError::Auth("not found".into()))
    }

    async fn refresh_app_access_token(&self, _profile: &str, _cfg: &Config) -> CowenResult<Token> {
        Err(CowenError::Auth("not found".into()))
    }

    async fn exchange_temp_code(
        &self,
        _profile: &str,
        _cfg: &Config,
        _org_id: &str,
        _temp_code: &str,
    ) -> CowenResult<Token> {
        Err(CowenError::Auth("not found".into()))
    }

    async fn get_user_access_token(
        &self,
        _profile: &str,
        _cfg: &Config,
        _org_id: &str,
        _user_id: &str,
    ) -> CowenResult<Token> {
        Err(CowenError::Auth("not found".into()))
    }

    async fn intercept_exchange(
        &self,
        _profile: &str,
        _cfg: &Config,
        _body_bytes: &[u8],
    ) -> CowenResult<serde_json::Value> {
        Ok(serde_json::json!({}))
    }

    async fn on_maintenance_tick(&self, _profile: &str, _cfg: &Config) -> CowenResult<()> {
        Ok(())
    }

    async fn requires_initial_push(&self, _cfg: &Config) -> bool {
        false
    }

    async fn handle_platform_event(
        &self,
        _profile: &str,
        _cfg: &Config,
        _event: cowen_auth::provider::PlatformEvent,
    ) -> CowenResult<()> {
        Ok(())
    }

    fn requires_ticket(&self, _cfg: &Config) -> bool {
        false
    }

    fn supports_webhooks(&self, _cfg: &Config) -> bool {
        false
    }

    fn supports_api_call(&self, _cfg: &Config) -> bool {
        false
    }

    async fn perform_login(
        &self,
        _profile: &str,
        _cfg: &Config,
        _force: bool,
        _finalize: Option<&str>,
        _daemon_service: Option<std::sync::Arc<dyn cowen_common::daemon::DaemonService>>,
    ) -> CowenResult<()> {
        Ok(())
    }

    async fn get_diagnostics(
        &self,
        _ctx: &cowen_common::status::StatusContext<'_>,
    ) -> CowenResult<Vec<cowen_common::status::StatusEntry>> {
        Ok(vec![])
    }

    fn get_provider(
        &self,
        _mode: &cowen_common::models::AuthMode,
    ) -> Option<std::sync::Arc<dyn cowen_auth::provider::AuthProvider>> {
        None
    }
}
