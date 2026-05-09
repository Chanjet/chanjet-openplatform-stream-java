use anyhow::Result;
use async_trait::async_trait;
use crate::models::{Token, Ticket, Item, AuditEntry, DlqMessage};
use std::sync::Arc;

#[async_trait]
pub trait Store: Send + Sync {
    // --- Config Domain ---
    async fn get_config(&self, profile: &str, key: &str) -> Result<String>;
    async fn get_config_metadata(&self, profile: &str, key: &str) -> Result<(u64, i64)>;
    async fn get_config_full(&self, profile: &str, key: &str) -> Result<Item>;
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> Result<()>;
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>>;
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()>;

    // --- Secret Domain ---
    async fn get_secret(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    async fn delete_secret(&self, profile: &str, key: &str) -> Result<()>;
    async fn list_secrets(&self, profile: &str) -> Result<Vec<String>>;

    // --- Token Domain ---
    async fn get_access_token(&self, profile: &str) -> Result<Token>;
    async fn save_access_token(&self, profile: &str, token: Token) -> Result<()>;
    async fn delete_access_token(&self, profile: &str) -> Result<()>;
    async fn get_app_access_token(&self, app_key: &str) -> Result<Token>;
    async fn save_app_access_token(&self, app_key: &str, token: Token) -> Result<()>;

    // --- Ticket Domain ---
    async fn get_app_ticket(&self, app_key: &str) -> Result<Ticket>;
    async fn save_app_ticket(&self, app_key: &str, ticket: Ticket) -> Result<()>;
    async fn delete_app_ticket(&self, app_key: &str) -> Result<()>;

    // --- Permanent Code Domain ---
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> Result<String>;
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> Result<()>;
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> Result<String>;
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> Result<()>;

    // --- Legacy Support ---
    async fn get_token(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> Result<()>;
    async fn delete_token(&self, profile: &str, key: &str) -> Result<()>;
    async fn list_tokens(&self, profile: &str) -> Result<Vec<String>>;

    // --- Audit Domain ---
    async fn save_audit(&self, entry: &AuditEntry) -> Result<()>;
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<AuditEntry>>;

    // --- DLQ Domain ---
    async fn push_dlq(&self, msg: &DlqMessage) -> Result<()>;
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<DlqMessage>>;
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<DlqMessage>>;
    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<DlqMessage>>;

    // --- Management ---
    async fn clear_profile(&self, profile: &str) -> Result<()>;
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()>;
    async fn list_all_profiles(&self) -> Result<Vec<String>>;
    async fn raw_del(&self, key: &str) -> Result<()>;
}

#[async_trait]
pub trait StoreBuilder: Send + Sync {
    fn scheme(&self) -> &str;
    async fn build(&self, url: &str, app_dir: &std::path::Path, fingerprint: &str) -> Result<Arc<dyn Store>>;
}

pub struct StoreBuilderRegistration {
    pub builder: &'static dyn StoreBuilder,
}

inventory::collect!(StoreBuilderRegistration);

#[async_trait]
pub trait CacheBuilder: Send + Sync {
    fn scheme(&self) -> &str;
    async fn build(&self, url: &str, primary: Arc<dyn Store>) -> Result<Arc<dyn Store>>;
}

pub struct CacheBuilderRegistration {
    pub builder: &'static dyn CacheBuilder,
}

inventory::collect!(CacheBuilderRegistration);
