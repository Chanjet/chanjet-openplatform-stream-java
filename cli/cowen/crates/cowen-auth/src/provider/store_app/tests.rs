use cowen_common::{CowenResult, CowenError};
use async_trait::async_trait;
use super::*;
use crate::models::{Token, Ticket, AuthSession};
use crate::client::{HttpSender, SimpleResponse};
use crate::VaultTokenPool;
use std::sync::Arc;


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
    async fn get_org_permanent_code(&self, _: &str, _: &str) -> CowenResult<String> { Err(CowenError::Auth(format!("not found"))) }
    async fn save_org_permanent_code(&self, _: &str, _: &str, _: &str) -> CowenResult<()> { Ok(()) }
    async fn get_user_permanent_code(&self, _: &str, _: &str, _: &str) -> CowenResult<String> { Err(CowenError::Auth(format!("not found"))) }
    async fn save_user_permanent_code(&self, _: &str, _: &str, _: &str, _: &str) -> CowenResult<()> { Ok(()) }
}

#[async_trait]
impl crate::domain::TicketDomain for MockVault {
    async fn get_app_ticket(&self, _: &str) -> CowenResult<Ticket> { Err(CowenError::Auth(format!("not found"))) }
    async fn save_app_ticket(&self, _: &str, _: Ticket) -> CowenResult<()> { Ok(()) }
}

#[async_trait]
impl crate::domain::TokenDomain for MockVault {
    async fn get_access_token(&self, _: &str) -> CowenResult<cowen_common::models::Token> { Err(CowenError::Auth(format!("not found"))) }
    async fn save_access_token(&self, _: &str, _: Token) -> CowenResult<()> { Ok(()) }
    async fn delete_access_token(&self, _: &str) -> CowenResult<()> { Ok(()) }
    async fn get_app_access_token(&self, _: &str) -> CowenResult<cowen_common::models::Token> { Err(CowenError::Auth(format!("not found"))) }
    async fn save_app_access_token(&self, _: &str, _: Token) -> CowenResult<()> { Ok(()) }
}

#[async_trait]
impl crate::domain::SessionDomain for MockVault {
    async fn get_session(&self, _: &str) -> CowenResult<AuthSession> { Err(CowenError::Auth(format!("not found"))) }
    async fn save_session(&self, _: AuthSession) -> CowenResult<()> { Ok(()) }
    async fn delete_session(&self, _: &str) -> CowenResult<()> { Ok(()) }
}

#[async_trait]
impl crate::domain::SecretDomain for MockVault {
    async fn get_secret(&self, _: &str, _: &str) -> CowenResult<String> { Ok("".to_string()) }
    async fn set_secret(&self, _: &str, _: &str, _: &str) -> CowenResult<()> { Ok(()) }
    async fn delete_secret(&self, _: &str, _: &str) -> CowenResult<()> { Ok(()) }
}

#[async_trait]
impl crate::domain::ConfigDomain for MockVault {
    async fn get_config(&self, _: &str, _: &str) -> CowenResult<String> { Ok("".to_string()) }
    async fn get_config_full(&self, _: &str, _: &str) -> CowenResult<cowen_store::Item> { Err(CowenError::Auth(format!("not found"))) }
    async fn set_config(&self, _: &str, _: &str, _: &str) -> CowenResult<()> { Ok(()) }
    async fn set_config_conditional(&self, _: &str, _: &str, _: &str, _: u64) -> CowenResult<()> { Ok(()) }
    async fn list_configs(&self, _: &str) -> CowenResult<Vec<String>> { Ok(vec![]) }
    async fn delete_config(&self, _: &str, _: &str) -> CowenResult<()> { Ok(()) }
}

#[async_trait]
impl crate::domain::AuditDomain for MockVault {
    async fn save_audit(&self, _: &cowen_store::AuditEntry) -> CowenResult<()> { Ok(()) }
    async fn list_audit(&self, _: &str, _: usize) -> CowenResult<Vec<cowen_store::AuditEntry>> { Ok(vec![]) }
}

#[async_trait]
impl crate::domain::DlqDomain for MockVault {
    async fn push_dlq(&self, _: &cowen_store::DlqMessage) -> CowenResult<()> { Ok(()) }
    async fn pop_dlq(&self, _: &str, _: &str) -> CowenResult<Option<cowen_store::DlqMessage>> { Ok(None) }
    async fn list_dlq(&self, _: &str, _: usize) -> CowenResult<Vec<cowen_store::DlqMessage>> { Ok(vec![]) }
    async fn list_all_dlq(&self, _: &str) -> CowenResult<Vec<cowen_store::DlqMessage>> { Ok(vec![]) }
}

#[async_trait]
impl crate::domain::ManagementDomain for MockVault {
    async fn clear_profile(&self, _: &str) -> CowenResult<()> { Ok(()) }
    async fn rename_profile(&self, _: &str, _: &str) -> CowenResult<()> { Ok(()) }
    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> { Ok(vec![]) }
}

struct MockHttpSender {}
#[async_trait]
impl HttpSender for MockHttpSender {
    async fn post(&self, _url: &str, _headers: reqwest::header::HeaderMap, _body: serde_json::Value) -> CowenResult<SimpleResponse> {
        Ok(SimpleResponse { status: 200, body: "{}".to_string() })
    }
    async fn post_form(&self, _url: &str, _headers: reqwest::header::HeaderMap, _body: serde_json::Value) -> CowenResult<SimpleResponse> {
        Ok(SimpleResponse { status: 200, body: "{}".to_string() })
    }
    async fn get(&self, _url: &str, _headers: reqwest::header::HeaderMap) -> CowenResult<SimpleResponse> {
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
