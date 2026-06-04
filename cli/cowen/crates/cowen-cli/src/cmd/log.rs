use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::time::Duration;
use std::fs::File;
use cowen_common::grpc::client::DaemonResponse;

pub async fn list(profile: &str) -> anyhow::Result<()> {
    let app_dir = cowen_common::config::get_app_dir();
    let log_dir = app_dir.join("logs");

    if !log_dir.exists() {
        println!("📂 No logs found.");
        return Ok(());
    }

    println!("\n📂 Log Files for profile '{}':", profile);
    let profile_log_prefix = format!("{}_", profile);

    for entry in std::fs::read_dir(log_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            
            // Only show logs starting with current profile name
            if filename.starts_with(&profile_log_prefix) {
                let metadata = std::fs::metadata(&path)?;
                println!("- {:<20} ({:>10} bytes)", filename, metadata.len());
            }
        }
    }
    println!();
    
    // Since we don't have Vault here, we just remind the user they can tail audit logs via daemon
    println!("🗄️ Store-based Audit Logs:");
    println!("- {:<20} (Managed by Daemon)", "audit (store)");
    println!();
    
    Ok(())
}

pub async fn view(
    profile: &str, 
    domain: &str, 
    follow: bool, 
    lines: usize,
) -> anyhow::Result<()> {
    if domain == "audit" {
        let ipc = cowen_common::grpc::client::DaemonClient::new(cowen_common::config::get_ipc_port_path());
        match ipc.tail_audit(profile, lines).await {
            Ok(DaemonResponse::AuditData { content }) => {
                if !content.is_empty() {
                    println!("🔍 Reading audit logs from Daemon (Store)...");
                    println!("{}", content);
                    if follow {
                        println!("\n⚠️ 'follow' mode is not yet supported for Store-based logs.");
                    }
                    return Ok(());
                }
            }
            Ok(DaemonResponse::Error { message, .. }) => {
                eprintln!("❌ Failed to retrieve audit logs: {}", message);
                return Ok(());
            }
            Err(e) => {
                eprintln!("❌ IPC Error retrieving audit logs: {}", e);
                return Ok(());
            }
            _ => {
                eprintln!("❌ Unexpected response when retrieving audit logs");
                return Ok(());
            }
        }
    }
    let app_dir = cowen_common::config::get_app_dir();
    let log_dir = app_dir.join("logs");
    
    let log_path = log_dir.join(format!("{}_{}.log", profile, domain));
    if !log_path.exists() {
        println!("❌ Log file not found for profile '{}': {}_{}.log", profile, profile, domain);
        
        // List available files to help the user
        let prefix = format!("{}_", profile);
        if let Ok(entries) = std::fs::read_dir(log_dir) {
            let available: Vec<String> = entries.filter_map(|e| {
                let p = e.ok()?.path();
                if p.extension()? == "log" {
                    let stem = p.file_stem()?.to_string_lossy().to_string();
                    if stem.starts_with(&prefix) {
                        Some(stem[prefix.len()..].to_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }).collect();
            
            if !available.is_empty() {
                println!("💡 Available domains for profile '{}': {}", profile, available.join(", "));
            }
        }
        return Ok(());
    }

    let mut file = File::open(&log_path)?;
    let metadata = file.metadata()?;
    let len = metadata.len();

    // Simple tail: start from some bytes back
    let mut pos = len.saturating_sub(lines as u64 * 200);

    file.seek(SeekFrom::Start(pos))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();

    // Initial tail
    let mut last_lines = Vec::new();
    while reader.read_line(&mut line)? > 0 {
        last_lines.push(line.clone());
        line.clear();
    }
    
    // Print only the last N lines
    let start_idx = if last_lines.len() > lines { last_lines.len() - lines } else { 0 };
    for l in &last_lines[start_idx..] {
        print!("{}", l);
    }

    if !follow {
        return Ok(());
    }

    println!("\n👀 Following log [{}]... (Ctrl+C to stop)", domain);
    
    // Refresh reader to pick up new data
    pos = reader.get_ref().metadata()?.len();
    
    loop {
        let mut f = File::open(&log_path)?;
        let current_len = f.metadata()?.len();
        
        if current_len > pos {
            f.seek(SeekFrom::Start(pos))?;
            let mut r = BufReader::new(f);
            while r.read_line(&mut line)? > 0 {
                print!("{}", line);
                line.clear();
            }
            pos = current_len;
        } else if current_len < pos {
            // File might have been rotated
            println!("\n🔄 Log file appears to have been rotated. Resetting...");
            pos = 0;
        }
        
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
