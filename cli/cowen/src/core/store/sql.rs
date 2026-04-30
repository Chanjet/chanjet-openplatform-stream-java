mod mysql;
mod postgres;
mod sqlite;
mod mssql;

use super::{Store, AuditEntry, DlqMessage, Item};
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
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

pub struct SqlBuilderRegistration {
    pub builder: &'static dyn SqlBuilder,
}

inventory::collect!(SqlBuilderRegistration);

pub struct SqlStore {
    driver: Arc<dyn SqlDriver>,
}

impl SqlStore {
    pub fn new(driver: Arc<dyn SqlDriver>) -> Self {
        Self { driver }
    }

    pub fn supported_schemes() -> Vec<String> {
        inventory::iter::<SqlBuilderRegistration>
            .into_iter()
            .map(|reg| reg.builder.scheme().to_string())
            .collect()
    }

    pub fn is_supported(scheme: &str) -> bool {
        let scheme = if scheme == "innerdb" { "sqlite" } else { scheme };
        inventory::iter::<SqlBuilderRegistration>
            .into_iter()
            .any(|reg| reg.builder.scheme() == scheme)
    }

    pub async fn from_url(url: &str) -> Result<Self> {
        let mut scheme = url.split(':').next().ok_or_else(|| anyhow::anyhow!("Invalid database URL"))?;
        
        if scheme == "innerdb" {
            scheme = "sqlite";
        }
        
        for reg in inventory::iter::<SqlBuilderRegistration> {
            if reg.builder.scheme() == scheme {
                let driver = reg.builder.build(url).await?;
                return Ok(Self::new(driver));
            }
        }

        Err(anyhow::anyhow!("Unsupported database scheme: {}. Supported: {:?}", scheme, Self::supported_schemes()))
    }
}

pub struct SqlStoreBuilder;

#[async_trait]
impl super::StoreBuilder for SqlStoreBuilder {
    fn scheme(&self) -> &str {
        "sql_proxy" // Internal marker, we handle multiple schemes
    }

    async fn build(&self, _url: &str, _app_dir: &Path, _fingerprint: &str) -> Result<Arc<dyn Store>> {
        unreachable!("SqlStoreBuilder uses a custom discovery loop in vault.rs or from_url")
    }
}

// Special case for InnerDB (Hybrid)
pub struct InnerDbStoreBuilder;

#[async_trait]
impl super::StoreBuilder for InnerDbStoreBuilder {
    fn scheme(&self) -> &str {
        "innerdb"
    }

    async fn build(&self, _url: &str, _app_dir: &Path, _fingerprint: &str) -> Result<Arc<dyn Store>> {
        // This is tricky because StoreBuilder only returns ONE store, 
        // but vault.rs needs two in its current architecture.
        // Let's refine StoreBuilder or vault.rs to handle this.
        unimplemented!("Refining Vault assembly to handle primary/sensitive split")
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
