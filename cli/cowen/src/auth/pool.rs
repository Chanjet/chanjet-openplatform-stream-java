use anyhow::Result;
use crate::auth::models::{Token, Ticket};
use crate::core::vault::Vault;
use std::sync::Arc;
use chrono::Utc;

use async_trait::async_trait;

#[async_trait]
pub trait TokenPool: Send + Sync {
    async fn get_app_ticket(&self, app_key: &str) -> Result<Ticket>;
    async fn set_app_ticket(&self, app_key: &str, ticket: &Ticket) -> Result<()>;
    
    async fn get_app_access_token(&self, app_key: &str) -> Result<Token>;
    async fn set_app_access_token(&self, app_key: &str, token: &Token) -> Result<()>;
    async fn delete_app_access_token(&self, app_key: &str) -> Result<()>;

    async fn get_access_token(&self, profile: &str) -> Result<Token>;
    async fn set_access_token(&self, profile: &str, token: &Token) -> Result<()>;
    async fn delete_access_token(&self, profile: &str) -> Result<()>;
    
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
    async fn get_app_ticket(&self, app_key: &str) -> Result<Ticket> {
        let global_profile = format!("app:{}", app_key);
        let val = self.v.get(&global_profile, "app_ticket").await?;
        let created_at = if let Ok(ts_str) = self.v.get(&global_profile, "app_ticket_created").await {
            chrono::DateTime::parse_from_rfc3339(&ts_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or(Utc::now())
        } else {
            Utc::now()
        };

        Ok(Ticket {
            value: val,
            created_at,
        })
    }

    async fn set_app_ticket(&self, app_key: &str, ticket: &Ticket) -> Result<()> {
        let global_profile = format!("app:{}", app_key);
        self.v.set(&global_profile, "app_ticket", &ticket.value).await?;
        self.v.set(&global_profile, "app_ticket_created", &ticket.created_at.to_rfc3339()).await
    }

    async fn get_app_access_token(&self, app_key: &str) -> Result<Token> {
        let global_profile = format!("app:{}", app_key);
        let val = self.v.get(&global_profile, "access_token").await?;
        let exp_str = self.v.get(&global_profile, "access_token_expires").await?;
        let created_str = self.v.get(&global_profile, "access_token_created").await?;
        
        let expires_at = chrono::DateTime::parse_from_rfc3339(&exp_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| anyhow::anyhow!("Invalid expiry date: {}", e))?;
            
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| anyhow::anyhow!("Invalid created date: {}", e))?;

        Ok(Token {
            value: val,
            expires_at,
            created_at,
        })
    }

    async fn set_app_access_token(&self, app_key: &str, token: &Token) -> Result<()> {
        let global_profile = format!("app:{}", app_key);
        self.v.set(&global_profile, "access_token", &token.value).await?;
        self.v.set(&global_profile, "access_token_expires", &token.expires_at.to_rfc3339()).await?;
        self.v.set(&global_profile, "access_token_created", &token.created_at.to_rfc3339()).await?;
        Ok(())
    }

    async fn delete_app_access_token(&self, app_key: &str) -> Result<()> {
        let global_profile = format!("app:{}", app_key);
        let _ = self.v.delete(&global_profile, "access_token").await;
        let _ = self.v.delete(&global_profile, "access_token_expires").await;
        let _ = self.v.delete(&global_profile, "access_token_created").await;
        Ok(())
    }

    async fn get_access_token(&self, profile: &str) -> Result<Token> {
        let val = self.v.get(profile, "access_token").await?;
        let exp_str = self.v.get(profile, "access_token_expires").await?;
        let created_str = self.v.get(profile, "access_token_created").await?;
        
        let expires_at = chrono::DateTime::parse_from_rfc3339(&exp_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| anyhow::anyhow!("Invalid expiry date: {}", e))?;
            
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| anyhow::anyhow!("Invalid created date: {}", e))?;

        Ok(Token {
            value: val,
            expires_at,
            created_at,
        })
    }

    async fn set_access_token(&self, profile: &str, token: &Token) -> Result<()> {
        self.v.set(profile, "access_token", &token.value).await?;
        self.v.set(profile, "access_token_expires", &token.expires_at.to_rfc3339()).await?;
        self.v.set(profile, "access_token_created", &token.created_at.to_rfc3339()).await?;
        Ok(())
    }

    async fn delete_access_token(&self, profile: &str) -> Result<()> {
        let _ = self.v.delete(profile, "access_token").await;
        let _ = self.v.delete(profile, "access_token_expires").await;
        let _ = self.v.delete(profile, "access_token_created").await;
        Ok(())
    }

    fn clear_cache(&self, _profile: &str) {
        // MultiVault doesn't have an internal cache that needs clearing yet
    }

    fn as_vault(&self) -> Arc<dyn Vault> {
        self.v.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::vault::StoreVault;
    use crate::core::store::file::FileStore;
    use tempfile::tempdir;
    use chrono::{Duration, Utc, SubsecRound};

    #[tokio::test]
    async fn test_vault_token_pool_lifecycle() {
        let dir = tempdir().unwrap();
        let vault_path = dir.path().join("test.vault");
        let store = Arc::new(FileStore::new(vault_path, "fingerprint").unwrap());
        let vault = Arc::new(StoreVault::new(store.clone(), store.clone()));
        let pool = VaultTokenPool::new(vault);
        let profile = "test-profile";

        // 1. Ticket (App level)
        let app_key = "test_app_key";
        let ticket = Ticket {
            value: "ticket-123".to_string(),
            created_at: Utc::now(),
        };
        pool.set_app_ticket(app_key, &ticket).await.unwrap();
        let retrieved_ticket = pool.get_app_ticket(app_key).await.unwrap();
        assert_eq!(retrieved_ticket.value, "ticket-123");

        // 2. Token (User level)
        let now = Utc::now().round_subsecs(0); // RFC3339 might lose some precision depending on implementation
        let token = Token {
            value: "token-abc".to_string(),
            expires_at: now + Duration::hours(2),
            created_at: now,
        };
        pool.set_access_token(profile, &token).await.unwrap();
        
        let retrieved_token = pool.get_access_token(profile).await.unwrap();
        assert_eq!(retrieved_token.value, "token-abc");
        assert_eq!(retrieved_token.expires_at.to_rfc3339(), token.expires_at.to_rfc3339());
        
        // 3. Delete (User level)
        pool.delete_access_token(profile).await.unwrap();
        assert!(pool.get_access_token(profile).await.is_err());
        
        // 4. Token (App level)
        pool.set_app_access_token(app_key, &token).await.unwrap();
        let retrieved_app_token = pool.get_app_access_token(app_key).await.unwrap();
        assert_eq!(retrieved_app_token.value, "token-abc");
        pool.delete_app_access_token(app_key).await.unwrap();
        assert!(pool.get_app_access_token(app_key).await.is_err());
    }
}
