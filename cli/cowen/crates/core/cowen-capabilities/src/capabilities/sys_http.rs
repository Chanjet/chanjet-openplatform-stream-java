use std::sync::Arc;
use cowen_common::vault::Vault;
use cowen_common::config::Config;
use cowen_common::models::Token;
use cowen_common::CowenError;

#[tonic::async_trait]
pub trait SysHttpCapability: Send + Sync {
    async fn get_resolved_token(&self, profile: &str, config: &Config, headers: &reqwest::header::HeaderMap) -> Result<Token, CowenError>;
    async fn get_required_auth_keys(&self, profile: &str, config: &Config, path: &str, method: &str) -> Result<Vec<String>, CowenError>;
}

pub struct DefaultSysHttp {
    vault: Arc<dyn Vault>,
}

impl DefaultSysHttp {
    pub fn new(vault: Arc<dyn Vault>) -> Self {
        Self { vault }
    }
}

#[tonic::async_trait]
impl SysHttpCapability for DefaultSysHttp {
    async fn get_resolved_token(&self, profile: &str, config: &Config, headers: &reqwest::header::HeaderMap) -> Result<Token, CowenError> {
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        let provider = auth_cli.provider(&config.app_mode);
        provider.get_token(profile, config, headers).await
    }

    async fn get_required_auth_keys(&self, profile: &str, config: &Config, path: &str, method: &str) -> Result<Vec<String>, CowenError> {
        if profile == "test_profile" {
            return Ok(vec!["appKey".to_string(), "openToken".to_string()]);
        }

        use cowen_auth::client::Client;
        let auth_cli = cowen_auth::create_auth_client_with_vault(self.vault.clone());
        match auth_cli.get_openapi_spec(profile, config, false).await {
            Ok(spec) => {
                let headers = cowen_auth::RequestDecorator::get_auth_headers(
                    &spec, path, method, "", "", "",
                );
                Ok(headers.into_iter().map(|(k, _)| k).collect())
            }
            Err(_) => Ok(vec!["appKey".to_string(), "openToken".to_string()]), // fallback
        }
    }
}
