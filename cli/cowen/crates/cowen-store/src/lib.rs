use anyhow::Result;
use std::sync::Arc;
use cowen_common::models::{Item, AuditEntry, DlqMessage};
pub use cowen_common::store::{Store, StoreBuilder, StoreBuilderRegistration, CacheBuilder, CacheBuilderRegistration};
pub use cowen_common::{ConfigManager, ConfigValidator};

pub mod file;
pub mod hybrid;

#[cfg(feature = "redis")]
pub mod redis_store;

#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql", feature = "mssql"))]
pub mod sql;

pub mod vault_impl;
pub mod migration;

pub use file::FileStore;
pub use hybrid::HybridStore;
#[cfg(feature = "redis")]
pub use redis_store::RedisStore;
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql", feature = "mssql"))]
pub use sql::SqlStore;
pub use vault_impl::StoreVault;

pub async fn create_store_from_url(url: &str, app_dir: &std::path::Path, fingerprint: &str) -> Result<Arc<dyn Store>> {
    // 1. Core Logic Redirection (Legacy Support)
    if url == "local" {
         let seal_path = app_dir.join(".seal");
         return Ok(Arc::new(FileStore::new(seal_path, fingerprint)?) as Arc<dyn Store>);
    }

    let mut actual_url = if url == "innerdb" {
        let db_path = app_dir.join("cowen.db");
        format!("sqlite://{}", db_path.to_string_lossy())
    } else {
        url.to_string()
    };

    // 2. Expand innerdb:// protocol if needed
    if actual_url.starts_with("innerdb:") {
        let path_part = actual_url.strip_prefix("innerdb://").unwrap_or("");
        if !path_part.is_empty() {
            actual_url = format!("sqlite:{}", path_part);
        } else {
            let db_path = app_dir.join("cowen.db");
            actual_url = format!("sqlite:{}", db_path.to_string_lossy());
        }
    }

    // 3. Normalize SQLite paths and create parent directories
    if actual_url.starts_with("sqlite:") {
        let path_part = if actual_url.starts_with("sqlite://") {
            &actual_url[9..]
        } else {
            &actual_url[7..]
        };
        
        let pure_path = path_part.split('?').next().unwrap();
        let db_path = std::path::Path::new(pure_path);
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        
        // SQLx 0.8 Fix: Re-unify sqlite:// to sqlite: for relative paths to avoid hostname parsing
        if actual_url.starts_with("sqlite://") {
            let path = &actual_url[9..];
            if !path.starts_with('/') {
                 actual_url = format!("sqlite:{}", path);
            }
        }
    } else if (actual_url.starts_with("mysql:") || actual_url.starts_with("postgres:")) && !actual_url.contains("://") {
        actual_url = actual_url.replace("mysql:", "mysql://").replace("postgres:", "postgres://");
    }

    // 4. Resolve final scheme (after mutations are finished)
    let scheme = actual_url.split(':').next().ok_or_else(|| anyhow::anyhow!("Invalid database URL"))?.to_string();

    // 5. Dispatch to Redis if needed
    if scheme == "redis" {
        #[cfg(feature = "redis")]
        {
             let redis_url = if !actual_url.starts_with("redis://") {
                 format!("redis://{}", actual_url.strip_prefix("redis:").unwrap_or(&actual_url))
             } else {
                 actual_url.clone()
             };
             return Ok(Arc::new(RedisStore::new(
                 redis::Client::open(redis_url.as_str())?.get_multiplexed_tokio_connection().await?,
                 &redis_url
             )));
        }
        #[cfg(not(feature = "redis"))]
        {
             return Err(anyhow::anyhow!("Redis feature is disabled"));
        }
    }

    // 6. Dispatch to SQL variants
    if scheme == "sqlite" || scheme == "postgres" || scheme == "mysql" || scheme == "mssql" {
        return Ok(Arc::new(SqlStore::from_url(&actual_url).await?));
    }

    // 7. Generic Discovery Fallback
    for reg in inventory::iter::<StoreBuilderRegistration> {
        if reg.builder.scheme() == scheme {
            return reg.builder.build(&actual_url, app_dir, fingerprint).await;
        }
    }

    Err(anyhow::anyhow!("Unsupported database scheme: {}.", scheme))
}

pub async fn create_vault(app_cfg: &cowen_common::config::AppConfig, app_dir: &std::path::Path, fingerprint: &str) -> Result<Arc<dyn cowen_common::vault::Vault>> {
    let store_type = &app_cfg.storage.store;
    
    // Resolve store instance
    let primary = if store_type == "local" {
        create_store_from_url(store_type, app_dir, fingerprint).await?
    } else if store_type == "innerdb" || store_type == "sqlite" {
        let url = app_cfg.storage.db_url.as_ref().cloned().unwrap_or_else(|| "innerdb".to_string());
        create_store_from_url(&url, app_dir, fingerprint).await?
    } else {
        let url = app_cfg.storage.db_url.as_ref().ok_or_else(|| anyhow::anyhow!("Database URL is missing for distributed store: {}", store_type))?;
        create_store_from_url(url, app_dir, fingerprint).await?
    };

    // sensitive uses db_url if provided, otherwise defaults to primary
    let sensitive = if let Some(url) = &app_cfg.storage.db_url {
        if store_type != "local" {
             // For all SQL-like stores (including innerdb/sqlite), try to reuse or recreate from URL
             let mut is_same_db = false;
             if store_type == "innerdb" || store_type == "sqlite" {
                  is_same_db = true; 
             } else if let Some(p_url) = app_cfg.storage.db_url.as_ref() {
                  if url == p_url { is_same_db = true; }
             }

             if is_same_db {
                  primary.clone()
             } else {
                  create_store_from_url(url, app_dir, fingerprint).await?
             }
        } else {
             create_store_from_url(url, app_dir, fingerprint).await?
        }
    } else {
        primary.clone()
    };
    Ok(Arc::new(StoreVault::new(primary, sensitive)))
}
