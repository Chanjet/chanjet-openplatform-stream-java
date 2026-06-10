use anyhow::Result;
use cowen_common::grpc::client::DaemonResponse;

pub async fn list(profile: &str, format: &str, page: usize, page_size: usize) -> Result<()> {
    let ipc = cowen_common::grpc::client::DaemonClient::new(crate::get_ipc_port_path());
    match ipc.dlq_list(profile, page, page_size).await {
        Ok(DaemonResponse::DlqData { json }) => {
            if format == "json" || format == "yaml" {
                let val: serde_json::Value = serde_json::from_str(&json)?;
                cowen_common::utils::render(&val, format).map_err(|e| anyhow::anyhow!(e))?;
                return Ok(());
            }
            let entries: Vec<serde_json::Value> = serde_json::from_str(&json)?;
            if entries.is_empty() {
                if page > 1 {
                    println!(
                        "✅ No more entries in DLQ for profile '{}' at page {}",
                        profile, page
                    );
                } else {
                    println!("✅ DLQ is empty for profile '{}'", profile);
                }
                return Ok(());
            }
            println!(
                "\n📥 Dead Letter Queue (Profile: {}, Page: {})",
                profile, page
            );
            println!("--------------------------------------------------");
            for entry in entries {
                let id = entry["id"].as_i64().unwrap_or(0);
                let created_at = entry["created_at"].as_str().unwrap_or("");
                let topic = entry["topic"].as_str().unwrap_or("");
                let retry_count = entry["retry_count"].as_i64().unwrap_or(0);
                println!(
                    "[ID: {}] [{}] {} - Retry: {}",
                    id, created_at, topic, retry_count
                );
                if let Some(err) = entry["error"].as_str() {
                    println!("   \x1b[31mError: {}\x1b[0m", err);
                }
                println!();
            }
        }
        Ok(DaemonResponse::Error { message, .. }) => eprintln!("❌ DLQ List failed: {}", message),
        Err(e) => eprintln!("❌ IPC Error: {}", e),
        _ => eprintln!("❌ Unexpected response"),
    }
    Ok(())
}

pub async fn view(profile: &str, id: String) -> Result<()> {
    let ipc = cowen_common::grpc::client::DaemonClient::new(crate::get_ipc_port_path());
    match ipc.dlq_view(profile, &id).await {
        Ok(DaemonResponse::DlqData { json }) => {
            let entry: serde_json::Value = serde_json::from_str(&json)?;
            println!("\n🔍 \x1b[1;36mDLQ Entry Details\x1b[0m");
            println!("--------------------------------------------------");
            println!("  ID:          {}", entry["id"].as_i64().unwrap_or(0));
            println!("  Topic:       {}", entry["topic"].as_str().unwrap_or(""));
            println!(
                "  Created:     {}",
                entry["created_at"].as_str().unwrap_or("")
            );
            println!(
                "  Retry Count: {}",
                entry["retry_count"].as_i64().unwrap_or(0)
            );
            println!("--------------------------------------------------");
            println!("\x1b[1;33mPayload:\x1b[0m");
            if let Some(payload_str) = entry["payload"].as_str() {
                match serde_json::from_str::<serde_json::Value>(payload_str) {
                    Ok(parsed_payload) => println!(
                        "{}",
                        serde_json::to_string_pretty(&parsed_payload).unwrap_or_default()
                    ),
                    Err(_) => println!("{}", payload_str),
                }
            }
            if let Some(error_str) = entry["error"].as_str() {
                if !error_str.is_empty() {
                    println!("--------------------------------------------------");
                    println!("\x1b[1;31mError Context:\x1b[0m");
                    println!("{}", error_str);
                }
            }
            println!();
        }
        Ok(DaemonResponse::Error { message, .. }) => eprintln!("❌ DLQ View failed: {}", message),
        Err(e) => eprintln!("❌ IPC Error: {}", e),
        _ => eprintln!("❌ Unexpected response"),
    }
    Ok(())
}

pub async fn retry(profile: &str, id: String) -> Result<()> {
    println!("🔄 Retrying DLQ message {}...", id);
    let ipc = cowen_common::grpc::client::DaemonClient::new(crate::get_ipc_port_path());
    match ipc.dlq_retry(profile, &id).await {
        Ok(DaemonResponse::Success { message }) => println!("✅ {}", message),
        Ok(DaemonResponse::Error { message, .. }) => eprintln!("❌ DLQ Retry failed: {}", message),
        Err(e) => eprintln!("❌ IPC Error: {}", e),
        _ => eprintln!("❌ Unexpected response"),
    }
    Ok(())
}

pub async fn purge(profile: &str) -> Result<()> {
    println!("⚠️ Purging DLQ...");
    let ipc = cowen_common::grpc::client::DaemonClient::new(crate::get_ipc_port_path());
    match ipc.dlq_purge(profile).await {
        Ok(DaemonResponse::Success { message }) => println!("✅ {}", message),
        Ok(DaemonResponse::Error { message, .. }) => eprintln!("❌ DLQ Purge failed: {}", message),
        Err(e) => eprintln!("❌ IPC Error: {}", e),
        _ => eprintln!("❌ Unexpected response"),
    }
    Ok(())
}
