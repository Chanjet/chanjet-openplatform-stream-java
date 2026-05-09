use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use cowen_common::models::{Token, Ticket, Item, AuditEntry, DlqMessage};
pub use cowen_common::store::{Store, StoreBuilder, StoreBuilderRegistration, CacheBuilder, CacheBuilderRegistration};
pub use cowen_common::{ConfigManager, ConfigValidator};

pub mod file;
pub mod hybrid;

#[cfg(feature = "redis")]
pub mod redis_store;

#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql", feature = "mssql"))]
pub mod sql;

pub mod vault_impl;

pub use file::FileStore;
pub use hybrid::HybridStore;
#[cfg(feature = "redis")]
pub use redis_store::RedisStore;
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql", feature = "mssql"))]
pub use sql::SqlStore;
pub use vault_impl::StoreVault;

pub async fn create_store_from_url(url: &str, app_dir: &std::path::Path, fingerprint: &str) -> Result<Arc<dyn Store>> {
    let mut scheme = url.split(':').next().ok_or_else(|| anyhow::anyhow!("Invalid database URL"))?;
    
    let mut actual_url = if scheme == "innerdb" {
        scheme = "sqlite";
        let path_part = url.strip_prefix("innerdb://").unwrap_or("");
        if !path_part.is_empty() {
            format!("sqlite:{}", path_part)
        } else {
            let db_path = app_dir.join("cowen.db");
            format!("sqlite:{}", db_path.to_string_lossy())
        }
    } else {
        url.to_string()
    };

    if actual_url.starts_with("sqlite://") {
        let path = actual_url.strip_prefix("sqlite://").unwrap();
        if !path.starts_with('/') {
            actual_url = format!("sqlite:{}", path);
        }
    }

    if scheme == "sqlite" || scheme == "postgres" || scheme == "mysql" || scheme == "mssql" {
        return Ok(Arc::new(SqlStore::from_url(&actual_url).await?));
    }

    if scheme == "local" {
         let seal_path = app_dir.join(".seal");
         return Ok(Arc::new(FileStore::new(seal_path, fingerprint)?) as Arc<dyn Store>);
    }

    for reg in inventory::iter::<StoreBuilderRegistration> {
        if reg.builder.scheme() == scheme {
            return reg.builder.build(&actual_url, app_dir, fingerprint).await;
        }
    }

    Err(anyhow::anyhow!("Unsupported database scheme: {}.", scheme))
}

pub async fn create_vault(app_cfg: &cowen_common::config::AppConfig, app_dir: &std::path::Path, fingerprint: &str) -> Result<Arc<dyn cowen_common::vault::Vault>> {
    let primary = create_store_from_url(&app_cfg.storage.store, app_dir, fingerprint).await?;
    let sensitive = if let Some(url) = &app_cfg.storage.db_url {
        create_store_from_url(url, app_dir, fingerprint).await?
    } else {
        primary.clone()
    };
    Ok(Arc::new(StoreVault::new(primary, sensitive)))
}
