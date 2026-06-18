use anyhow::Result;
use cowen_common::grpc::client::DaemonResponse;

use serde_json::Value;

fn handle_json_yaml_format(json: &str, format: &str, all: bool) -> Result<()> {
    let val: Value = serde_json::from_str(json)?;
    if !all {
        if let Some(arr) = val.as_array() {
            if arr.len() == 1 {
                cowen_common::utils::render(&arr[0], format).map_err(|e| anyhow::anyhow!(e))?;
                return Ok(());
            }
        }
    }
    cowen_common::utils::render(&val, format).map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

async fn print_human_readable_status(
    ipc: &cowen_common::grpc::client::DaemonClient,
    json: &str,
) -> Result<()> {
    let results: Vec<Value> = serde_json::from_str(json)?;

    println!("\n🔍 COWEN System Status Diagnostics");
    println!("----------------------------------");
    println!("Build ID:      {}", cowen_common::BUILD_ID);
    println!("Build Time:    {}", cowen_common::BUILD_TIME);
    println!();

    let mut store_mode = "unknown".to_string();
    if let Ok(DaemonResponse::StoreStatusData { json: store_json }) = ipc.store_status().await {
        if let Ok(store_val) = serde_json::from_str::<Value>(&store_json) {
            if let Some(s) = store_val.get("store").and_then(|v| v.as_str()) {
                store_mode = s.to_string();
            }
        }
    }
    println!("📦 Storage: Mode: {} \x1b[32m(OK)\x1b[0m", store_mode);
    println!();

    if results.is_empty() {
        println!("👤 Profile: Not Initialized");
        println!("----------------------------------");
        println!("⚙️  System is not initialized. Please run `cowen auth login` or `cowen init` to configure a profile.\n");
    } else {
        for s in results {
            let prof = s["profile"].as_str().unwrap_or("");
            if prof.trim().is_empty() {
                continue;
            }
            println!("👤 Profile: '{}'", prof);
            println!("----------------------------------");
            if let Some(entries) = s["entries"].as_array() {
                for entry in entries {
                    render_json_entry(entry, 0);
                }
            }
            println!();
        }
    }
    Ok(())
}

pub async fn status(profile: &str, format: &str, all: bool) -> Result<()> {
    let port_path = crate::get_ipc_port_path();
    let ipc = cowen_common::grpc::client::DaemonClient::new(port_path);

    match ipc.system_status(profile, all).await {
        Ok(DaemonResponse::SystemStatusData { json }) => {
            if format == "json" || format == "yaml" {
                handle_json_yaml_format(&json, format, all)?;
            } else {
                print_human_readable_status(&ipc, &json).await?;
            }
        }
        Ok(DaemonResponse::Error { message, .. }) => {
            eprintln!("❌ Status fetch failed: {}", message)
        }
        Err(e) => eprintln!("❌ IPC Error: {}", e),
        _ => eprintln!("❌ Unexpected response"),
    }
    Ok(())
}

fn render_json_entry(entry: &Value, indent: usize) {
    let prefix = "  ".repeat(indent);
    let name = entry["name"].as_str().unwrap_or("");
    let icon = entry["icon"].as_str().unwrap_or("");
    let message = entry["message"].as_str().unwrap_or("");

    let level_str = match entry["level"].as_i64() {
        Some(0) => "\x1b[32m(OK)\x1b[0m",
        Some(1) => "\x1b[33m(WARN)\x1b[0m",
        Some(2) => "\x1b[31m(ERROR)\x1b[0m",
        _ => match entry["level"].as_str() {
            Some("OK") => "\x1b[32m(OK)\x1b[0m",
            Some("WARN") => "\x1b[33m(WARN)\x1b[0m",
            Some("ERROR") => "\x1b[31m(ERROR)\x1b[0m",
            _ => "(UNKNOWN)",
        },
    };

    println!("{}{} {}: {} {}", prefix, icon, name, message, level_str);

    if let Some(reason) = entry["reason"].as_str() {
        println!("{}   \x1b[31m╰─ Reason: {}\x1b[0m", prefix, reason);
    }

    if let Some(details) = entry["details"].as_array() {
        for d in details {
            if let Some(s) = d.as_str() {
                println!("{}   - {}", prefix, s);
            }
        }
    }

    if let Some(children) = entry["children"].as_array() {
        for c in children {
            render_json_entry(c, indent + 1);
        }
    }
}

pub async fn config(profile: &str, format: &str, all: bool) -> Result<()> {
    let port_path = crate::get_ipc_port_path();
    let ipc = cowen_common::grpc::client::DaemonClient::new(port_path);
    match ipc.list_config(profile, format, all).await {
        Ok(cowen_common::grpc::client::DaemonResponse::ConfigData {
            config_json: message,
        }) => {
            let message = cowen_common::utils::mask_sensitive_json(&message);
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&message) {
                let _ = cowen_common::utils::render(&val, format);
            } else {
                println!("{}", message);
            }
        }
        Ok(cowen_common::grpc::client::DaemonResponse::Error { message, .. }) => {
            eprintln!("❌ Fetch config failed: {}", message);
        }
        Err(e) => {
            eprintln!("❌ Fetch config failed: {}", e);
        }
        _ => eprintln!("❌ Unexpected response"),
    }
    Ok(())
}

fn dry_run_reset() {
    println!("🔍 [DRY RUN] Full Reset Execution Plan:");
    println!("  - Kill master daemon");
    println!("  - Delete all .db, .db-wal, .db-shm, .yaml, .lock, .pid files");
    println!("  - Delete logs/ and profiles/ directories");
}

