use anyhow::Result;
use crate::daemon::dlq::DlqStore;
use crate::daemon::forwarder::Forwarder;
use std::sync::Arc;
use crate::core::config::Config;

pub async fn list(profile: &str) -> Result<()> {
    let dlq_store = DlqStore::new(profile)?;
    let entries = dlq_store.list()?;

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

pub async fn retry(profile: &str, config: &Config, id: &str) -> Result<()> {
    let dlq_store = DlqStore::new(profile)?;
    let entry = dlq_store.get(id)?;

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
    dlq_arc.delete(id)?;
    println!("🗑️ Original DLQ entry [{}] removed.", id);

    Ok(())
}

pub async fn purge(profile: &str) -> Result<()> {
    let dlq_store = DlqStore::new(profile)?;
    let entries = dlq_store.list()?;

    if entries.is_empty() {
        println!("✅ DLQ is already empty.");
        return Ok(());
    }

    println!("⚠️ Purging {} entries from DLQ...", entries.len());
    for entry in entries {
        dlq_store.delete(&entry.id)?;
    }
    println!("✅ DLQ purged.");

    Ok(())
}
