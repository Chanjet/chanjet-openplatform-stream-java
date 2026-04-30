mod mysql;
mod postgres;
mod sqlite;

use super::{Store, AuditEntry, DlqMessage, Item};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait SqlDriver: Send + Sync {
    // --- Config ---
    async fn get_config(&self, profile: &str, key: &str) -> Result<String>;
    async fn get_config_full(&self, profile: &str, key: &str) -> Result<Item>;
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> Result<()>;
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>>;
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()>;

    // --- Secret ---
    async fn get_secret(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()>;

    // --- Token ---
    async fn get_token(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> Result<()>;
    async fn list_tokens(&self, profile: &str) -> Result<Vec<String>>;

    // --- Audit ---
    async fn save_audit(&self, entry: &AuditEntry) -> Result<()>;
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<AuditEntry>>;

    // --- DLQ ---
    async fn push_dlq(&self, msg: &DlqMessage) -> Result<()>;
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<DlqMessage>>;
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<DlqMessage>>;
    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<DlqMessage>>;

    // --- Cache ---
    async fn get_cache(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_cache(&self, profile: &str, key: &str, value: &str, ttl_secs: u64) -> Result<()>;

    // --- Management ---
    async fn clear_profile(&self, profile: &str) -> Result<()>;
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()>;
    async fn list_all_profiles(&self) -> Result<Vec<String>>;

    // --- Notification ---
    async fn notify_config_changed(&self, profile: &str, key: &str) -> Result<()>;
    async fn watch_config(&self, profile: &str) -> Result<std::pin::Pin<Box<dyn tokio_stream::Stream<Item = String> + Send>>>;
}

#[async_trait]
pub trait SqlBuilder: Send + Sync {
    fn scheme(&self) -> &str;
    async fn build(&self, url: &str) -> Result<Arc<dyn SqlDriver>>;
}

pub struct SqlStore {
    driver: Arc<dyn SqlDriver>,
}

impl SqlStore {
    pub fn new(driver: Arc<dyn SqlDriver>) -> Self {
        Self { driver }
    }

    pub async fn from_url(url: &str) -> Result<Self> {
        let mut scheme = url.split(':').next().ok_or_else(|| anyhow::anyhow!("Invalid database URL"))?;
        
        // Alias support
        if scheme == "innerdb" {
            scheme = "sqlite";
        }
        
        let builders = [
            Arc::new(mysql::MySqlBuilder) as Arc<dyn SqlBuilder>,
            Arc::new(postgres::PostgresBuilder) as Arc<dyn SqlBuilder>,
            Arc::new(sqlite::SqliteBuilder) as Arc<dyn SqlBuilder>,
        ];

        for builder in builders {
            if builder.scheme() == scheme {
                let driver = builder.build(url).await?;
                return Ok(Self::new(driver));
            }
        }

        Err(anyhow::anyhow!("Unsupported database scheme: {}", scheme))
    }
}

#[async_trait]
impl Store for SqlStore {
    // --- Config ---
    async fn get_config(&self, profile: &str, key: &str) -> Result<String> { self.driver.get_config(profile, key).await }
    async fn get_config_full(&self, profile: &str, key: &str) -> Result<Item> { self.driver.get_config_full(profile, key).await }
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()> { self.driver.set_config(profile, key, value).await }
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, ev: u64) -> Result<()> { self.driver.set_config_conditional(profile, key, value, ev).await }
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>> { self.driver.list_configs(profile).await }
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()> { self.driver.delete_config(profile, key).await }

    // --- Secret ---
    async fn get_secret(&self, profile: &str, key: &str) -> Result<String> { self.driver.get_secret(profile, key).await }
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()> { self.driver.set_secret(profile, key, value).await }

    // --- Token ---
    async fn get_token(&self, profile: &str, key: &str) -> Result<String> { self.driver.get_token(profile, key).await }
    async fn set_token(&self, profile: &str, key: &str, value: &str, exp: u64) -> Result<()> { self.driver.set_token(profile, key, value, exp).await }
    async fn list_tokens(&self, profile: &str) -> Result<Vec<String>> { self.driver.list_tokens(profile).await }

    // --- Audit ---
    async fn save_audit(&self, entry: &AuditEntry) -> Result<()> { self.driver.save_audit(entry).await }
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<AuditEntry>> { self.driver.list_audit(profile, limit).await }

    // --- DLQ ---
    async fn push_dlq(&self, msg: &DlqMessage) -> Result<()> { self.driver.push_dlq(msg).await }
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<DlqMessage>> { self.driver.pop_dlq(profile, topic).await }
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<DlqMessage>> { self.driver.list_dlq(profile, limit).await }
    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<DlqMessage>> { self.driver.list_all_dlq(profile).await }

    // --- Cache ---
    async fn get_cache(&self, profile: &str, key: &str) -> Result<String> { self.driver.get_cache(profile, key).await }
    async fn set_cache(&self, profile: &str, key: &str, value: &str, ttl: u64) -> Result<()> { self.driver.set_cache(profile, key, value, ttl).await }

    // --- Legacy ---
    async fn delete(&self, profile: &str, key: &str) -> Result<()> { self.driver.delete_config(profile, key).await }

    // --- Management ---
    async fn clear_profile(&self, profile: &str) -> Result<()> { self.driver.clear_profile(profile).await }
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()> { self.driver.rename_profile(old_name, new_name).await }
    async fn list_all_profiles(&self) -> Result<Vec<String>> { self.driver.list_all_profiles().await }

    // --- Notification ---
    async fn notify_config_changed(&self, profile: &str, key: &str) -> Result<()> { self.driver.notify_config_changed(profile, key).await }
    async fn watch_config(&self, profile: &str) -> Result<std::pin::Pin<Box<dyn tokio_stream::Stream<Item = String> + Send>>> { self.driver.watch_config(profile).await }
}
