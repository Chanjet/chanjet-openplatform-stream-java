use async_trait::async_trait;
use chrono::{Duration, Utc};
use cowen_auth::provider::oauth2::OAuth2Provider;
use cowen_auth::provider::AuthProvider;
use cowen_common::domain::*;
use cowen_common::models::{AuthSession, Ticket, Token};
use cowen_common::{CowenError, CowenResult};
use cowen_store::Item;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

struct MockVault {
    tokens: Mutex<HashMap<String, Token>>,
    refresh_tokens: Mutex<HashMap<String, Token>>,
    configs: Mutex<HashMap<String, String>>,
}

impl MockVault {
    fn new() -> Self {
        Self {
            tokens: Mutex::new(HashMap::new()),
            refresh_tokens: Mutex::new(HashMap::new()),
            configs: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl cowen_common::vault::Vault for MockVault {
    fn primary_store(&self) -> Arc<dyn cowen_store::Store> {
        unimplemented!()
    }
}

#[async_trait]
impl TokenDomain for MockVault {
    async fn get_access_token(&self, p: &str) -> CowenResult<Token> {
        self.tokens
            .lock()
            .await
            .get(p)
            .cloned()
            .ok_or(CowenError::Auth("not found".to_string()))
    }
    async fn save_access_token(&self, p: &str, t: Token) -> CowenResult<()> {
        self.tokens.lock().await.insert(p.to_string(), t);
        Ok(())
    }
    async fn delete_access_token(&self, p: &str) -> CowenResult<()> {
        self.tokens.lock().await.remove(p);
        Ok(())
    }
    async fn get_refresh_token(&self, p: &str) -> CowenResult<Token> {
        self.refresh_tokens
            .lock()
            .await
            .get(p)
            .cloned()
            .ok_or(CowenError::Auth("not found".to_string()))
    }
    async fn save_refresh_token(&self, p: &str, t: Token) -> CowenResult<()> {
        self.refresh_tokens.lock().await.insert(p.to_string(), t);
        Ok(())
    }
    async fn delete_refresh_token(&self, p: &str) -> CowenResult<()> {
        self.refresh_tokens.lock().await.remove(p);
        Ok(())
    }
    async fn get_app_access_token(&self, _: &str) -> CowenResult<Token> {
        Err(CowenError::Auth("not found".to_string()))
    }
    async fn save_app_access_token(&self, _: &str, _: Token) -> CowenResult<()> {
        Ok(())
    }
    async fn delete_app_access_token(&self, _: &str) -> CowenResult<()> {
        Ok(())
    }
}

#[async_trait]
impl ConfigDomain for MockVault {
    async fn get_config(&self, _: &str, k: &str) -> CowenResult<String> {
        self.configs
            .lock()
            .await
            .get(k)
            .cloned()
            .ok_or(CowenError::Auth("not found".to_string()))
    }
    async fn set_config(&self, _: &str, k: &str, v: &str) -> CowenResult<()> {
        self.configs
            .lock()
            .await
            .insert(k.to_string(), v.to_string());
        Ok(())
    }
    async fn delete_config(&self, _: &str, k: &str) -> CowenResult<()> {
        self.configs.lock().await.remove(k);
        Ok(())
    }
    // Implement others with defaults...
    async fn get_config_metadata(&self, _: &str, _: &str) -> CowenResult<(u64, i64)> {
        Ok((0, 0))
    }
    async fn get_config_full(&self, _: &str, _: &str) -> CowenResult<Item> {
        Err(CowenError::Auth("not found".to_string()))
    }
    async fn set_config_conditional(&self, _: &str, _: &str, _: &str, _: u64) -> CowenResult<()> {
        Ok(())
    }
    async fn list_configs(&self, _: &str) -> CowenResult<Vec<String>> {
        Ok(vec![])
    }
}

// Stub implementation for other traits...
#[async_trait]
impl PermanentCodeDomain for MockVault {
    async fn get_org_permanent_code(&self, _: &str, _: &str) -> CowenResult<String> {
        Err(CowenError::Auth("not found".to_string()))
    }
    async fn save_org_permanent_code(&self, _: &str, _: &str, _: &str) -> CowenResult<()> {
        Ok(())
    }
    async fn get_user_permanent_code(&self, _: &str, _: &str, _: &str) -> CowenResult<String> {
        Err(CowenError::Auth("not found".to_string()))
    }
    async fn save_user_permanent_code(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
    ) -> CowenResult<()> {
        Ok(())
    }
}
#[async_trait]
impl TicketDomain for MockVault {
    async fn get_app_ticket(&self, _: &str) -> CowenResult<Ticket> {
        Err(CowenError::Auth("not found".to_string()))
    }
    async fn save_app_ticket(&self, _: &str, _: Ticket) -> CowenResult<()> {
        Ok(())
    }
    async fn delete_app_ticket(&self, _: &str) -> CowenResult<()> {
        Ok(())
    }
}
#[async_trait]
impl SessionDomain for MockVault {
    async fn get_session(&self, _: &str) -> CowenResult<AuthSession> {
        Err(CowenError::Auth("not found".to_string()))
    }
    async fn save_session(&self, _: AuthSession) -> CowenResult<()> {
        Ok(())
    }
    async fn delete_session(&self, _: &str) -> CowenResult<()> {
        Ok(())
    }
}
#[async_trait]
impl SecretDomain for MockVault {
    async fn get_secret(&self, _: &str, _: &str) -> CowenResult<String> {
        Err(CowenError::Auth("not found".to_string()))
    }
    async fn set_secret(&self, _: &str, _: &str, _: &str) -> CowenResult<()> {
        Ok(())
    }
    async fn delete_secret(&self, _: &str, _: &str) -> CowenResult<()> {
        Ok(())
    }
}
#[async_trait]
impl AuditDomain for MockVault {
    async fn save_audit(&self, _: &cowen_common::models::AuditEntry) -> CowenResult<()> {
        Ok(())
    }
    async fn list_audit(
        &self,
        _: &str,
        _: usize,
    ) -> CowenResult<Vec<cowen_common::models::AuditEntry>> {
        Ok(vec![])
    }
}
#[async_trait]
impl DlqDomain for MockVault {
    async fn push_dlq(&self, _: &cowen_common::models::DlqMessage) -> CowenResult<()> {
        Ok(())
    }
    async fn pop_dlq(
        &self,
        _: &str,
        _: &str,
    ) -> CowenResult<Option<cowen_common::models::DlqMessage>> {
        Ok(None)
    }
    async fn list_dlq(
        &self,
        _: &str,
        _: usize,
    ) -> CowenResult<Vec<cowen_common::models::DlqMessage>> {
        Ok(vec![])
    }
    async fn list_all_dlq(&self, _: &str) -> CowenResult<Vec<cowen_common::models::DlqMessage>> {
        Ok(vec![])
    }
    async fn get_dlq_by_id(&self, _: i64) -> CowenResult<Option<cowen_common::models::DlqMessage>> {
        Ok(None)
    }
    async fn list_dlq_paged(&self, _: &str, _: usize, _: usize) -> CowenResult<Vec<cowen_common::models::DlqMessage>> {
        Ok(vec![])
    }
    async fn delete_dlq_by_id(&self, _: i64) -> CowenResult<()> {
        Ok(())
    }
}
#[async_trait]
impl ManagementDomain for MockVault {
    async fn clear_profile(&self, _: &str) -> CowenResult<()> {
        Ok(())
    }
    async fn rename_profile(&self, _: &str, _: &str) -> CowenResult<()> {
        Ok(())
    }
    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> {
        Ok(vec![])
    }
}

struct MockHttpSender {}
#[async_trait]
impl cowen_auth::client::HttpSender for MockHttpSender {
    async fn post(
        &self,
        _: &str,
        _: reqwest::header::HeaderMap,
        _: serde_json::Value,
    ) -> CowenResult<cowen_auth::client::SimpleResponse> {
        Ok(cowen_auth::client::SimpleResponse {
            status: 200,
            body: "{\"access_token\":\"new_at\",\"refresh_token\":\"new_rt\",\"expires_in\":3600}"
                .to_string(),
        })
    }
    async fn post_form(
        &self,
        _: &str,
        _: reqwest::header::HeaderMap,
        _: serde_json::Value,
    ) -> CowenResult<cowen_auth::client::SimpleResponse> {
        Ok(cowen_auth::client::SimpleResponse {
            status: 200,
            body: "{\"access_token\":\"new_at\",\"refresh_token\":\"new_rt\",\"expires_in\":3600}"
                .to_string(),
        })
    }
    async fn get(
        &self,
        _: &str,
        _: reqwest::header::HeaderMap,
    ) -> CowenResult<cowen_auth::client::SimpleResponse> {
        Ok(cowen_auth::client::SimpleResponse {
            status: 200,
            body: "{}".to_string(),
        })
    }
}

#[tokio::test]
async fn test_oauth2_refresh_works_with_structured_rt() {
    let vault = Arc::new(MockVault::new());

    // 1. Setup structured refresh token
    vault
        .save_refresh_token(
            "p1",
            Token {
                value: "old_rt".to_string(),
                expires_at: Utc::now() + Duration::days(7),
                created_at: Utc::now(),
            },
        )
        .await
        .unwrap();

    let pool = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));
    let sender = Arc::new(MockHttpSender {});
    let provider = OAuth2Provider::new(pool, sender);

    let config = cowen_common::Config::default_with_profile("p1");

    // 2. Try to refresh. THIS SHOULD NOW SUCCEED.
    let res = provider.refresh("p1", &config, &Default::default()).await;

    assert!(
        res.is_ok(),
        "Refresh should succeed now with structured RT support: {:?}",
        res.err()
    );
    let token = res.unwrap();
    assert_eq!(token.value, "new_at");
}

#[tokio::test]
async fn test_oauth2_on_maintenance_tick_refreshes_expired_token() {
    let vault = Arc::new(MockVault::new());

    // 1. Setup expired access token and valid refresh token (structured)
    vault
        .save_access_token(
            "p1",
            Token {
                value: "expired_at".to_string(),
                expires_at: Utc::now() - Duration::minutes(10), // Expired
                created_at: Utc::now() - Duration::hours(1),
            },
        )
        .await
        .unwrap();

    vault
        .save_refresh_token(
            "p1",
            Token {
                value: "valid_rt".to_string(),
                expires_at: Utc::now() + Duration::days(7),
                created_at: Utc::now() - Duration::hours(1),
            },
        )
        .await
        .unwrap();

    let pool = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));
    let sender = Arc::new(MockHttpSender {});
    let provider = OAuth2Provider::new(pool, sender);

    let config = cowen_common::Config::default_with_profile("p1");

    // 2. Trigger maintenance tick
    provider
        .on_maintenance_tick("p1", &config)
        .await
        .expect("Maintenance tick failed");

    // 3. Check if access token was updated.
    // EXPECTATION: It SHOULD be updated now.
    let token = vault.get_access_token("p1").await.unwrap();
    assert_eq!(
        token.value, "new_at",
        "Token should have been updated to new_at"
    );
}
