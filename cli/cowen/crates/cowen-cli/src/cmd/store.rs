use anyhow::Result;
use crate::Colorize;

pub async fn set(
    store: &Option<String>,
    db_url: &Option<String>,
    cache: &Option<String>,
    cache_url: &Option<String>,
) -> Result<()> {
    let port_path = crate::get_ipc_port_path();
    let ipc = cowen_common::grpc::client::DaemonClient::new(port_path);
    let mut changed = false;

    if let Some(s) = store {
        if s != "local" && s != "innerdb" && s != "sqlite" && s != "mysql" && s != "postgres" && s != "mssql" && s != "redis" {
            return Err(anyhow::anyhow!("Unsupported store type: {}. Supported: local, innerdb, sqlite, mysql, postgres, mssql, redis", s));
        }
        let _ = ipc.set_global_config("storage.store", s).await;
        changed = true;
    }

    if let Some(url) = db_url {
        let _ = ipc.set_global_config("storage.db_url", url).await;
        changed = true;
    }

    if let Some(c) = cache {
        if c != "none" && c != "redis" && c != "memory" {
            return Err(anyhow::anyhow!("Unsupported cache type: {}. Supported: none, redis, memory", c));
        }
        let _ = ipc.set_global_config("storage.cache", c).await;
        changed = true;
    }

    if let Some(url) = cache_url {
        let _ = ipc.set_global_config("storage.cache_url", url).await;
        changed = true;
    }

    if changed {
        println!("✨ Storage configuration updated successfully. Please restart daemon to apply.");
    } else {
        println!("ℹ️ No changes provided. Run with --help to see available options.");
    }

    Ok(())
}

pub async fn status() -> Result<()> {
    let port_path = crate::get_ipc_port_path();
    let ipc = cowen_common::grpc::client::DaemonClient::new(port_path);

    let storage: cowen_common::config::StorageConfig = match ipc.store_status().await {
        Ok(cowen_common::grpc::client::DaemonResponse::StoreStatusData { json }) => serde_json::from_str(&json).unwrap_or_default(),
        _ => return Err(anyhow::anyhow!("Failed to retrieve storage status from daemon")),
    };

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
    _to: &str,
    _mode: String,
) -> Result<()> {
    Err(anyhow::anyhow!("Migration via CLI is deprecated. Please use daemon IPC or offline tools."))
}
