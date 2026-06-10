use crate::sql::SqlBuilder;
pub use cowen_common::models::{AuditEntry, DlqMessage, Item};
pub use cowen_common::store::{
    CacheBuilder, CacheBuilderRegistration, Store, StoreBuilder, StoreBuilderRegistration,
};
use cowen_common::{CowenError, CowenResult};
pub use cowen_config::{ConfigManager, ConfigValidator};
use std::sync::Arc;

pub mod file;
pub mod hybrid;

#[cfg(feature = "redis")]
pub mod redis_store;

#[cfg(any(
    feature = "sqlite",
    feature = "postgres",
    feature = "mysql",
    feature = "mssql"
))]
pub mod sql;

#[cfg(feature = "sqlite")]
pub mod dlq_store;

pub mod diagnostics;
pub mod migration;
pub mod vault_impl;

#[cfg(all(test, feature = "redis"))]
mod redis_tests;

pub use file::FileStore;
pub use hybrid::HybridStore;
#[cfg(feature = "redis")]
pub use redis_store::RedisStore;
#[cfg(any(
    feature = "sqlite",
    feature = "postgres",
    feature = "mysql",
    feature = "mssql"
))]
pub use sql::SqlStore;
pub use vault_impl::StoreVault;

async fn handle_local_store(
    app_dir: &std::path::Path,
    fingerprint: &str,
) -> CowenResult<Arc<dyn Store>> {
    let seal_path = app_dir.join(".seal");
    let vault_dir = app_dir.join("vault");
    let is_sealed = seal_path.is_file();
    let fp_opt = if is_sealed { Some(fingerprint) } else { None };

    // 🚀 V3 MIGRATION TRIGGER
    if vault_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&vault_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if let Some(profile) = name.strip_suffix(".json") {
                        let _ =
                            file::migration::migrate_v2_to_v3(&vault_dir, profile, fp_opt).await;
                    }
                }
            }
        }
    }

    if is_sealed {
        return Ok(
            Arc::new(file::MonolithicSealStore::new(vault_dir, fingerprint)) as Arc<dyn Store>,
        );
    }
    Ok(Arc::new(FileStore::new(vault_dir, None)?) as Arc<dyn Store>)
}

