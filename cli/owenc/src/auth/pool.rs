use crate::core::vault::Vault;
use crate::auth::models::{Token, Ticket};
use anyhow::Result;
use std::sync::RwLock;
use std::collections::HashMap;

use std::sync::Arc;

pub trait TokenPool: Send + Sync {
    fn get_app_ticket(&self, profile: &str) -> Result<Ticket>;
    fn set_app_ticket(&self, profile: &str, ticket: &Ticket) -> Result<()>;
    fn get_access_token(&self, profile: &str) -> Result<Token>;
    fn set_access_token(&self, profile: &str, token: &Token) -> Result<()>;
    fn delete_access_token(&self, profile: &str) -> Result<()>;
    #[allow(dead_code)]
    fn delete_app_ticket(&self, profile: &str) -> Result<()>;
    
    /// Acquire a global lock for this profile (multi-process)
    fn lock(&self, profile: &str) -> Result<Box<dyn std::any::Any + Send>>;

    /// Helper for downcasting
    #[allow(dead_code)]
    fn as_any(&self) -> &dyn std::any::Any;
}

pub struct VaultTokenPool {
    v: Arc<dyn Vault>,
    tickets: RwLock<HashMap<String, Ticket>>,
    tokens: RwLock<HashMap<String, Token>>,
}

impl VaultTokenPool {
    pub fn new(v: Arc<dyn Vault>) -> Self {
        Self {
            v,
            tickets: RwLock::new(HashMap::new()),
            tokens: RwLock::new(HashMap::new()),
        }
    }

    #[allow(dead_code)]
    pub fn clear_cache(&self, profile: &str) {
        let mut tickets = self.tickets.write().unwrap();
        tickets.remove(profile);
        let mut tokens = self.tokens.write().unwrap();
        tokens.remove(profile);
    }
}

impl TokenPool for VaultTokenPool {
    fn get_app_ticket(&self, profile: &str) -> Result<Ticket> {
        let tickets = self.tickets.read().unwrap();
        if let Some(t) = tickets.get(profile) {
            return Ok(t.clone());
        }
        drop(tickets);

        let val = self.v.get(profile, "app_ticket")?;
        let t: Ticket = serde_json::from_str(&val)?;

        let mut tickets = self.tickets.write().unwrap();
        tickets.insert(profile.to_string(), t.clone());
        Ok(t)
    }

    fn set_app_ticket(&self, profile: &str, ticket: &Ticket) -> Result<()> {
        let raw = serde_json::to_string(ticket)?;
        self.v.set(profile, "app_ticket", &raw)?;

        let mut tickets = self.tickets.write().unwrap();
        tickets.insert(profile.to_string(), ticket.clone());
        Ok(())
    }

    fn get_access_token(&self, profile: &str) -> Result<Token> {
        let tokens = self.tokens.read().unwrap();
        if let Some(t) = tokens.get(profile) {
            return Ok(t.clone());
        }
        drop(tokens);

        let val = self.v.get(profile, "access_token")?;
        let t: Token = serde_json::from_str(&val)?;

        let mut tokens = self.tokens.write().unwrap();
        tokens.insert(profile.to_string(), t.clone());
        Ok(t)
    }

    fn set_access_token(&self, profile: &str, token: &Token) -> Result<()> {
        let raw = serde_json::to_string(token)?;
        self.v.set(profile, "access_token", &raw)?;

        let mut tokens = self.tokens.write().unwrap();
        tokens.insert(profile.to_string(), token.clone());
        Ok(())
    }

    fn delete_access_token(&self, profile: &str) -> Result<()> {
        let mut tokens = self.tokens.write().unwrap();
        tokens.remove(profile);
        self.v.delete(profile, "access_token")
    }

    fn delete_app_ticket(&self, profile: &str) -> Result<()> {
        let mut tickets = self.tickets.write().unwrap();
        tickets.remove(profile);
        self.v.delete(profile, "app_ticket")
    }

    fn lock(&self, profile: &str) -> Result<Box<dyn std::any::Any + Send>> {
        self.v.lock(profile)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
