use anyhow::Result;
use cowen_server::daemon::dlq::DlqStore;
use cowen_server::daemon::forwarder::Forwarder;
use std::sync::Arc;
use cowen_common::Config;
use cowen_auth::client::Client;

pub async fn list(profile: &str, config: &Config, format: &str, vault: Arc<dyn cowen_common::vault::Vault>) -> Result<()> {
    let auth = cowen_auth::create_auth_client_with_vault(vault.clone());
    if !auth.supports_webhooks(config) {
        println!("⚠️  Mode '{:?}' does not support Webhooks/Streaming, DLQ is disabled.", config.app_mode);
        return Ok(());
    }

    let dlq_store = DlqStore::new(profile, vault)?;
    let entries = dlq_store.list().await?;

    if format == "json" || format == "yaml" {
        return cowen_common::utils::render(&entries, format);
    }

    if entries.is_empty() {
        println!("✅ DLQ is empty for profile '{}'", profile);
        return Ok(());
    }

    println!("\n📦 Dead Letter Queue ({} entries):", entries.len());
    println!("{:<38} {:<10} {:<20} {}", "ID", "TYPE", "CREATED AT", "ERROR");
    println!("{}", "-".repeat(100));

    for entry in entries {
        println!("{:<38} {:<10} {:<20} {}", 
            entry.id, 
            entry.msg_type, 
            entry.created_at.format("%Y-%m-%d %H:%M:%S"), 
            entry.error
        );
    }
    println!("{}", "-".repeat(100));
    println!("(TIP: Run 'dlq retry <ID>' or 'dlq purge')\n");

    Ok(())
}

pub async fn retry(profile: &str, config: &Config, id: &str, vault: Arc<dyn cowen_common::vault::Vault>) -> Result<()> {
    let auth = cowen_auth::create_auth_client_with_vault(vault.clone());
    if !auth.supports_webhooks(config) {
        println!("⚠️  Mode '{:?}' does not support Webhooks/Streaming, DLQ is disabled.", config.app_mode);
        return Ok(());
    }

    let dlq_store = DlqStore::new(profile, vault)?;
    let entry = dlq_store.get(id).await?;

    println!("🔄 Retrying event [{}] ({})", entry.msg_type, entry.id);

    let payload: serde_json::Value = serde_json::from_str(&entry.payload)?;
    let dlq_arc = Arc::new(dlq_store);
    let forwarder = Forwarder::new(dlq_arc.clone(), &config.webhook_target);

    forwarder.forward(payload).await;

    // If forwarding was successful (or at least attempted), we should probably delete the old entry
    // if the user wants it. For now, let's just let it stay or provide a flag.
    // In Go version, retry usually deletes if successful.
    // Our forwarder.forward handles saving TO dlq if it fails again.
    
    // We'll delete it from original store to avoid duplicates if it's being retried manually.
    dlq_arc.delete(id).await?;
    println!("🗑️ Original DLQ entry [{}] removed.", id);

    Ok(())
}

pub async fn purge(profile: &str, config: &Config, vault: Arc<dyn cowen_common::vault::Vault>) -> Result<()> {
    let auth = cowen_auth::create_auth_client_with_vault(vault.clone());
    if !auth.supports_webhooks(config) {
        println!("⚠️  Mode '{:?}' does not support Webhooks/Streaming, DLQ is disabled.", config.app_mode);
        return Ok(());
    }

    let dlq_store = DlqStore::new(profile, vault)?;
    let entries = dlq_store.list().await?;

    if entries.is_empty() {
        println!("✅ DLQ is already empty.");
        return Ok(());
    }

    println!("⚠️ Purging {} entries from DLQ...", entries.len());
    for entry in entries {
        dlq_store.delete(&entry.id).await?;
    }
    println!("✅ DLQ purged.");

    Ok(())
}