fn resolve_innerdb_url(url: &str, app_dir: &std::path::Path) -> String {
    let mut actual_url = if url == "innerdb" {
        let db_path = app_dir.join("cowen.db");
        format!("sqlite://{}", db_path.to_string_lossy())
    } else {
        url.to_string()
    };

    if actual_url.starts_with("innerdb:") {
        let path_part = actual_url.strip_prefix("innerdb://").unwrap_or("");
        if !path_part.is_empty() {
            let path = std::path::Path::new(path_part);
            if path.is_relative() {
                if path.starts_with(app_dir)
                    || (app_dir.is_absolute()
                        && path
                            .to_string_lossy()
                            .contains(app_dir.to_string_lossy().as_ref()))
                {
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
    actual_url
}

fn normalize_sqlite_url(mut actual_url: String, app_dir: &std::path::Path) -> String {
    if actual_url.starts_with("sqlite:") {
        let path_part = if let Some(stripped) = actual_url.strip_prefix("sqlite://") {
            stripped.to_string()
        } else if let Some(stripped) = actual_url.strip_prefix("sqlite:") {
            stripped.to_string()
        } else {
            actual_url.clone()
        };

        let pure_path = path_part.split('?').next().unwrap();

        let db_path = if pure_path.is_empty() {
            app_dir.join("cowen.db")
        } else if std::path::Path::new(pure_path).is_absolute() {
            std::path::PathBuf::from(pure_path)
        } else {
            app_dir.join(pure_path)
        };

        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        actual_url = format!("sqlite:{}", db_path.to_string_lossy());
        eprintln!("🔍 SQLITE URL RESOLVED TO: {}", actual_url);
    } else if (actual_url.starts_with("mysql:") || actual_url.starts_with("postgres:"))
        && !actual_url.contains("://")
    {
        actual_url = actual_url
            .replace("mysql:", "mysql://")
            .replace("postgres:", "postgres://");
    }
    actual_url
}

async fn handle_redis_url(_actual_url: &str) -> CowenResult<Arc<dyn Store>> {
    #[cfg(feature = "redis")]
    {
        let redis_url = if !_actual_url.starts_with("redis://") {
            format!(
                "redis://{}",
                _actual_url.strip_prefix("redis:").unwrap_or(_actual_url)
            )
        } else {
            _actual_url.to_string()
        };
        let client = redis::Client::open(redis_url.as_str())
            .map_err(|e| CowenError::Store(e.to_string()))?;
        let conn = client
            .get_multiplexed_tokio_connection()
            .await
            .map_err(|e| CowenError::Store(e.to_string()))?;
        Ok(Arc::new(RedisStore::new(conn, redis_url)))
    }
    #[cfg(not(feature = "redis"))]
    {
        return Err(CowenError::Store("Redis feature not enabled".to_string()));
    }
}

pub async fn create_store_from_url(
    url: &str,
    app_dir: &std::path::Path,
    fingerprint: &str,
) -> CowenResult<Arc<dyn Store>> {
    if url == "local" {
        return handle_local_store(app_dir, fingerprint).await;
    }

    let mut actual_url = resolve_innerdb_url(url, app_dir);
    actual_url = normalize_sqlite_url(actual_url, app_dir);

    let scheme = if actual_url.starts_with("sqlite:") {
        "sqlite".to_string()
    } else {
        actual_url
            .split(':')
            .next()
            .ok_or_else(|| CowenError::api("Invalid database URL"))?
            .to_string()
    };

    if scheme == "redis" {
        return handle_redis_url(&actual_url).await;
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

    Err(CowenError::api(format!(
        "Unsupported database scheme: {}.",
        scheme
    )))
}

async fn create_primary_store(
    app_cfg: &cowen_common::config::AppConfig,
    app_dir: &std::path::Path,
    fingerprint: &str,
) -> CowenResult<Arc<dyn Store>> {
    let store_type = &app_cfg.storage.store;
    if store_type == "local" {
        create_store_from_url(store_type, app_dir, fingerprint).await
    } else if store_type == "innerdb" || store_type == "sqlite" {
        let url = app_cfg
            .storage
            .db_url
            .as_ref()
            .cloned()
            .unwrap_or_else(|| "innerdb".to_string());
        create_store_from_url(&url, app_dir, fingerprint).await
    } else {
        let url = app_cfg.storage.db_url.as_ref().ok_or_else(|| {
            CowenError::store(format!(
                "Database URL is missing for distributed store: {}",
                store_type
            ))
        })?;
        create_store_from_url(url, app_dir, fingerprint).await
    }
}

async fn create_sensitive_store(
    app_cfg: &cowen_common::config::AppConfig,
    app_dir: &std::path::Path,
    fingerprint: &str,
    primary: &Arc<dyn Store>,
) -> CowenResult<Arc<dyn Store>> {
    let store_type = &app_cfg.storage.store;
    if let Some(url) = &app_cfg.storage.db_url {
        if store_type != "local" {
            let mut is_same_db = false;
            if store_type == "innerdb" || store_type == "sqlite" {
                is_same_db = true;
            } else if let Some(p_url) = app_cfg.storage.db_url.as_ref() {
                if url == p_url {
                    is_same_db = true;
                }
            }

            if is_same_db {
                Ok(primary.clone())
            } else {
                create_store_from_url(url, app_dir, fingerprint).await
            }
        } else {
            create_store_from_url(url, app_dir, fingerprint).await
        }
    } else {
        Ok(primary.clone())
    }
}

pub async fn create_vault(
    app_cfg: &cowen_common::config::AppConfig,
    app_dir: &std::path::Path,
    fingerprint: &str,
) -> CowenResult<Arc<dyn cowen_common::vault::Vault>> {
    let primary = create_primary_store(app_cfg, app_dir, fingerprint).await?;
    let sensitive = create_sensitive_store(app_cfg, app_dir, fingerprint, &primary).await?;

    let vault = Arc::new(StoreVault::new(primary, sensitive));
    if let Err(e) = cowen_common::vault::Vault::migrate(vault.as_ref()).await {
        eprintln!("⚠️ Failed to run store migrations: {}", e);
        tracing::error!(target: "sys", "Failed to run store migrations: {}", e);
    }
    Ok(vault)
}
pub mod reset;
