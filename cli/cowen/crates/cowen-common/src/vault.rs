use crate::{CowenResult, CowenError};
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait Vault: Send + Sync {
    async fn get_config(&self, profile: &str, key: &str) -> CowenResult<String>;
    async fn get_config_metadata(&self, profile: &str, key: &str) -> CowenResult<(u64, i64)>;
    async fn get_config_full(&self, profile: &str, key: &str) -> CowenResult<crate::models::Item>;
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> CowenResult<()>;
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> CowenResult<()>;
    async fn list_configs(&self, profile: &str) -> CowenResult<Vec<String>>;
    async fn delete_config(&self, profile: &str, key: &str) -> CowenResult<()>;

    async fn get_secret(&self, profile: &str, key: &str) -> CowenResult<String>;
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> CowenResult<()>;
    async fn delete_secret(&self, profile: &str, key: &str) -> CowenResult<()>;

    async fn get_access_token(&self, profile: &str) -> CowenResult<crate::models::Token>;
    async fn save_access_token(&self, profile: &str, token: crate::models::Token) -> CowenResult<()>;
    async fn delete_access_token(&self, profile: &str) -> CowenResult<()>;
    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<crate::models::Token>;
    async fn save_app_access_token(&self, app_key: &str, token: crate::models::Token) -> CowenResult<()>;

    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<crate::models::Ticket>;
    async fn save_app_ticket(&self, app_key: &str, ticket: crate::models::Ticket) -> CowenResult<()>;
    async fn delete_app_ticket(&self, app_key: &str) -> CowenResult<()>;

    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> CowenResult<String>;
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> CowenResult<()>;
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> CowenResult<String>;
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> CowenResult<()>;

    async fn save_audit(&self, entry: &crate::models::AuditEntry) -> CowenResult<()>;
    async fn list_audit(&self, profile: &str, limit: usize) -> CowenResult<Vec<crate::models::AuditEntry>>;

    async fn push_dlq(&self, msg: &crate::models::DlqMessage) -> CowenResult<()>;
    async fn pop_dlq(&self, profile: &str, topic: &str) -> CowenResult<Option<crate::models::DlqMessage>>;
    async fn list_dlq(&self, profile: &str, limit: usize) -> CowenResult<Vec<crate::models::DlqMessage>>;
    async fn list_all_dlq(&self, profile: &str) -> CowenResult<Vec<crate::models::DlqMessage>>;

    async fn clear_profile(&self, profile: &str) -> CowenResult<()>;
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> CowenResult<()>;
    async fn list_all_profiles(&self) -> CowenResult<Vec<String>>;

    async fn save_session(&self, session: crate::models::AuthSession) -> CowenResult<()>;
    async fn get_session(&self, state: &str) -> CowenResult<crate::models::AuthSession>;
    async fn delete_session(&self, state: &str) -> CowenResult<()>;

    fn primary_store(&self) -> Arc<dyn crate::store::Store>;
}
