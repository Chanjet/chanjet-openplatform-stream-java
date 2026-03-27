use crate::core::vault::Vault;
use crate::auth::models::{Token, Ticket};
use anyhow::Result;
use std::sync::RwLock;
use std::collections::HashMap;

pub trait TokenPool: Send + Sync {
    fn get_app_ticket(&self, profile: &str) -> Result<Ticket>;
    fn get_access_token(&self, profile: &str) -> Result<Token>;
    fn set_access_token(&self, profile: &str, token: &Token) -> Result<()>;
}

pub struct VaultTokenPool<'a> {
    v: &'a dyn Vault,
    tickets: RwLock<HashMap<String, Ticket>>,
    tokens: RwLock<HashMap<String, Token>>,
}

impl<'a> VaultTokenPool<'a> {
    pub fn new(v: &'a dyn Vault) -> Self {
        Self {
            v,
            tickets: RwLock::new(HashMap::new()),
            tokens: RwLock::new(HashMap::new()),
        }
    }
}

impl<'a> TokenPool for VaultTokenPool<'a> {
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
}
