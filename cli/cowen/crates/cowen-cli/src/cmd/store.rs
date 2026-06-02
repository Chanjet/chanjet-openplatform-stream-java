use anyhow::Result;
use cowen_common::AppConfig;
use cowen_config::ConfigManager;
use crate::Colorize;

pub async fn set(
    app_config: &mut AppConfig,
    cfg_mgr: &ConfigManager,
    store: &Option<String>,
    db_url: &Option<String>,
    cache: &Option<String>,
    cache_url: &Option<String>,
) -> Result<()> {
    let mut changed = false;

    if let Some(s) = store {
        if s != "local" && s != "innerdb" && s != "sqlite" && s != "mysql" && s != "postgres" && s != "mssql" && s != "redis" {
            return Err(anyhow::anyhow!("Unsupported store type: {}. Supported: local, innerdb, sqlite, mysql, postgres, mssql, redis", s));
        }
        app_config.storage.store = s.clone();
        changed = true;
    }

    if let Some(url) = db_url {
        app_config.storage.db_url = Some(url.clone());
        changed = true;
    }

    if let Some(c) = cache {
        if c != "none" && c != "redis" && c != "memory" {
            return Err(anyhow::anyhow!("Unsupported cache type: {}. Supported: none, redis, memory", c));
        }
        app_config.storage.cache = c.clone();
        changed = true;
    }

    if let Some(url) = cache_url {
        app_config.storage.cache_url = Some(url.clone());
        changed = true;
    }

    if changed {
        cfg_mgr.save_app_config(app_config).await.map_err(|e| anyhow::anyhow!(e))?;
        println!("✨ Storage configuration updated successfully. Please restart daemon to apply.");
    } else {
        println!("ℹ️ No changes provided. Run with --help to see available options.");
    }

    Ok(())
}

pub async fn status(app_config: &AppConfig) -> Result<()> {
    let storage = &app_config.storage;

    println!("\n{}", "Storage Configuration Status".bold().underline());
    println!("  Type:  {}", storage.store.cyan());
    if let Some(url) = &storage.db_url {
        println!("  URL:   {}", url);
    }
    println!("  Cache: {}", storage.cache.cyan());
    if let Some(url) = &storage.cache_url {
        println!("  URL:   {}", url);
    }
    println!();
    
    println!("ℹ️ Storage connectivity is verified by daemon at runtime.");

    Ok(())
}

pub async fn migrate(
    _cfg_mgr: &ConfigManager,
    _to: &str,
    _mode: String,
    _daemon_svc: std::sync::Arc<dyn cowen_common::daemon::DaemonService>,
) -> Result<()> {
    Err(anyhow::anyhow!("Migration via CLI is deprecated. Please use daemon IPC or offline tools."))
}
