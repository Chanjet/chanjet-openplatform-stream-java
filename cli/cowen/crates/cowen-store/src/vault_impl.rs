use cowen_common::{CowenResult, CowenError};
use async_trait::async_trait;
use std::sync::Arc;
use cowen_common::vault::Vault;
use cowen_common::domain::*;
use crate::Store;

pub struct StoreVault {
    primary: Arc<dyn Store>,
    sensitive: Arc<dyn Store>,
}

impl StoreVault {
    pub fn new(primary: Arc<dyn Store>, sensitive: Arc<dyn Store>) -> Self {
        Self { primary, sensitive }
    }
}

#[async_trait]
impl Vault for StoreVault {
    fn primary_store(&self) -> Arc<dyn Store> {
        self.primary.clone()
    }

    async fn migrate(&self) -> CowenResult<()> {
        self.primary.migrate().await?;
        if self.sensitive.name() != self.primary.name() || self.sensitive.description() != self.primary.description() {
            self.sensitive.migrate().await?;
        }
        Ok(())
    }
}

#[async_trait]
impl TicketDomain for StoreVault {
    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<cowen_common::models::Ticket> {
        self.sensitive.get_app_ticket(app_key).await
    }
    async fn save_app_ticket(&self, app_key: &str, ticket: cowen_common::models::Ticket) -> CowenResult<()> {
        self.sensitive.save_app_ticket(app_key, ticket).await
    }
    async fn delete_app_ticket(&self, app_key: &str) -> CowenResult<()> {
        self.sensitive.delete_app_ticket(app_key).await
    }
}

#[async_trait]
impl TokenDomain for StoreVault {
    async fn get_access_token(&self, profile: &str) -> CowenResult<cowen_common::models::Token> {
        self.sensitive.get_access_token(profile).await
    }
    async fn save_access_token(&self, profile: &str, token: cowen_common::models::Token) -> CowenResult<()> {
        self.sensitive.save_access_token(profile, token).await
    }
    async fn delete_access_token(&self, profile: &str) -> CowenResult<()> {
        self.sensitive.delete_access_token(profile).await
    }
    async fn get_refresh_token(&self, profile: &str) -> CowenResult<cowen_common::models::Token> {
        self.sensitive.get_refresh_token(profile).await
    }
    async fn save_refresh_token(&self, profile: &str, token: cowen_common::models::Token) -> CowenResult<()> {
        self.sensitive.save_refresh_token(profile, token).await
    }
    async fn delete_refresh_token(&self, profile: &str) -> CowenResult<()> {
        self.sensitive.delete_refresh_token(profile).await
    }
    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<cowen_common::models::Token> {
        self.sensitive.get_app_access_token(app_key).await
    }
    async fn save_app_access_token(&self, app_key: &str, token: cowen_common::models::Token) -> CowenResult<()> {
        self.sensitive.save_app_access_token(app_key, token).await
    }
    async fn delete_app_access_token(&self, app_key: &str) -> CowenResult<()> {
        self.sensitive.delete_app_access_token(app_key).await
    }
}

#[async_trait]
impl PermanentCodeDomain for StoreVault {
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> CowenResult<String> {
        self.sensitive.get_org_permanent_code(app_key, org_id).await
    }
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> CowenResult<()> {
        self.sensitive.save_org_permanent_code(app_key, org_id, code).await
    }
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> CowenResult<String> {
        self.sensitive.get_user_permanent_code(app_key, org_id, user_id).await
    }
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> CowenResult<()> {
        self.sensitive.save_user_permanent_code(app_key, org_id, user_id, code).await
    }
}

#[async_trait]
impl SessionDomain for StoreVault {
    async fn get_session(&self, state: &str) -> CowenResult<cowen_common::models::AuthSession> {
        let json = self.sensitive.get_token("global", &format!("session:{}", state)).await?;
        serde_json::from_str(&json).map_err(|e| CowenError::Store(e.to_string()))
    }
    async fn save_session(&self, session: cowen_common::models::AuthSession) -> CowenResult<()> {
        let json = serde_json::to_string(&session).map_err(|e| CowenError::Store(e.to_string()))?;
        self.sensitive.set_token("global", &format!("session:{}", session.state), &json, 3600).await
    }
    async fn delete_session(&self, state: &str) -> CowenResult<()> {
        self.sensitive.delete_token("global", &format!("session:{}", state)).await
    }
    async fn list_sessions(&self) -> CowenResult<Vec<cowen_common::models::AuthSession>> {
        let keys = self.sensitive.list_tokens("global").await?;
        let mut sessions = Vec::new();
        for key in keys {
            if key.starts_with("session:") {
                if let Ok(json) = self.sensitive.get_token("global", &key).await {
                    if let Ok(session) = serde_json::from_str(&json) {
                        sessions.push(session);
                    }
                }
            }
        }
        Ok(sessions)
    }
}

