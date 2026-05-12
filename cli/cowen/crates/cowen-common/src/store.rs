use crate::CowenResult;
use async_trait::async_trait;
use crate::models::{Token, Ticket, Item, AuditEntry, DlqMessage};
use std::sync::Arc;

#[async_trait]
pub trait Store: Send + Sync {
    // --- Config Domain ---
    async fn get_config(&self, profile: &str, key: &str) -> CowenResult<String>;
    async fn get_config_metadata(&self, profile: &str, key: &str) -> CowenResult<(u64, i64)>;
    async fn get_config_full(&self, profile: &str, key: &str) -> CowenResult<Item>;
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> CowenResult<()>;
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> CowenResult<()>;
    async fn list_configs(&self, profile: &str) -> CowenResult<Vec<String>>;
    async fn delete_config(&self, profile: &str, key: &str) -> CowenResult<()>;

    // --- Secret Domain ---
    async fn get_secret(&self, profile: &str, key: &str) -> CowenResult<String>;
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> CowenResult<()>;
    async fn delete_secret(&self, profile: &str, key: &str) -> CowenResult<()>;
    async fn list_secrets(&self, profile: &str) -> CowenResult<Vec<String>>;

    // --- Token Domain ---
    async fn get_access_token(&self, profile: &str) -> CowenResult<Token>;
    async fn save_access_token(&self, profile: &str, token: Token) -> CowenResult<()>;
    async fn delete_access_token(&self, profile: &str) -> CowenResult<()>;
    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<Token>;
    async fn save_app_access_token(&self, app_key: &str, token: Token) -> CowenResult<()>;

    // --- Ticket Domain ---
    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<Ticket>;
    async fn save_app_ticket(&self, app_key: &str, ticket: Ticket) -> CowenResult<()>;
    async fn delete_app_ticket(&self, app_key: &str) -> CowenResult<()>;

    // --- Permanent Code Domain ---
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> CowenResult<String>;
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> CowenResult<()>;
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> CowenResult<String>;
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> CowenResult<()>;

    // --- Legacy Support ---
    async fn get_token(&self, profile: &str, key: &str) -> CowenResult<String>;
    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> CowenResult<()>;
    async fn delete_token(&self, profile: &str, key: &str) -> CowenResult<()>;
    async fn list_tokens(&self, profile: &str) -> CowenResult<Vec<String>>;

    // --- Audit Domain ---
    async fn save_audit(&self, entry: &AuditEntry) -> CowenResult<()>;
    async fn list_audit(&self, profile: &str, limit: usize) -> CowenResult<Vec<AuditEntry>>;

    // --- DLQ Domain ---
    async fn push_dlq(&self, msg: &DlqMessage) -> CowenResult<()>;
    async fn pop_dlq(&self, profile: &str, topic: &str) -> CowenResult<Option<DlqMessage>>;
    async fn list_dlq(&self, profile: &str, limit: usize) -> CowenResult<Vec<DlqMessage>>;
    async fn list_all_dlq(&self, profile: &str) -> CowenResult<Vec<DlqMessage>>;

    // --- Management ---
    async fn clear_profile(&self, profile: &str) -> CowenResult<()>;
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> CowenResult<()>;
    async fn list_all_profiles(&self) -> CowenResult<Vec<String>>;
    async fn raw_del(&self, key: &str) -> CowenResult<()>;

    // --- Metadata ---
    fn name(&self) -> &str;
    fn description(&self) -> String;
}

#[async_trait]
pub trait StoreBuilder: Send + Sync {
    fn scheme(&self) -> &str;
    async fn build(&self, url: &str, app_dir: &std::path::Path, fingerprint: &str) -> CowenResult<Arc<dyn Store>>;
}

pub struct StoreBuilderRegistration {
    pub builder: &'static dyn StoreBuilder,
}

inventory::collect!(StoreBuilderRegistration);

#[async_trait]
pub trait CacheBuilder: Send + Sync {
    fn scheme(&self) -> &str;
    async fn build(&self, url: &str, primary: Arc<dyn Store>) -> CowenResult<Arc<dyn Store>>;
}

pub struct CacheBuilderRegistration {
    pub builder: &'static dyn CacheBuilder,
}

inventory::collect!(CacheBuilderRegistration);
