use anyhow::Result;
use async_trait::async_trait;
use crate::core::store::{Store, AuditEntry, DlqMessage, Item};
use crate::core::config::AppConfig;
use std::path::Path;
use std::sync::Arc;

#[async_trait]
pub trait Vault: Send + Sync {
    // --- Config Domain ---
    async fn get_config(&self, profile: &str, key: &str) -> Result<String>;
    async fn get_config_full(&self, profile: &str, key: &str) -> Result<Item>;
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> Result<()>;
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>>;
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()>;

    // --- Secret Domain ---
    async fn get_secret(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()>;

    // --- Token Domain ---
    async fn get_token(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> Result<()>;

    // --- Audit Domain ---
    async fn save_audit(&self, entry: &AuditEntry) -> Result<()>;
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<AuditEntry>>;

    // --- DLQ Domain ---
    async fn push_dlq(&self, msg: &DlqMessage) -> Result<()>;
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<DlqMessage>>;
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<DlqMessage>>;

    // --- Cache Domain ---
    async fn get_cache(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_cache(&self, profile: &str, key: &str, value: &str, ttl_secs: u64) -> Result<()>;

    // --- Legacy / Generic Fallback ---
    async fn get(&self, profile: &str, key: &str) -> Result<String>;
    async fn set(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    async fn delete(&self, profile: &str, key: &str) -> Result<()>;
    async fn list_keys(&self, profile: &str, prefix: &str) -> Result<Vec<String>>;

    // --- Management ---
    async fn clear_profile(&self, profile: &str) -> Result<()>;
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()>;
    async fn list_all_profiles(&self) -> Result<Vec<String>>;

    // --- Notification ---
    async fn notify_config_changed(&self, profile: &str, key: &str) -> Result<()>;
    async fn watch_config(&self, profile: &str) -> Result<std::pin::Pin<Box<dyn tokio_stream::Stream<Item = String> + Send>>>;

    // --- Migration Support ---
    fn primary_store(&self) -> Arc<dyn crate::core::store::Store>;
}

pub struct StoreVault {
    primary: Arc<dyn Store>,    // Config, Token, Logs, DLQ...
    sensitive: Arc<dyn Store>,  // Pinned to .seal OR Database
}

impl StoreVault {
    pub fn new(primary: Arc<dyn Store>, sensitive: Arc<dyn Store>) -> Self {
        Self { primary, sensitive }
    }
}

#[async_trait]
impl Vault for StoreVault {
    // --- Config Domain (Primary) ---
    async fn get_config(&self, profile: &str, key: &str) -> Result<String> { self.primary.get_config(profile, key).await }
    async fn get_config_full(&self, profile: &str, key: &str) -> Result<Item> { self.primary.get_config_full(profile, key).await }
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()> { self.primary.set_config(profile, key, value).await }
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, ev: u64) -> Result<()> { self.primary.set_config_conditional(profile, key, value, ev).await }
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>> { self.primary.list_configs(profile).await }
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()> { self.primary.delete_config(profile, key).await }

    // --- Secret Domain (Routed based on Stateless requirement) ---
    async fn get_secret(&self, profile: &str, key: &str) -> Result<String> { self.sensitive.get_secret(profile, key).await }
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()> { self.sensitive.set_secret(profile, key, value).await }

    // --- Token Domain (Primary) ---
    async fn get_token(&self, profile: &str, key: &str) -> Result<String> { self.primary.get_token(profile, key).await }
    async fn set_token(&self, profile: &str, key: &str, value: &str, exp: u64) -> Result<()> { self.primary.set_token(profile, key, value, exp).await }

    // --- Audit Domain (Primary) ---
    async fn save_audit(&self, entry: &AuditEntry) -> Result<()> { self.primary.save_audit(entry).await }
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<AuditEntry>> { self.primary.list_audit(profile, limit).await }

    // --- DLQ Domain (Primary) ---
    async fn push_dlq(&self, msg: &DlqMessage) -> Result<()> { self.primary.push_dlq(msg).await }
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<DlqMessage>> { self.primary.pop_dlq(profile, topic).await }
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<DlqMessage>> { self.primary.list_dlq(profile, limit).await }

    // --- Cache Domain (Primary) ---
    async fn get_cache(&self, profile: &str, key: &str) -> Result<String> { self.primary.get_cache(profile, key).await }
    async fn set_cache(&self, profile: &str, key: &str, value: &str, ttl: u64) -> Result<()> { self.primary.set_cache(profile, key, value, ttl).await }

    // --- Legacy Fallback ---
    async fn get(&self, profile: &str, key: &str) -> Result<String> { self.primary.get(profile, key).await }
    async fn set(&self, profile: &str, key: &str, value: &str) -> Result<()> { self.primary.set(profile, key, value).await }
    async fn delete(&self, profile: &str, key: &str) -> Result<()> { self.primary.delete(profile, key).await }
    async fn list_keys(&self, profile: &str, prefix: &str) -> Result<Vec<String>> { self.primary.list_keys(profile, prefix).await }

    // --- Management ---
    async fn clear_profile(&self, profile: &str) -> Result<()> {
        let _ = self.sensitive.clear_profile(profile).await;
        if !Arc::ptr_eq(&self.sensitive, &self.primary) {
            let _ = self.primary.clear_profile(profile).await;
        }
        Ok(())
    }
    async fn rename_profile(&self, old: &str, new: &str) -> Result<()> {
        let _ = self.sensitive.rename_profile(old, new).await;
        if !Arc::ptr_eq(&self.sensitive, &self.primary) {
            let _ = self.primary.rename_profile(old, new).await;
        }
        Ok(())
    }
    async fn list_all_profiles(&self) -> Result<Vec<String>> {
        // We primary use 'primary' to list profiles since it has configs
        self.primary.list_all_profiles().await
    }

    async fn notify_config_changed(&self, profile: &str, key: &str) -> Result<()> {
        self.primary.notify_config_changed(profile, key).await
    }
    async fn watch_config(&self, profile: &str) -> Result<std::pin::Pin<Box<dyn tokio_stream::Stream<Item = String> + Send>>> {
        self.primary.watch_config(profile).await
    }

    fn primary_store(&self) -> Arc<dyn crate::core::store::Store> {
        self.primary.clone()
    }
}

pub async fn create_vault(app_config: &AppConfig, app_dir: &Path, fingerprint: &str) -> Result<Arc<dyn Vault>> {
    use crate::core::store::sql::SqlStore;
    use crate::core::store::redis_store::RedisStore;
    use crate::core::store::hybrid::HybridStore;
    use crate::core::store::file::FileStore;

    let storage_cfg = &app_config.storage;
    let seal_path = app_dir.join(".seal");

    let (primary, sensitive): (Arc<dyn Store>, Arc<dyn Store>) = match storage_cfg.store.as_str() {
        "local" => {
            let store: Arc<dyn Store> = Arc::new(FileStore::new(seal_path, fingerprint)?);
            (store.clone(), store)
        }
        "innerdb" | "sqlite" => {
            let db_url = storage_cfg.db_url.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Database URL is required for InnerDB"))?;
            let sql_store: Arc<dyn Store> = Arc::new(SqlStore::from_url(db_url).await?);
            // COMPATIBILITY: InnerDB pins secrets to .seal to avoid breaking existing setups
            let secret_store: Arc<dyn Store> = Arc::new(FileStore::new(seal_path, fingerprint)?);
            (sql_store, secret_store)
        }
        "mysql" | "postgres" => {
            let db_url = storage_cfg.db_url.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Database URL is required for remote SQL storage"))?;
            let sql_store: Arc<dyn Store> = Arc::new(SqlStore::from_url(db_url).await?);
            // STATELESS: Remote SQL stores secrets in the database for node parity
            (sql_store.clone(), sql_store)
        }
        _ => return Err(anyhow::anyhow!("Unsupported store type: {}", storage_cfg.store)),
    };

    let final_primary = if storage_cfg.cache == "redis" {
        let redis_url = storage_cfg.cache_url.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Redis URL is required for cache storage"))?;
        let client = redis::Client::open(redis_url.as_str())?;
        let conn = client.get_multiplexed_tokio_connection().await?;
        let redis_store: Arc<dyn Store> = Arc::new(RedisStore::new(conn, redis_url));
        Arc::new(HybridStore::new(redis_store, primary))
    } else {
        primary
    };

    Ok(Arc::new(StoreVault::new(final_primary, sensitive)))
}
