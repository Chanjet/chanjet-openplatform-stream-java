use super::*;
use crate::models::{Token, Ticket, AuthSession};
use crate::client::{HttpSender, SimpleResponse};
use crate::VaultTokenPool;
use std::sync::Arc;
use async_trait::async_trait;

// --- Manual Mocks ---

struct MockVault {}
#[async_trait]
impl cowen_common::vault::Vault for MockVault {
    fn primary_store(&self) -> Arc<dyn cowen_store::Store> {
        unimplemented!()
    }
}

#[async_trait]
impl crate::domain::PermanentCodeDomain for MockVault {
    async fn get_org_permanent_code(&self, _: &str, _: &str) -> Result<String> { Err(anyhow!("not found")) }
    async fn save_org_permanent_code(&self, _: &str, _: &str, _: &str) -> Result<()> { Ok(()) }
    async fn get_user_permanent_code(&self, _: &str, _: &str, _: &str) -> Result<String> { Err(anyhow!("not found")) }
    async fn save_user_permanent_code(&self, _: &str, _: &str, _: &str, _: &str) -> Result<()> { Ok(()) }
}

#[async_trait]
impl crate::domain::TicketDomain for MockVault {
    async fn get_app_ticket(&self, _: &str) -> Result<Ticket> { Err(anyhow!("not found")) }
    async fn save_app_ticket(&self, _: &str, _: Ticket) -> Result<()> { Ok(()) }
}

#[async_trait]
impl crate::domain::TokenDomain for MockVault {
    async fn get_access_token(&self, _: &str) -> Result<cowen_common::models::Token> { Err(anyhow!("not found")) }
    async fn save_access_token(&self, _: &str, _: Token) -> Result<()> { Ok(()) }
    async fn delete_access_token(&self, _: &str) -> Result<()> { Ok(()) }
    async fn get_app_access_token(&self, _: &str) -> Result<cowen_common::models::Token> { Err(anyhow!("not found")) }
    async fn save_app_access_token(&self, _: &str, _: Token) -> Result<()> { Ok(()) }
}

#[async_trait]
impl crate::domain::SessionDomain for MockVault {
    async fn get_session(&self, _: &str) -> Result<AuthSession> { Err(anyhow!("not found")) }
    async fn save_session(&self, _: AuthSession) -> Result<()> { Ok(()) }
    async fn delete_session(&self, _: &str) -> Result<()> { Ok(()) }
}

#[async_trait]
impl crate::domain::SecretDomain for MockVault {
    async fn get_secret(&self, _: &str, _: &str) -> Result<String> { Ok("".to_string()) }
    async fn set_secret(&self, _: &str, _: &str, _: &str) -> Result<()> { Ok(()) }
    async fn delete_secret(&self, _: &str, _: &str) -> Result<()> { Ok(()) }
}

#[async_trait]
impl crate::domain::ConfigDomain for MockVault {
    async fn get_config(&self, _: &str, _: &str) -> Result<String> { Ok("".to_string()) }
    async fn get_config_full(&self, _: &str, _: &str) -> Result<cowen_store::Item> { Err(anyhow!("not found")) }
    async fn set_config(&self, _: &str, _: &str, _: &str) -> Result<()> { Ok(()) }
    async fn set_config_conditional(&self, _: &str, _: &str, _: &str, _: u64) -> Result<()> { Ok(()) }
    async fn list_configs(&self, _: &str) -> Result<Vec<String>> { Ok(vec![]) }
    async fn delete_config(&self, _: &str, _: &str) -> Result<()> { Ok(()) }
}

#[async_trait]
impl crate::domain::AuditDomain for MockVault {
    async fn save_audit(&self, _: &cowen_store::AuditEntry) -> Result<()> { Ok(()) }
    async fn list_audit(&self, _: &str, _: usize) -> Result<Vec<cowen_store::AuditEntry>> { Ok(vec![]) }
}

#[async_trait]
impl crate::domain::DlqDomain for MockVault {
    async fn push_dlq(&self, _: &cowen_store::DlqMessage) -> Result<()> { Ok(()) }
    async fn pop_dlq(&self, _: &str, _: &str) -> Result<Option<cowen_store::DlqMessage>> { Ok(None) }
    async fn list_dlq(&self, _: &str, _: usize) -> Result<Vec<cowen_store::DlqMessage>> { Ok(vec![]) }
    async fn list_all_dlq(&self, _: &str) -> Result<Vec<cowen_store::DlqMessage>> { Ok(vec![]) }
}

#[async_trait]
impl crate::domain::ManagementDomain for MockVault {
    async fn clear_profile(&self, _: &str) -> Result<()> { Ok(()) }
    async fn rename_profile(&self, _: &str, _: &str) -> Result<()> { Ok(()) }
    async fn list_all_profiles(&self) -> Result<Vec<String>> { Ok(vec![]) }
}

struct MockHttpSender {}
#[async_trait]
impl HttpSender for MockHttpSender {
    async fn post(&self, _url: &str, _headers: reqwest::header::HeaderMap, _body: serde_json::Value) -> Result<SimpleResponse> {
        Ok(SimpleResponse { status: 200, body: "{}".to_string() })
    }
    async fn post_form(&self, _url: &str, _headers: reqwest::header::HeaderMap, _body: serde_json::Value) -> Result<SimpleResponse> {
        Ok(SimpleResponse { status: 200, body: "{}".to_string() })
    }
    async fn get(&self, _url: &str, _headers: reqwest::header::HeaderMap) -> Result<SimpleResponse> {
        Ok(SimpleResponse { status: 200, body: "{}".to_string() })
    }
}

#[tokio::test]
async fn test_get_token_missing_org_id_rejection() {
    let vault = Arc::new(MockVault {});
    let pool: Arc<dyn TokenPool> = Arc::new(VaultTokenPool::new(vault));
    let sender = Arc::new(MockHttpSender {});
    let provider = StoreAppProvider::new(pool, sender);
    
    let config = Config::default_with_profile("test");
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-user-id", "U123".parse().unwrap());

    let result = provider.get_token("default", &config, &headers).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("401 Unauthorized"));
    assert!(err.to_string().contains("x-org-id"));
}

#[tokio::test]
async fn test_get_token_with_org_only_isolation() {
    let vault = Arc::new(MockVault {});
    let pool: Arc<dyn TokenPool> = Arc::new(VaultTokenPool::new(vault));
    let sender = Arc::new(MockHttpSender {});
    let _provider = StoreAppProvider::new(pool, sender);
    
    let mut config = Config::default_with_profile("test");
    config.app_key = "test_app".to_string();
    
    let key = storage::get_org_token_key("test_app", "ORG1");
    assert_eq!(key, "oauth2_token_pair_org_test_app_ORG1");
}

#[tokio::test]
async fn test_get_token_with_user_isolation() {
    let vault = Arc::new(MockVault {});
    let pool: Arc<dyn TokenPool> = Arc::new(VaultTokenPool::new(vault));
    let sender = Arc::new(MockHttpSender {});
    let _provider = StoreAppProvider::new(pool, sender);
    
    let mut config = Config::default_with_profile("test");
    config.app_key = "test_app".to_string();
    
    let key = storage::get_user_token_key("test_app", "ORG1", "USER1");
    assert_eq!(key, "oauth2_token_pair_user_test_app_ORG1_USER1");
}
