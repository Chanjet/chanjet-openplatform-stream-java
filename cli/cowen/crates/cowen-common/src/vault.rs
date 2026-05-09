use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait Vault: Send + Sync {
    async fn get_config(&self, profile: &str, key: &str) -> Result<String>;
    async fn get_config_metadata(&self, profile: &str, key: &str) -> Result<(u64, i64)>;
    async fn get_config_full(&self, profile: &str, key: &str) -> Result<crate::models::Item>;
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> Result<()>;
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>>;
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()>;

    async fn get_secret(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    async fn delete_secret(&self, profile: &str, key: &str) -> Result<()>;

    async fn get_access_token(&self, profile: &str) -> Result<crate::models::Token>;
    async fn save_access_token(&self, profile: &str, token: crate::models::Token) -> Result<()>;
    async fn delete_access_token(&self, profile: &str) -> Result<()>;
    async fn get_app_access_token(&self, app_key: &str) -> Result<crate::models::Token>;
    async fn save_app_access_token(&self, app_key: &str, token: crate::models::Token) -> Result<()>;

    async fn get_app_ticket(&self, app_key: &str) -> Result<crate::models::Ticket>;
    async fn save_app_ticket(&self, app_key: &str, ticket: crate::models::Ticket) -> Result<()>;
    async fn delete_app_ticket(&self, app_key: &str) -> Result<()>;

    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> Result<String>;
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> Result<()>;
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> Result<String>;
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> Result<()>;

    async fn save_audit(&self, entry: &crate::models::AuditEntry) -> Result<()>;
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<crate::models::AuditEntry>>;

    async fn push_dlq(&self, msg: &crate::models::DlqMessage) -> Result<()>;
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<crate::models::DlqMessage>>;
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<crate::models::DlqMessage>>;
    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<crate::models::DlqMessage>>;

    async fn clear_profile(&self, profile: &str) -> Result<()>;
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()>;
    async fn list_all_profiles(&self) -> Result<Vec<String>>;

    async fn save_session(&self, session: crate::models::AuthSession) -> Result<()>;
    async fn get_session(&self, state: &str) -> Result<crate::models::AuthSession>;
    async fn delete_session(&self, state: &str) -> Result<()>;

    fn primary_store(&self) -> Arc<dyn crate::store::Store>;
}
