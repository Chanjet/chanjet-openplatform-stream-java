use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use cowen_common::vault::Vault;
use cowen_common::models::{Token, Ticket, Item, AuditEntry, DlqMessage, AuthSession};


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
impl Vault for StoreVault {
    async fn get_config(&self, p: &str, k: &str) -> Result<String> { self.primary.get_config(p, k).await }
    async fn get_config_metadata(&self, p: &str, k: &str) -> Result<(u64, i64)> { self.primary.get_config_metadata(p, k).await }
    async fn get_config_full(&self, p: &str, k: &str) -> Result<Item> { self.primary.get_config_full(p, k).await }
    async fn set_config(&self, p: &str, k: &str, v: &str) -> Result<()> { self.primary.set_config(p, k, v).await }
    async fn set_config_conditional(&self, p: &str, k: &str, v: &str, ev: u64) -> Result<()> { self.primary.set_config_conditional(p, k, v, ev).await }
    async fn list_configs(&self, p: &str) -> Result<Vec<String>> { self.primary.list_configs(p).await }
    async fn delete_config(&self, p: &str, k: &str) -> Result<()> { self.primary.delete_config(p, k).await }

    async fn get_secret(&self, p: &str, k: &str) -> Result<String> { self.sensitive.get_secret(p, k).await }
    async fn set_secret(&self, p: &str, k: &str, v: &str) -> Result<()> { self.sensitive.set_secret(p, k, v).await }
    async fn delete_secret(&self, p: &str, k: &str) -> Result<()> { self.sensitive.delete_secret(p, k).await }

    async fn get_access_token(&self, p: &str) -> Result<Token> { self.sensitive.get_access_token(p).await }
    async fn save_access_token(&self, p: &str, t: Token) -> Result<()> { self.sensitive.save_access_token(p, t).await }
    async fn delete_access_token(&self, p: &str) -> Result<()> { self.sensitive.delete_access_token(p).await }
    
    async fn get_app_access_token(&self, ak: &str) -> Result<Token> { self.sensitive.get_app_access_token(ak).await }
    async fn save_app_access_token(&self, ak: &str, t: Token) -> Result<()> { self.sensitive.save_app_access_token(ak, t).await }

    async fn get_app_ticket(&self, ak: &str) -> Result<Ticket> { self.sensitive.get_app_ticket(ak).await }
    async fn save_app_ticket(&self, ak: &str, t: Ticket) -> Result<()> { self.sensitive.save_app_ticket(ak, t).await }
    async fn delete_app_ticket(&self, ak: &str) -> Result<()> { self.sensitive.delete_app_ticket(ak).await }

    async fn get_org_permanent_code(&self, ak: &str, oid: &str) -> Result<String> { self.sensitive.get_org_permanent_code(ak, oid).await }
    async fn save_org_permanent_code(&self, ak: &str, oid: &str, c: &str) -> Result<()> { self.sensitive.save_org_permanent_code(ak, oid, c).await }
    async fn get_user_permanent_code(&self, ak: &str, oid: &str, uid: &str) -> Result<String> { self.sensitive.get_user_permanent_code(ak, oid, uid).await }
    async fn save_user_permanent_code(&self, ak: &str, oid: &str, uid: &str, c: &str) -> Result<()> { self.sensitive.save_user_permanent_code(ak, oid, uid, c).await }

    async fn save_audit(&self, e: &AuditEntry) -> Result<()> { self.primary.save_audit(e).await }
    async fn list_audit(&self, p: &str, l: usize) -> Result<Vec<AuditEntry>> { self.primary.list_audit(p, l).await }

    async fn push_dlq(&self, m: &DlqMessage) -> Result<()> { self.primary.push_dlq(m).await }
    async fn pop_dlq(&self, p: &str, t: &str) -> Result<Option<DlqMessage>> { self.primary.pop_dlq(p, t).await }
    async fn list_dlq(&self, p: &str, l: usize) -> Result<Vec<DlqMessage>> { self.primary.list_dlq(p, l).await }
    async fn list_all_dlq(&self, p: &str) -> Result<Vec<DlqMessage>> { self.primary.list_all_dlq(p).await }

    async fn rename_profile(&self, old: &str, new: &str) -> Result<()> { self.primary.rename_profile(old, new).await }

    async fn clear_profile(&self, p: &str) -> Result<()> {
        let _ = self.sensitive.clear_profile(p).await;
        if !Arc::ptr_eq(&self.sensitive, &self.primary) {
            let _ = self.primary.clear_profile(p).await;
        }
        Ok(())
    }
    fn primary_store(&self) -> Arc<dyn cowen_common::store::Store> { self.primary.clone() }
    async fn list_all_profiles(&self) -> Result<Vec<String>> { self.primary.list_all_profiles().await }

    async fn save_session(&self, session: AuthSession) -> Result<()> {
        let json = serde_json::to_string(&session)?;
        self.sensitive.set_token("global", &format!("session:{}", session.state), &json, 3600).await
    }

    async fn get_session(&self, state: &str) -> Result<AuthSession> {
        let json = self.sensitive.get_token("global", &format!("session:{}", state)).await?;
        Ok(serde_json::from_str(&json)?)
    }

    async fn delete_session(&self, state: &str) -> Result<()> {
        self.sensitive.delete_token("global", &format!("session:{}", state)).await
    }
}