fn kill_master_daemon(app_dir: &std::path::Path) {
    let daemon_pid_file = app_dir.join("master_daemon.pid");
    if daemon_pid_file.exists() {
        if let Ok(pid_str) = std::fs::read_to_string(&daemon_pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                let sys = sysinfo::System::new_with_specifics(
                    sysinfo::RefreshKind::nothing()
                        .with_processes(sysinfo::ProcessRefreshKind::nothing()),
                );
                if let Some(process) = sys.process(sysinfo::Pid::from_u32(pid)) {
                    process.kill();
                }
            }
        }
    }
}

fn clean_app_dir(app_dir: &std::path::Path) {
    if let Ok(entries) = std::fs::read_dir(app_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".db")
                    || name.ends_with(".db-wal")
                    || name.ends_with(".db-shm")
                    || name.ends_with(".ddl.lock")
                    || name.ends_with(".yaml")
                    || name.ends_with(".pid")
                {
                    let _ = std::fs::remove_file(entry.path());
                } else if name == "profiles" || name == "logs" {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }
    }
}

async fn execute_local_reset() {
    let app_dir = cowen_common::config::get_app_dir();
    kill_master_daemon(&app_dir);
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    clean_app_dir(&app_dir);
    println!("✅ System reset successful");
}

pub async fn reset(target_profile: Option<&str>, dry_run: bool) -> Result<()> {
    if target_profile.is_none() {
        if dry_run {
            dry_run_reset();
        } else {
            execute_local_reset().await;
        }
        return Ok(());
    }

    if !dry_run {
        let _ = ensure_daemon_running(target_profile.unwrap_or("")).await;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let port_path = crate::get_ipc_port_path();
    let ipc = cowen_common::grpc::client::DaemonClient::new(port_path);

    if !dry_run {
        if let Some(profile) = target_profile {
            let _ = ipc.stop_daemon(profile).await;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    let is_ipc_error = match ipc.system_reset(target_profile, dry_run).await {
        Ok(DaemonResponse::Success { message }) => {
            if dry_run {
                println!("{}", message);
            } else {
                println!("✅ {}", message);
            }
            false
        }
        Ok(DaemonResponse::Error { message, .. }) => {
            eprintln!("❌ Reset failed: {}", message);
            return Err(anyhow::anyhow!("Reset failed"));
        }
        Err(e) => {
            eprintln!("🔥 IPC system_reset error: {:?}", e);
            true
        }
        _ => {
            eprintln!("❌ Unexpected response");
            true
        }
    };

    if is_ipc_error {
        eprintln!("❌ Daemon not reachable. Reset cannot be performed locally in this CLI mode. Please start the daemon first.");
        return Err(anyhow::anyhow!("Reset failed: daemon unreachable"));
    }

    Ok(())
}

pub async fn rename_profile(old: &str, new: &str) -> Result<()> {
    let port_path = crate::get_ipc_port_path();
    let ipc = cowen_common::grpc::client::DaemonClient::new(port_path);

    match ipc.rename_profile(old, new).await {
        Ok(DaemonResponse::Success { message }) => println!("✅ {}", message),
        Ok(DaemonResponse::Error { message, .. }) => eprintln!("❌ Rename failed: {}", message),
        Err(e) => eprintln!("❌ IPC Error: {}", e),
        _ => eprintln!("❌ Unexpected response"),
    }
    Ok(())
}

pub async fn ensure_daemon_running(_profile: &str) -> Result<()> {
    let app_dir = cowen_common::config::get_app_dir();
    let stopped_file = app_dir.join("master_daemon.stopped");
    if stopped_file.exists() {
        return Ok(());
    }

    let port_path = crate::get_ipc_port_path();
    let ipc_client = cowen_common::grpc::client::DaemonClient::new(port_path);
    if ipc_client.ping().await.is_err() {
        eprintln!("⚠️  Daemon not running, triggering auto-recovery...");
    }
    if let Err(e) = ipc_client.ensure_daemon().await {
        eprintln!("❌ ensure_daemon failed: {}", e);
    }

    Ok(())
}

pub async fn enforce_daemon_version_sync(_profile: &str) -> Result<()> {
    let app_dir = cowen_common::config::get_app_dir();
    let version_file = app_dir.join("master_daemon.version");
    let pid_file = app_dir.join("master_daemon.pid");

    if !version_file.exists() || !pid_file.exists() {
        return Ok(());
    }

    let running_version = std::fs::read_to_string(&version_file)
        .unwrap_or_default()
        .trim()
        .to_string();
    let current_version = env!("CARGO_PKG_VERSION");

    if !running_version.is_empty() && running_version != current_version {
        println!(
            "⚠️  发现后台进程版本已过时 (运行中: v{}, 当前CLI: v{})",
            running_version, current_version
        );
        println!("🔄 正在自动重启守护进程...");

        let pid_str = std::fs::read_to_string(&pid_file).unwrap_or_default();
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            println!("🛑 Stopping master daemon (PID: {})...", pid);
            let pm = cowen_sys::get_process_manager();
            let _ = pm.kill_process(pid, true).await;
        }

        let _ = std::fs::remove_file(&pid_file);
        let _ = std::fs::remove_file(&version_file);
        let port_file = crate::get_ipc_port_path();
        let _ = std::fs::remove_file(&port_file);

        // It will be auto-started by ensure_daemon_running later
    }

    Ok(())
}
