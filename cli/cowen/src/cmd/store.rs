use anyhow::Result;
use cowen_common::{ConfigManager, AppConfig};
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
    let storage = &app_config.storage;

    println!("\n{}", "Storage Configuration Status".bold().underline());
    println!("  Type:  {}", storage.store.cyan());
    if let Some(url) = &storage.db_url {
        println!("  URL:   {}", cowen_common::utils::mask_url_query(url));
    }
    println!("  Cache: {}", storage.cache.cyan());
    if let Some(url) = &storage.cache_url {
        println!("  URL:   {}", cowen_common::utils::mask_url_query(url));
    }
    println!();

    // 1. Check Primary Store
    if storage.store == "local" {
        println!("  {} Store: Local storage is always available.", "✅".green());
    } else {
        match check_db_connectivity(storage.store.as_str(), storage.db_url.as_deref()).await {
            Ok(_) => println!("  {} Store: Connected to {} successfully.", "✅".green(), storage.store),
            Err(e) => println!("  {} Store: Failed to connect to {}. Reason: {}", "❌".red(), storage.store, e),
        }
    }

    // 2. Check Cache
    if storage.cache == "none" {
        println!("  {} Cache: No cache configured.", "ℹ️".blue());
    } else {
        match check_cache_connectivity(storage.cache.as_str(), storage.cache_url.as_deref()).await {
            Ok(_) => println!("  {} Cache: Connected successfully.", "✅".green()),
            Err(e) => println!("  {} Cache: Failed to connect. Reason: {}", "❌".red(), e),
        }
    }

    println!();
    Ok(())
}

async fn check_db_connectivity(store_type: &str, url: Option<&str>) -> Result<()> {
    if store_type == "local" || store_type == "innerdb" {
        return Ok(());
    }
    let url = url.ok_or_else(|| anyhow::anyhow!("Database URL is missing"))?;
    let _ = cowen_store::create_store_from_url(url, &cowen_common::config::get_app_dir(), &cowen_common::security::get_machine_fingerprint()?).await
        .map_err(|e| anyhow::anyhow!("Failed to connect to database: {}", e))?;
    Ok(())
}

async fn check_cache_connectivity(cache_type: &str, url: Option<&str>) -> Result<()> {
    if cache_type == "none" {
        return Ok(());
    }
    let url = url.ok_or_else(|| anyhow::anyhow!("Cache URL is missing"))?;
    let _ = cowen_store::create_store_from_url(url, &cowen_common::config::get_app_dir(), &cowen_common::security::get_machine_fingerprint()?).await
        .map_err(|e| anyhow::anyhow!("Failed to connect to cache: {}", e))?;
    Ok(())
}

pub async fn migrate(
    cfg_mgr: &ConfigManager,
    to: &str,
    mode: cowen_store::migration::MigrationMode,
) -> Result<()> {
    let app_dir = cowen_common::config::get_app_dir();
    let fingerprint = cowen_common::security::get_machine_fingerprint()?;
    cowen_store::migration::perform_migration(cfg_mgr, to, mode, &app_dir, &fingerprint).await
}