#[async_trait]
impl SecretDomain for StoreVault {
    async fn get_secret(&self, profile: &str, key: &str) -> CowenResult<String> {
        self.sensitive.get_secret(profile, key).await
    }
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> {
        self.sensitive.set_secret(profile, key, value).await
    }
    async fn delete_secret(&self, profile: &str, key: &str) -> CowenResult<()> {
        self.sensitive.delete_secret(profile, key).await
    }
}

#[async_trait]
impl ConfigDomain for StoreVault {
    async fn get_config(&self, profile: &str, key: &str) -> CowenResult<String> {
        self.primary.get_config(profile, key).await
    }
    async fn get_config_metadata(&self, profile: &str, key: &str) -> CowenResult<(u64, i64)> {
        self.primary.get_config_metadata(profile, key).await
    }
    async fn get_config_full(&self, profile: &str, key: &str) -> CowenResult<cowen_common::models::Item> {
        self.primary.get_config_full(profile, key).await
    }
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> {
        self.primary.set_config(profile, key, value).await
    }
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> CowenResult<()> {
        self.primary.set_config_conditional(profile, key, value, expected_version).await
    }
    async fn delete_config(&self, profile: &str, key: &str) -> CowenResult<()> {
        self.primary.delete_config(profile, key).await
    }
    async fn list_configs(&self, profile: &str) -> CowenResult<Vec<String>> {
        self.primary.list_configs(profile).await
    }
}

#[async_trait]
impl AuditDomain for StoreVault {
    async fn save_audit(&self, entry: &cowen_common::models::AuditEntry) -> CowenResult<()> {
        self.primary.save_audit(entry).await
    }
    async fn list_audit(&self, profile: &str, limit: usize) -> CowenResult<Vec<cowen_common::models::AuditEntry>> {
        self.primary.list_audit(profile, limit).await
    }
}

#[async_trait]
impl DlqDomain for StoreVault {
    async fn push_dlq(&self, msg: &cowen_common::models::DlqMessage) -> CowenResult<()> {
        self.primary.push_dlq(msg).await
    }
    async fn pop_dlq(&self, profile: &str, topic: &str) -> CowenResult<Option<cowen_common::models::DlqMessage>> {
        self.primary.pop_dlq(profile, topic).await
    }
    async fn list_dlq(&self, profile: &str, limit: usize) -> CowenResult<Vec<cowen_common::models::DlqMessage>> {
        self.primary.list_dlq(profile, limit).await
    }
    async fn list_all_dlq(&self, profile: &str) -> CowenResult<Vec<cowen_common::models::DlqMessage>> {
        self.primary.list_all_dlq(profile).await
    }
    async fn get_dlq_by_id(&self, id: i64) -> CowenResult<Option<cowen_common::models::DlqMessage>> {
        self.primary.get_dlq_by_id(id).await
    }
    async fn list_dlq_paged(&self, profile: &str, offset: usize, limit: usize) -> CowenResult<Vec<cowen_common::models::DlqMessage>> {
        self.primary.list_dlq_paged(profile, offset, limit).await
    }
    async fn delete_dlq_by_id(&self, id: i64) -> CowenResult<()> {
        self.primary.delete_dlq_by_id(id).await
    }
}

#[async_trait]
impl ManagementDomain for StoreVault {
    async fn clear_profile(&self, profile: &str) -> CowenResult<()> {
        self.primary.clear_profile(profile).await?;
        self.sensitive.clear_profile(profile).await
    }
    async fn rename_profile(&self, old: &str, new: &str) -> CowenResult<()> {
        self.primary.rename_profile(old, new).await?;
        self.sensitive.rename_profile(old, new).await
    }
    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> {
        self.primary.list_all_profiles().await
    }
}
