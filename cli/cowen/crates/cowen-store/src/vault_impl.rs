use async_trait::async_trait;
use cowen_common::CowenResult;

use std::sync::Arc;
use cowen_common::vault::Vault;
use cowen_common::models::{Token, Ticket, Item, AuditEntry, DlqMessage, AuthSession};
use cowen_common::domain::*;

pub struct StoreVault {
    primary: Arc<dyn cowen_common::store::Store>,
    sensitive: Arc<dyn cowen_common::store::Store>,
}

impl StoreVault {
    pub fn new(primary: Arc<dyn cowen_common::store::Store>, sensitive: Arc<dyn cowen_common::store::Store>) -> Self {
        Self { primary, sensitive }
    }
}

#[async_trait]
impl TicketDomain for StoreVault {
    async fn get_app_ticket(&self, ak: &str) -> CowenResult<Ticket> { self.sensitive.get_app_ticket(ak).await }
    async fn save_app_ticket(&self, ak: &str, t: Ticket) -> CowenResult<()> { self.sensitive.save_app_ticket(ak, t).await }
    async fn delete_app_ticket(&self, ak: &str) -> CowenResult<()> { self.sensitive.delete_app_ticket(ak).await }
}

#[async_trait]
impl TokenDomain for StoreVault {
    async fn get_access_token(&self, p: &str) -> CowenResult<Token> { self.sensitive.get_access_token(p).await }
    async fn save_access_token(&self, p: &str, t: Token) -> CowenResult<()> { self.sensitive.save_access_token(p, t).await }
    async fn delete_access_token(&self, p: &str) -> CowenResult<()> { self.sensitive.delete_access_token(p).await }
    async fn get_refresh_token(&self, p: &str) -> CowenResult<Token> { self.sensitive.get_refresh_token(p).await }
    async fn save_refresh_token(&self, p: &str, t: Token) -> CowenResult<()> { self.sensitive.save_refresh_token(p, t).await }
    async fn delete_refresh_token(&self, p: &str) -> CowenResult<()> { self.sensitive.delete_refresh_token(p).await }
    async fn get_app_access_token(&self, ak: &str) -> CowenResult<Token> { self.sensitive.get_app_access_token(ak).await }
    async fn save_app_access_token(&self, ak: &str, t: Token) -> CowenResult<()> { self.sensitive.save_app_access_token(ak, t).await }
    async fn delete_app_access_token(&self, ak: &str) -> CowenResult<()> { self.sensitive.delete_app_access_token(ak).await }
}

#[async_trait]
impl PermanentCodeDomain for StoreVault {
    async fn get_org_permanent_code(&self, ak: &str, oid: &str) -> CowenResult<String> { self.sensitive.get_org_permanent_code(ak, oid).await }
    async fn save_org_permanent_code(&self, ak: &str, oid: &str, c: &str) -> CowenResult<()> { self.sensitive.save_org_permanent_code(ak, oid, c).await }
    async fn get_user_permanent_code(&self, ak: &str, oid: &str, uid: &str) -> CowenResult<String> { self.sensitive.get_user_permanent_code(ak, oid, uid).await }
    async fn save_user_permanent_code(&self, ak: &str, oid: &str, uid: &str, c: &str) -> CowenResult<()> { self.sensitive.save_user_permanent_code(ak, oid, uid, c).await }
}

#[async_trait]
impl SessionDomain for StoreVault {
    async fn save_session(&self, session: AuthSession) -> CowenResult<()> {
        let json = serde_json::to_string(&session)?;
        self.sensitive.set_token("global", &format!("session:{}", session.state), &json, 3600).await
    }
    async fn get_session(&self, state: &str) -> CowenResult<AuthSession> {
        let json = self.sensitive.get_token("global", &format!("session:{}", state)).await?;
        Ok(serde_json::from_str(&json)?)
    }
    async fn delete_session(&self, state: &str) -> CowenResult<()> {
        self.sensitive.delete_token("global", &format!("session:{}", state)).await
    }
}

#[async_trait]
impl SecretDomain for StoreVault {
    async fn get_secret(&self, p: &str, k: &str) -> CowenResult<String> { self.sensitive.get_secret(p, k).await }
    async fn set_secret(&self, p: &str, k: &str, v: &str) -> CowenResult<()> { self.sensitive.set_secret(p, k, v).await }
    async fn delete_secret(&self, p: &str, k: &str) -> CowenResult<()> { self.sensitive.delete_secret(p, k).await }
}

#[async_trait]
impl ConfigDomain for StoreVault {
    async fn get_config(&self, p: &str, k: &str) -> CowenResult<String> { self.primary.get_config(p, k).await }
    async fn get_config_metadata(&self, p: &str, k: &str) -> CowenResult<(u64, i64)> { self.primary.get_config_metadata(p, k).await }
    async fn get_config_full(&self, p: &str, k: &str) -> CowenResult<Item> { self.primary.get_config_full(p, k).await }
    async fn set_config(&self, p: &str, k: &str, v: &str) -> CowenResult<()> { self.primary.set_config(p, k, v).await }
    async fn set_config_conditional(&self, p: &str, k: &str, v: &str, ev: u64) -> CowenResult<()> { self.primary.set_config_conditional(p, k, v, ev).await }
    async fn list_configs(&self, p: &str) -> CowenResult<Vec<String>> { self.primary.list_configs(p).await }
    async fn delete_config(&self, p: &str, k: &str) -> CowenResult<()> { self.primary.delete_config(p, k).await }
}

#[async_trait]
impl AuditDomain for StoreVault {
    async fn save_audit(&self, e: &AuditEntry) -> CowenResult<()> { self.primary.save_audit(e).await }
    async fn list_audit(&self, p: &str, l: usize) -> CowenResult<Vec<AuditEntry>> { self.primary.list_audit(p, l).await }
}

#[async_trait]
impl DlqDomain for StoreVault {
    async fn push_dlq(&self, m: &DlqMessage) -> CowenResult<()> { self.primary.push_dlq(m).await }
    async fn pop_dlq(&self, p: &str, t: &str) -> CowenResult<Option<DlqMessage>> { self.primary.pop_dlq(p, t).await }
    async fn list_dlq(&self, p: &str, l: usize) -> CowenResult<Vec<DlqMessage>> { self.primary.list_dlq(p, l).await }
    async fn list_all_dlq(&self, p: &str) -> CowenResult<Vec<DlqMessage>> { self.primary.list_all_dlq(p).await }
}

#[async_trait]
impl ManagementDomain for StoreVault {
    async fn clear_profile(&self, p: &str) -> CowenResult<()> {
        let _ = self.sensitive.clear_profile(p).await;
        if !Arc::ptr_eq(&self.sensitive, &self.primary) {
            let _ = self.primary.clear_profile(p).await;
        }
        Ok(())
    }
    async fn rename_profile(&self, old: &str, new: &str) -> CowenResult<()> { self.primary.rename_profile(old, new).await }
    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> { self.primary.list_all_profiles().await }
}

#[async_trait]
impl Vault for StoreVault {
    fn primary_store(&self) -> Arc<dyn cowen_common::store::Store> { self.primary.clone() }
}
