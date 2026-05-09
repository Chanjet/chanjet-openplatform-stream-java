use cowen_common::{CowenResult, CowenError};
use async_trait::async_trait;
use crate::models::{Ticket, Token};
use cowen_common::vault::Vault;
use std::sync::Arc;



#[async_trait]
pub trait TokenPool: Send + Sync {
    #[allow(dead_code)]
    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<Ticket>;
    async fn set_app_ticket(&self, app_key: &str, ticket: &Ticket) -> CowenResult<()>;

    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<cowen_common::models::Token>;
    async fn set_app_access_token(&self, app_key: &str, token: &Token) -> CowenResult<()>;
    #[allow(dead_code)]
    async fn delete_app_access_token(&self, app_key: &str) -> CowenResult<()>;

    async fn get_access_token(&self, profile: &str) -> CowenResult<cowen_common::models::Token>;
    async fn set_access_token(&self, profile: &str, token: &Token) -> CowenResult<()>;
    async fn delete_access_token(&self, profile: &str) -> CowenResult<()>;

    fn clear_cache(&self, profile: &str);
    fn as_vault(&self) -> Arc<dyn Vault>;
}

pub struct VaultTokenPool {
    v: Arc<dyn Vault>,
}

impl VaultTokenPool {
    pub fn new(v: Arc<dyn Vault>) -> Self {
        Self { v }
    }
}

#[async_trait]
impl TokenPool for VaultTokenPool {
    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<Ticket> {
        self.v.get_app_ticket(app_key).await
    }

    async fn set_app_ticket(&self, app_key: &str, ticket: &Ticket) -> CowenResult<()> {
        self.v.save_app_ticket(app_key, ticket.clone()).await
    }

    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<cowen_common::models::Token> {
        self.v.get_app_access_token(app_key).await
    }

    async fn set_app_access_token(&self, app_key: &str, token: &Token) -> CowenResult<()> {
        self.v.save_app_access_token(app_key, token.clone()).await
    }

    async fn delete_app_access_token(&self, app_key: &str) -> CowenResult<()> {
        // TokenDomain currently doesn't have delete_app_access_token, we can use the generic delete if needed 
        // or add it to the trait. For now, we'll use delete_access_token with app profile.
        let profile = format!("app:{}", app_key);
        self.v.delete_access_token(&profile).await
    }

    async fn get_access_token(&self, profile: &str) -> CowenResult<cowen_common::models::Token> {
        self.v.get_access_token(profile).await
    }

    async fn set_access_token(&self, profile: &str, token: &Token) -> CowenResult<()> {
        self.v.save_access_token(profile, token.clone()).await
    }

    async fn delete_access_token(&self, profile: &str) -> CowenResult<()> {
        self.v.delete_access_token(profile).await
    }

    fn clear_cache(&self, _profile: &str) {}

    fn as_vault(&self) -> Arc<dyn Vault> {
        self.v.clone()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use cowen_store::file::FileStore;
    use cowen_common::vault::StoreVault;
    use chrono::{Duration, SubsecRound, Utc};
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_vault_token_pool_lifecycle() {
        let dir = tempdir().unwrap();
        let vault_path = dir.path().join("test.vault");
        let store = Arc::new(FileStore::new(vault_path, "fingerprint").unwrap());
        let vault = Arc::new(StoreVault::new(store.clone(), store.clone()));
        let pool = VaultTokenPool::new(vault);
        let profile = "test-profile";

        // 1. Ticket
        let app_key = "test_app_key";
        let ticket = Ticket {
            value: "ticket-123".to_string(),
            created_at: Utc::now(),
        };
        pool.set_app_ticket(app_key, &ticket).await.unwrap();
        let retrieved_ticket = pool.get_app_ticket(app_key).await.unwrap();
        assert_eq!(retrieved_ticket.value, "ticket-123");

        // 2. Token
        let now = Utc::now().round_subsecs(0);
        let token = Token {
            value: "token-abc".to_string(),
            expires_at: now + Duration::hours(2),
            created_at: now,
        };
        pool.set_access_token(profile, &token).await.unwrap();

        let retrieved_token = pool.get_access_token(profile).await.unwrap();
        assert_eq!(retrieved_token.value, "token-abc");
        assert_eq!(
            retrieved_token.expires_at.to_rfc3339(),
            token.expires_at.to_rfc3339()
        );

        // 3. Delete
        pool.delete_access_token(profile).await.unwrap();
        assert!(pool.get_access_token(profile).await.is_err());
    }
}
