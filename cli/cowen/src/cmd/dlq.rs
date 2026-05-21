use anyhow::Result;
use cowen_server::daemon::dlq::DlqStore;
use std::sync::Arc;
use cowen_common::Config;
use cowen_auth::client::Client;

pub async fn list(profile: &str, config: &Config, format: &str, page: usize, page_size: usize, vault: Arc<dyn cowen_common::vault::Vault>) -> Result<()> {
    let auth = cowen_auth::create_auth_client_with_vault(vault.clone());
    if !auth.supports_webhooks(config) {
        println!("⚠️  Mode '{:?}' does not support Webhooks/Streaming, DLQ is disabled.", config.app_mode);
        return Ok(());
    }

    let dlq_store = DlqStore::new(profile, vault).map_err(|e| anyhow::anyhow!(e))?;
    let entries = dlq_store.list_paged(page, page_size).await.map_err(|e| anyhow::anyhow!(e))?;

    if format == "json" || format == "yaml" {
        cowen_common::utils::render(&entries, format).map_err(|e| anyhow::anyhow!(e))?;
        return Ok(());
    }

    if entries.is_empty() {
        if page > 1 {
            println!("✅ No more entries in DLQ for profile '{}' at page {}", profile, page);
        } else {
            println!("✅ DLQ is empty for profile '{}'", profile);
        }
        return Ok(());
    }

    println!("\n📥 Dead Letter Queue (Profile: {}, Page: {})", profile, page);
    println!("--------------------------------------------------");
    for entry in entries {
        println!("[ID: {}] [{}] {} - Retry: {}", entry.id, entry.created_at, entry.topic, entry.retry_count);
        if let Some(err) = &entry.error {
            println!("   \x1b[31mError: {}\x1b[0m", err);
        }
        println!();
    }

    Ok(())
}

pub async fn retry(profile: &str, config: &Config, id: String, vault: Arc<dyn cowen_common::vault::Vault>) -> Result<()> {
    let dlq_store = DlqStore::new(profile, vault).map_err(|e| anyhow::anyhow!(e))?;
    let entry_id = id.parse::<i64>().map_err(|_| anyhow::anyhow!("Invalid DLQ entry ID"))?;
    
    let forwarder = cowen_server::daemon::forwarder::Forwarder::new(profile, config.clone(), dlq_store.vault().clone())
        .map_err(|e| anyhow::anyhow!("Failed to initialize forwarder: {}", e))?;
    
    println!("🔄 Retrying DLQ message {}...", id);
    forwarder.retry_message(entry_id).await.map_err(|e| anyhow::anyhow!(e))?;
    println!("✅ Message retried successfully.");

    Ok(())
}

pub async fn purge(profile: &str, _config: &Config, vault: Arc<dyn cowen_common::vault::Vault>) -> Result<()> {
    let dlq_store = DlqStore::new(profile, vault).map_err(|e| anyhow::anyhow!(e))?;
    let entries = dlq_store.list_all().await.map_err(|e| anyhow::anyhow!(e))?;

    if entries.is_empty() {
        println!("✅ DLQ is already empty.");
        return Ok(());
    }

    println!("⚠️ Purging {} entries from DLQ...", entries.len());
    for entry in entries {
        dlq_store.delete(entry.id, &entry.topic).await.map_err(|e| anyhow::anyhow!(e))?;
    }
    println!("✅ DLQ purged.");

    Ok(())
}
