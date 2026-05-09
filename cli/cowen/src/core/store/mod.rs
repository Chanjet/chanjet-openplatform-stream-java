use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::path::Path;

pub mod file;
pub mod hybrid;
pub mod redis_store;
pub mod sql;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub profile: String,
    pub key: String,
    #[allow(dead_code)]
    pub value: String,
    pub version: u64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub profile: String,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DlqMessage {
    pub id: Option<i64>,
    pub profile: String,
    pub topic: String,
    pub payload: String,
    pub retry_count: i32,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

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

    // --- Secret Domain (Sensitive/Persistent) ---
    async fn get_secret(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    async fn delete_secret(&self, profile: &str, key: &str) -> Result<()>;
    #[allow(dead_code)]
    async fn list_secrets(&self, profile: &str) -> Result<Vec<String>>;

    // --- Token Domain (Ephemeral/Auto-expiring) ---
    async fn get_access_token(&self, profile: &str) -> Result<crate::auth::models::Token>;
    async fn save_access_token(&self, profile: &str, token: crate::auth::models::Token) -> Result<()>;
    async fn delete_access_token(&self, profile: &str) -> Result<()>;
    async fn get_app_access_token(&self, app_key: &str) -> Result<crate::auth::models::Token>;
    async fn save_app_access_token(&self, app_key: &str, token: crate::auth::models::Token) -> Result<()>;

    // --- Ticket Domain ---
    async fn get_app_ticket(&self, app_key: &str) -> Result<crate::auth::models::Ticket>;
    async fn save_app_ticket(&self, app_key: &str, ticket: crate::auth::models::Ticket) -> Result<()>;
    async fn delete_app_ticket(&self, app_key: &str) -> Result<()>;

    // --- Permanent Code Domain (Authority) ---
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> Result<String>;
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> Result<()>;
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> Result<String>;
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> Result<()>;

    // --- Legacy Support ---
    async fn get_token(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> Result<()>;
    async fn delete_token(&self, profile: &str, key: &str) -> Result<()>;
    async fn list_tokens(&self, profile: &str) -> Result<Vec<String>>;

    // --- Audit Domain (Structural Logs) ---
    async fn save_audit(&self, entry: &AuditEntry) -> Result<()>;
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<AuditEntry>>;

    // --- DLQ Domain (Queue-like) ---
    async fn push_dlq(&self, msg: &DlqMessage) -> Result<()>;
    #[allow(dead_code)]
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<DlqMessage>>;
    #[allow(dead_code)]
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<DlqMessage>>;
    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<DlqMessage>>;

    // --- Management Domain ---
    async fn clear_profile(&self, profile: &str) -> Result<()>;
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()>;
    async fn list_all_profiles(&self) -> Result<Vec<String>>;

}

#[async_trait]
pub trait StoreBuilder: Send + Sync {
    fn scheme(&self) -> &str;
    async fn build(&self, url: &str, app_dir: &Path, fingerprint: &str) -> Result<Arc<dyn Store>>;
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

pub async fn create_store_from_url(url: &str) -> Result<Arc<dyn Store>> {
    if url == "local" {
        let app_dir = crate::core::config::get_app_dir();
        let seal_path = app_dir.join(".seal");
        return Ok(Arc::new(file::FileStore::new(seal_path, "migration")?) as Arc<dyn Store>);
    }

    let final_url = if url == "innerdb" {
        let app_dir = crate::core::config::get_app_dir();
        let db_path = app_dir.join("cowen.db");
        format!("sqlite://{}", db_path.to_str().unwrap())
    } else {
        url.to_string()
    };

    if final_url.starts_with("redis://") {
        let client = redis::Client::open(final_url.as_str())?;
        let conn = client.get_multiplexed_tokio_connection().await?;
        return Ok(Arc::new(redis_store::RedisStore::new(conn, &final_url)) as Arc<dyn Store>);
    }

    // SQL variants (sqlite, mysql, postgres, mssql)
    Ok(Arc::new(sql::SqlStore::from_url(final_url.as_str()).await?) as Arc<dyn Store>)
}
