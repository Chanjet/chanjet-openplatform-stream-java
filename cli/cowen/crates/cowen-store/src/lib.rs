use crate::sql::SqlBuilder;
use cowen_common::{CowenResult, CowenError};
use std::sync::Arc;
pub use cowen_common::models::{Item, AuditEntry, DlqMessage};
pub use cowen_common::store::{Store, StoreBuilder, StoreBuilderRegistration, CacheBuilder, CacheBuilderRegistration};
pub use cowen_config::{ConfigManager, ConfigValidator};

pub mod file;
pub mod hybrid;

#[cfg(feature = "redis")]
pub mod redis_store;

#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql", feature = "mssql"))]
pub mod sql;

pub mod vault_impl;
pub mod migration;

#[cfg(test)]
mod redis_tests;

pub use file::FileStore;
pub use hybrid::HybridStore;
#[cfg(feature = "redis")]
pub use redis_store::RedisStore;
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql", feature = "mssql"))]
pub use sql::SqlStore;
pub use vault_impl::StoreVault;

pub async fn create_store_from_url(url: &str, app_dir: &std::path::Path, fingerprint: &str) -> CowenResult<Arc<dyn Store>> {
    // 1. Core Logic Redirection (Legacy Support)
    if url == "local" {
         let seal_path = app_dir.join(".seal");
         let vault_dir = app_dir.join("vault");
         let is_sealed = seal_path.is_file();
         let fp_opt = if is_sealed { Some(fingerprint) } else { None };

         // 🚀 V3 MIGRATION TRIGGER
         if vault_dir.is_dir() {
             if let Ok(entries) = std::fs::read_dir(&vault_dir) {
                 for entry in entries.flatten() {
                     if let Some(name) = entry.file_name().to_str() {
                         if name.ends_with(".json") {
                             let profile = &name[..name.len()-5];
                             let _ = file::migration::migrate_v2_to_v3(&vault_dir, profile, fp_opt).await;
                         }
                     }
                 }
             }
         }

         if is_sealed {
             return Ok(Arc::new(file::MonolithicSealStore::new(vault_dir, fingerprint)) as Arc<dyn Store>);
         }
         return Ok(Arc::new(FileStore::new(vault_dir, None)?) as Arc<dyn Store>);
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
            let path = std::path::Path::new(path_part);
            if path.is_relative() {
                if path.starts_with(&app_dir) || (app_dir.is_absolute() && path.to_string_lossy().contains(app_dir.to_string_lossy().as_ref())) {
                     actual_url = format!("sqlite://{}", path.to_string_lossy());
                } else {
                     let db_path = app_dir.join(path);
                     actual_url = format!("sqlite://{}", db_path.to_string_lossy());
                }
            } else {
                actual_url = format!("sqlite://{}", path_part);
            }
        } else {
            let db_path = app_dir.join("cowen.db");
            actual_url = format!("sqlite:{}", db_path.to_string_lossy());
        }
    }

    // 3. Normalize SQLite paths and create parent directories
    // 3. Normalize SQLite paths and create parent directories
    if actual_url.starts_with("sqlite:") {
        let path_part = if actual_url.starts_with("sqlite://") {
            actual_url[9..].to_string()
        } else if actual_url.starts_with("sqlite:") {
            actual_url[7..].to_string()
        } else {
            actual_url.clone()
        };
        
        let pure_path = path_part.split('?').next().unwrap();
        
        let db_path = if pure_path.is_empty() {
            app_dir.join("cowen.db")
        } else if std::path::Path::new(pure_path).is_absolute() {
            std::path::PathBuf::from(pure_path)
        } else {
            // 🚀 SYNC: For relative paths, we MUST decide if it is relative to app_dir
            // In E2E tests, it might be relative to the test root.
            // If it starts with './' or '../' or just 'file.db', we join with app_dir
            app_dir.join(pure_path)
        };

        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        // Ensure format is always sqlite:<path> for SQLx
        actual_url = format!("sqlite:{}", db_path.to_string_lossy());
    } else if (actual_url.starts_with("mysql:") || actual_url.starts_with("postgres:")) && !actual_url.contains("://") {
        actual_url = actual_url.replace("mysql:", "mysql://").replace("postgres:", "postgres://");
    }

    
    let scheme = if actual_url.starts_with("sqlite:") {
        "sqlite".to_string()
    } else {
        actual_url.split(':').next().ok_or_else(|| CowenError::api("Invalid database URL"))?.to_string()
    };

    if scheme == "redis" {
        #[cfg(feature = "redis")]
        {
             let redis_url = if !actual_url.starts_with("redis://") {
                 format!("redis://{}", actual_url.strip_prefix("redis:").unwrap_or(&actual_url))
             } else {
                 actual_url.clone()
             };
             let client = redis::Client::open(redis_url.as_str()).map_err(|e| CowenError::Store(e.to_string()))?;
             let conn = client.get_multiplexed_tokio_connection().await.map_err(|e| CowenError::Store(e.to_string()))?;
             return Ok(Arc::new(RedisStore::new(conn, redis_url)));
        }
        #[cfg(not(feature = "redis"))]
        {
             return Err(CowenError::Store("Redis feature not enabled".to_string()));
        }
    }

    if scheme == "sqlite" {
        let driver = crate::sql::sqlite::SqliteBuilder.build(&actual_url).await?;
        return Ok(Arc::new(SqlStore::new(driver, "sqlite", &actual_url)) as Arc<dyn Store>);
    }

    if SqlStore::is_supported(&scheme) {
        return Ok(Arc::new(SqlStore::from_url(&actual_url).await?));
    }

    for reg in inventory::iter::<StoreBuilderRegistration> {
        if reg.builder.scheme() == scheme {
            return reg.builder.build(&actual_url, app_dir, fingerprint).await;
        }
    }

    Err(CowenError::api(format!("Unsupported database scheme: {}.", scheme)))
}

pub async fn create_vault(app_cfg: &cowen_common::config::AppConfig, app_dir: &std::path::Path, fingerprint: &str) -> CowenResult<Arc<dyn cowen_common::vault::Vault>> {
    let store_type = &app_cfg.storage.store;
    
    let primary = if store_type == "local" {
        create_store_from_url(store_type, app_dir, fingerprint).await?
    } else if store_type == "innerdb" || store_type == "sqlite" {
        let url = app_cfg.storage.db_url.as_ref().cloned().unwrap_or_else(|| "innerdb".to_string());
        create_store_from_url(&url, app_dir, fingerprint).await?
    } else {
        let url = app_cfg.storage.db_url.as_ref().ok_or_else(|| CowenError::store(format!("Database URL is missing for distributed store: {}", store_type)))?;
        create_store_from_url(url, app_dir, fingerprint).await?
    };

    let sensitive = if let Some(url) = &app_cfg.storage.db_url {
        if store_type != "local" {
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
    let vault = Arc::new(StoreVault::new(primary, sensitive));
    if let Err(e) = cowen_common::vault::Vault::migrate(vault.as_ref()).await {
        tracing::error!(target: "sys", "Failed to run store migrations: {}", e);
    }
    Ok(vault)
}
