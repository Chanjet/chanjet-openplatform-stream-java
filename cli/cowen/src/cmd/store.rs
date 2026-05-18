use anyhow::Result;
use cowen_common::AppConfig;
use cowen_config::ConfigManager;
use cowen_store::migration::{StoreMigrator, MigrationMode};
use colored::Colorize;

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
        if !cowen_store::SqlStore::is_supported(s) && s != "local" && s != "redis" {
            return Err(anyhow::anyhow!("Unsupported store type: {}. Supported: local, innerdb, sqlite, mysql, postgres, mssql, redis", s));
        }
        app_config.storage.store = s.clone();
        changed = true;
    }

    if let Some(url) = db_url {
        // Validation attempt
        check_db_connectivity(&app_config.storage.store, Some(url)).await?;
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
        check_cache_connectivity(&app_config.storage.cache, Some(url)).await?;
        app_config.storage.cache_url = Some(url.clone());
        changed = true;
    }

    if changed {
        cfg_mgr.save_app_config(app_config).await.map_err(|e| anyhow::anyhow!(e))?;
        println!("✨ Storage configuration updated successfully.");
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
        println!("  URL:   {}", cowen_infra::mask_url_query(url));
    }
    println!("  Cache: {}", storage.cache.cyan());
    if let Some(url) = &storage.cache_url {
        println!("  URL:   {}", cowen_infra::mask_url_query(url));
    }
    println!();

    // 1. Check Primary Store
    if storage.store == "local" {
        println!("✅ Local filesystem storage is active.");
    } else {
        match check_db_connectivity(&storage.store, storage.db_url.as_deref()).await {
            Ok(_) => println!("✅ Primary database connection is healthy."),
            Err(e) => println!("❌ Primary database connection failed: {}", e),
        }
    }

    // 2. Check Cache
    if storage.cache == "none" {
        println!("ℹ️ Cache is disabled.");
    } else if storage.cache == "memory" {
        println!("✅ In-memory cache is active.");
    } else {
        match check_cache_connectivity(&storage.cache, storage.cache_url.as_deref()).await {
            Ok(_) => println!("✅ Cache connection is healthy."),
            Err(e) => println!("❌ Cache connection failed: {}", e),
        }
    }

    Ok(())
}

async fn check_db_connectivity(store_type: &str, url: Option<&str>) -> Result<()> {
    if store_type == "local" || store_type == "innerdb" {
        return Ok(());
    }
    let url = url.ok_or_else(|| anyhow::anyhow!("Database URL is missing"))?;
    let _ = cowen_store::create_store_from_url(url, &cowen_infra::get_app_dir(), &cowen_common::security::get_machine_fingerprint().map_err(|e| anyhow::anyhow!(e))?).await
        .map_err(|e| anyhow::anyhow!("Failed to connect to database: {}", e))?;
    Ok(())
}

async fn check_cache_connectivity(cache_type: &str, url: Option<&str>) -> Result<()> {
    if cache_type == "none" {
        return Ok(());
    }
    let url = url.ok_or_else(|| anyhow::anyhow!("Cache URL is missing"))?;
    let _ = cowen_store::create_store_from_url(url, &cowen_infra::get_app_dir(), &cowen_common::security::get_machine_fingerprint().map_err(|e| anyhow::anyhow!(e))?).await
        .map_err(|e| anyhow::anyhow!("Failed to connect to cache: {}", e))?;
    Ok(())
}

pub async fn migrate(
    cfg_mgr: &ConfigManager,
    to: &str,
    mode: MigrationMode,
) -> Result<()> {
    let _app_dir = cowen_infra::get_app_dir();
    let _fingerprint = cowen_common::security::get_machine_fingerprint().map_err(|e| anyhow::anyhow!(e))?;
    let vault = cfg_mgr.get_vault().ok_or_else(|| anyhow::anyhow!("Vault not initialized"))?;
    let source = vault.primary_store();
    let migrator = StoreMigrator::new(source);
    migrator.migrate(cfg_mgr, to, mode).await.map_err(|e| anyhow::anyhow!(e))?;
    let profiles = cfg_mgr.list_profiles().await.map_err(|e| anyhow::anyhow!(e))?;

    // 🚀 STABILITY: After migration, all active daemons MUST be restarted to pick up the new store.
    // Otherwise, they'll keep talking to the old store while the CLI talks to the new one.
    println!("🔄 Storage switched successfully. Restarting active daemons to apply changes...");
    
    for p in profiles {
        // We only care about standard profiles (not hidden app: ones) for daemon restarts
        if !p.starts_with("app:") {
            if let Some(_info) = cowen_monitor::status::get_active_daemon_info(&p) {
                if let Ok(config) = cfg_mgr.load(&p).await {
                    if !config.app_key.is_empty() {
                         println!("♻️  Restarting daemon for profile: {}", p);
                         let _ = cowen_server::restart(&p, &config, config.proxy_port, config.proxy_enabled, false, cfg_mgr, vault.clone()).await;
                    }
                }
            }
        }
    }
    
    Ok(())
}
