use anyhow::Result;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::time::Duration;
use std::fs::File;

pub async fn list(profile: &str) -> Result<()> {
    let app_dir = crate::core::config::get_app_dir();
    let log_dir = app_dir.join("logs");

    if !log_dir.exists() {
        println!("📂 No logs found.");
        return Ok(());
    }

    println!("\n📂 Log Files for profile '{}':", profile);
    for entry in std::fs::read_dir(log_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let metadata = std::fs::metadata(&path)?;
            println!("- {:<20} ({:>10} bytes)", 
                path.file_name().and_then(|s| s.to_str()).unwrap_or(""),
                metadata.len()
            );
        }
    }
    println!();
    Ok(())
}

pub async fn view(_profile: &str, domain: &str, follow: bool, lines: usize) -> Result<()> {
    let app_dir = crate::core::config::get_app_dir();
    let log_path = app_dir.join("logs").join(format!("{}.log", domain));

    if !log_path.exists() {
        println!("❌ Log file not found: {:?}", log_path);
        return Ok(());
    }

    let mut file = File::open(&log_path)?;
    let metadata = file.metadata()?;
    let len = metadata.len();

    // Simple tail: start from some bytes back
    let mut pos = if len > (lines as u64 * 200) {
        len - (lines as u64 * 200)
    } else {
        0
    };

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
