use crate::models::{Ticket, Token};
use async_trait::async_trait;
use cowen_common::vault::Vault;
use cowen_common::CowenResult;
use std::sync::Arc;

#[async_trait]
pub trait TokenPool: Send + Sync {
    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<Ticket>;
    async fn set_app_ticket(&self, app_key: &str, ticket: &Ticket) -> CowenResult<()>;

    async fn get_app_access_token(&self, app_key: &str)
        -> CowenResult<cowen_common::models::Token>;
    async fn set_app_access_token(&self, app_key: &str, token: &Token) -> CowenResult<()>;
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

    async fn get_app_access_token(
        &self,
        app_key: &str,
    ) -> CowenResult<cowen_common::models::Token> {
        self.v.get_app_access_token(app_key).await
    }

    async fn set_app_access_token(&self, app_key: &str, token: &Token) -> CowenResult<()> {
        self.v.save_app_access_token(app_key, token.clone()).await
    }

    async fn delete_app_access_token(&self, app_key: &str) -> CowenResult<()> {
        self.v.delete_app_access_token(app_key).await
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
