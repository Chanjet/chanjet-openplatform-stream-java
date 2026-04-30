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
    async fn get_config_full(&self, profile: &str, key: &str) -> Result<Item>;
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> Result<()>;
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>>;
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()>;

    // --- Secret Domain (Sensitive) ---
    async fn get_secret(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()>;

    // --- Token Domain (Ephemeral/Dynamic) ---
    async fn get_token(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> Result<()>;
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

    // --- Legacy / Generic Fallback ---
    async fn get(&self, profile: &str, key: &str) -> Result<String> { self.get_config(profile, key).await }
    async fn set(&self, profile: &str, key: &str, value: &str) -> Result<()> { self.set_config(profile, key, value).await }
    async fn delete(&self, profile: &str, key: &str) -> Result<()> { self.delete_config(profile, key).await }
    async fn list_keys(&self, profile: &str, prefix: &str) -> Result<Vec<String>> {
        let keys = self.list_configs(profile).await?;
        Ok(keys.into_iter().filter(|k| k.starts_with(prefix)).collect())
    }

    // --- Management Domain ---
    async fn clear_profile(&self, profile: &str) -> Result<()>;
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()>;
    async fn list_all_profiles(&self) -> Result<Vec<String>>;

    // --- Notification Domain (Reactive) ---
    async fn notify_config_changed(&self, profile: &str, key: &str) -> Result<()>;
    async fn watch_config(&self, profile: &str) -> Result<std::pin::Pin<Box<dyn tokio_stream::Stream<Item = String> + Send>>>;
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
        let app_dir = super::config::get_app_dir();
        let seal_path = app_dir.join(".seal");
        return Ok(Arc::new(file::FileStore::new(seal_path, "migration")?) as Arc<dyn Store>);
    }

    let final_url = if url == "innerdb" {
        let app_dir = super::config::get_app_dir();
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
