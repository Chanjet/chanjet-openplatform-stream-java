use crate::core::config::{AppConfig, ConfigManager};
use anyhow::Result;
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
        app_config.storage.store = s.clone();
        changed = true;
    }
    if let Some(du) = db_url {
        app_config.storage.db_url = Some(du.clone());
        changed = true;
    }
    if let Some(c) = cache {
        app_config.storage.cache = c.clone();
        changed = true;
    }
    if let Some(cu) = cache_url {
        app_config.storage.cache_url = Some(cu.clone());
        changed = true;
    }

    if changed {
        cfg_mgr.save_app_config(app_config).await?;
        println!("{}", "✅ Global storage configuration updated successfully.".green());
        // Trigger a status check immediately to verify the new settings
        status(app_config).await?;
    } else {
        println!("No changes provided. Use flags like --store, --db-url, etc. to update configuration.");
    }

    Ok(())
}

pub async fn status(app_config: &AppConfig) -> Result<()> {
    println!("\n{}", "🌐 Global Storage Status".bold().cyan());
    println!("{}", "-----------------------".cyan());
    
    let storage = &app_config.storage;
    println!("{:<15} {}", "Store Type:".bold(), storage.store);
    if let Some(url) = &storage.db_url {
        println!("{:<15} {}", "DB URL:".bold(), url);
    }
    println!("{:<15} {}", "Cache Type:".bold(), storage.cache);
    if let Some(url) = &storage.cache_url {
        println!("{:<15} {}", "Cache URL:".bold(), url);
    }

    println!("\n{}", "🔍 Connectivity Check".bold().cyan());
    
    // 1. Check Store
    if storage.store == "local" {
        println!("  {} Store: Local filesystem is always available.", "✅".green());
    } else {
        match check_db_connectivity(storage.store.as_str(), storage.db_url.as_deref()).await {
            Ok(_) => println!("  {} Store: Connected to {} successfully.", "✅".green(), storage.store),
            Err(e) => println!("  {} Store: Failed to connect to {}. Reason: {}", "❌".red(), storage.store, e),
        }
    }

    // 2. Check Cache
    if storage.cache == "none" {
        println!("  {} Cache: No cache configured.", "ℹ️".blue());
    } else if storage.cache == "redis" {
        match check_redis_connectivity(storage.cache_url.as_deref()).await {
            Ok(_) => println!("  {} Cache: Connected to Redis successfully.", "✅".green()),
            Err(e) => println!("  {} Cache: Failed to connect to Redis. Reason: {}", "❌".red(), e),
        }
    }

    println!();
    Ok(())
}

async fn check_db_connectivity(store_type: &str, url: Option<&str>) -> Result<()> {
    let url = url.ok_or_else(|| anyhow::anyhow!("Database URL is missing"))?;
    
    // Use the same logic as main.rs create_vault to verify connectivity
    match store_type {
        _ if crate::core::store::sql::SqlStore::is_supported(store_type) => {
            // We use the SqlStore's from_url which performs a connection check
            crate::core::store::sql::SqlStore::from_url(url).await?;
            Ok(())
        }
        _ => Err(anyhow::anyhow!("Unsupported distributed store type: {}", store_type)),
    }
}
async fn check_cache_connectivity(cache_type: &str, url: Option<&str>) -> Result<()> {
    if cache_type == "none" {
        return Ok(());
    }

    let url = url.ok_or_else(|| anyhow::anyhow!("Cache URL is missing"))?;
    
    let mut found = false;
    for reg in inventory::iter::<crate::core::store::CacheBuilderRegistration> {
        if reg.builder.scheme() == cache_type {
            // For now, only Redis needs connectivity check here. 
            // In a more generic way, builders could provide a 'check' method.
            if cache_type == "redis" {
                let client = redis::Client::open(url)?;
                let mut conn = client.get_multiplexed_tokio_connection().await?;
                let _: () = redis::cmd("PING").query_async(&mut conn).await?;
            }
            found = true;
            break;
        }
    }

    if found {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Unsupported cache type: {}", cache_type))
    }
}

pub async fn migrate(
    cfg_mgr: &crate::core::config::ConfigManager,
    to: &str,
    mode: crate::core::migration::MigrationMode,
) -> Result<()> {
    crate::core::migration::perform_migration(cfg_mgr, to, mode).await
}
