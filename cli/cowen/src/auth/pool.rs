use anyhow::Result;
use crate::auth::models::{Token, Ticket};
use crate::core::vault::Vault;
use std::sync::Arc;
use chrono::Utc;

pub trait TokenPool: Send + Sync {
    fn get_app_ticket(&self, profile: &str) -> Result<Ticket>;
    fn set_app_ticket(&self, profile: &str, ticket: &Ticket) -> Result<()>;
    fn get_access_token(&self, profile: &str) -> Result<Token>;
    fn set_access_token(&self, profile: &str, token: &Token) -> Result<()>;
    fn delete_access_token(&self, profile: &str) -> Result<()>;
    fn clear_cache(&self, profile: &str);
    fn lock(&self, profile: &str) -> Result<Box<dyn std::any::Any + Send>>;
}

pub struct VaultTokenPool {
    v: Arc<dyn Vault>,
}

impl VaultTokenPool {
    pub fn new(v: Arc<dyn Vault>) -> Self {
        Self { v }
    }
}

impl TokenPool for VaultTokenPool {
    fn get_app_ticket(&self, profile: &str) -> Result<Ticket> {
        let val = self.v.get(profile, "app_ticket")?;
        // For now, we don't have a reliable receive time in vault, so we use now as proxy
        // Better: store as JSON in vault.
        Ok(Ticket {
            value: val,
            created_at: Utc::now(),
        })
    }

    fn set_app_ticket(&self, profile: &str, ticket: &Ticket) -> Result<()> {
        self.v.set(profile, "app_ticket", &ticket.value)
    }

    fn get_access_token(&self, profile: &str) -> Result<Token> {
        let val = self.v.get(profile, "access_token")?;
        let exp_str = self.v.get(profile, "access_token_expires")?;
        let created_str = self.v.get(profile, "access_token_created")?;
        
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

    fn set_access_token(&self, profile: &str, token: &Token) -> Result<()> {
        self.v.set(profile, "access_token", &token.value)?;
        self.v.set(profile, "access_token_expires", &token.expires_at.to_rfc3339())?;
        self.v.set(profile, "access_token_created", &token.created_at.to_rfc3339())?;
        Ok(())
    }

    fn delete_access_token(&self, profile: &str) -> Result<()> {
        let _ = self.v.delete(profile, "access_token");
        let _ = self.v.delete(profile, "access_token_expires");
        let _ = self.v.delete(profile, "access_token_created");
        Ok(())
    }

    fn clear_cache(&self, _profile: &str) {
        // MultiVault doesn't have an internal cache that needs clearing yet
    }

    fn lock(&self, profile: &str) -> Result<Box<dyn std::any::Any + Send>> {
        let file = self.v.lock(profile)?;
        Ok(Box::new(file))
    }
}
