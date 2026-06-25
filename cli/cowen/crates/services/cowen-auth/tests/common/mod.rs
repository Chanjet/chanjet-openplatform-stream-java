#![allow(dead_code)] // Test utility module

use async_trait::async_trait;
use cowen_common::domain::*;
use cowen_common::models::{AuthSession, Ticket, Token};
use cowen_common::{CowenError, CowenResult};
use cowen_store::Item;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct MockVault {
    pub store: Option<Arc<dyn cowen_store::Store>>,
    pub tokens: Mutex<HashMap<String, Token>>,
    pub refresh_tokens: Mutex<HashMap<String, Token>>,
    pub configs: Mutex<HashMap<String, String>>,
    pub sessions: Mutex<HashMap<String, AuthSession>>,
}

impl MockVault {
    pub fn new() -> Self {
        Self {
            store: None,
            tokens: Mutex::new(HashMap::new()),
            refresh_tokens: Mutex::new(HashMap::new()),
            configs: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_store(store: Arc<dyn cowen_store::Store>) -> Self {
        Self {
            store: Some(store),
            tokens: Mutex::new(HashMap::new()),
            refresh_tokens: Mutex::new(HashMap::new()),
            configs: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl cowen_common::vault::Vault for MockVault {
    fn primary_store(&self) -> Arc<dyn cowen_store::Store> {
        self.store
            .clone()
            .expect("MockVault was not initialized with a store")
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
        let val = self
            .configs
            .lock()
            .await
            .get(k)
            .cloned()
            .unwrap_or_else(|| "".to_string());
        Ok(val)
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
    async fn get_session(&self, state: &str) -> CowenResult<AuthSession> {
        self.sessions
            .lock()
            .await
            .get(state)
            .cloned()
            .ok_or(CowenError::Auth("not found".to_string()))
    }
    async fn save_session(&self, s: AuthSession) -> CowenResult<()> {
        self.sessions.lock().await.insert(s.state.clone(), s);
        Ok(())
    }
    async fn delete_session(&self, state: &str) -> CowenResult<()> {
        self.sessions.lock().await.remove(state);
        Ok(())
    }
    async fn list_sessions(&self) -> CowenResult<Vec<AuthSession>> {
        Ok(vec![])
    }
}

#[async_trait]
impl SecretDomain for MockVault {
    async fn get_secret(&self, _: &str, _: &str) -> CowenResult<String> {
        Ok("".to_string())
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
    async fn list_dlq_paged(
        &self,
        _: &str,
        _: usize,
        _: usize,
    ) -> CowenResult<Vec<cowen_common::models::DlqMessage>> {
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

pub struct MockHttpSender {
    pub response_body: String,
}

impl MockHttpSender {
    pub fn new() -> Self {
        Self {
            response_body: "{}".to_string(),
        }
    }
    pub fn with_body(body: &str) -> Self {
        Self {
            response_body: body.to_string(),
        }
    }
}

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
            body: self.response_body.clone(),
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
            body: self.response_body.clone(),
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
