use anyhow::Result;
use cowen_common::ipc::client::IpcDaemonService;
use cowen_common::ipc::DaemonResponse;
use cowen_config::ConfigManager;
use serde_json::Value;

pub async fn status(
    profile: &str,
    cfg_mgr: &ConfigManager,
    format: &str,
    all: bool,
) -> Result<()> {
    let port_path = cowen_common::ipc::get_ipc_port_path();
    let ipc = IpcDaemonService::new(port_path);

    match ipc.system_status(profile, all).await {
        Ok(DaemonResponse::SystemStatusData { json }) => {
            if format == "json" || format == "yaml" {
                let val: Value = serde_json::from_str(&json)?;
                if !all {
                    if let Some(arr) = val.as_array() {
                        if arr.len() == 1 {
                            cowen_common::utils::render(&arr[0], format).map_err(|e| anyhow::anyhow!(e))?;
                            return Ok(());
                        }
                    }
                }
                cowen_common::utils::render(&val, format).map_err(|e| anyhow::anyhow!(e))?;
                return Ok(());
            }

            let results: Vec<Value> = serde_json::from_str(&json)?;
            
            println!("\n🔍 COWEN System Status Diagnostics");
            println!("----------------------------------");
            println!("Build ID:      {}", cowen_common::BUILD_ID);
            println!("Build Time:    {}", cowen_common::BUILD_TIME);
            println!();

            // Print global storage configuration status
            if let Ok(app_cfg) = cfg_mgr.load_app_config().await {
                let mode = &app_cfg.storage.store;
                println!("📦 Storage: Mode: {} \x1b[32m(OK)\x1b[0m", mode);
            } else {
                println!("📦 Storage: Mode: local \x1b[32m(OK)\x1b[0m");
            }
            println!();

            if results.is_empty() {
                println!("👤 Profile: Not Initialized");
                println!("----------------------------------");
                println!("⚙️  System is not initialized. Please run `cowen auth login` or `cowen init` to configure a profile.\n");
            } else {
                for s in results {
                    let prof = s["profile"].as_str().unwrap_or("Unknown");
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
        }
        Ok(DaemonResponse::Error { message, .. }) => eprintln!("❌ Status fetch failed: {}", message),
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
    
    let level_str = match entry["level"].as_i64().unwrap_or(0) {
        0 => "\x1b[32m(OK)\x1b[0m",
        1 => "\x1b[33m(WARN)\x1b[0m",
        2 => "\x1b[31m(ERROR)\x1b[0m",
        _ => "(UNKNOWN)",
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

pub async fn config(profile: &str, cfg_mgr: &ConfigManager, format: &str, all: bool) -> Result<()> {
    if format == "json" || format == "yaml" {
        let val = if all {
            cfg_mgr.list_all_values().await.map_err(|e| anyhow::anyhow!(e))?
        } else {
            cfg_mgr.list_values(profile).await.map_err(|e| anyhow::anyhow!(e))?
        };

        if format == "json" {
            println!("{}", serde_json::to_string_pretty(&val).unwrap());
        } else {
            println!("{}", serde_yaml::to_string(&val).unwrap());
        }
        return Ok(());
    }

    let profiles_to_show = if all {
        let mut list = cfg_mgr.list_local_profiles().map_err(|e| anyhow::anyhow!(e))?;
        list.sort();
        list
    } else {
        vec![profile.to_string()]
    };

    let print_block = |title: &str, fields: Vec<cowen_config::config_manager::ConfigFieldDisplay>| {
        println!("\n{}", title);
        println!("-------------------------------------------------------------------------");
        for field in fields {
            let key = if field.readonly { format!("{} 🔒", field.key) } else { field.key };
            println!("{:<20} : {}", key, field.value);
        }
    };

    let global_fields = cfg_mgr.get_global_display().await.map_err(|e| anyhow::anyhow!(e))?;
    print_block("🌐 Global Configuration (app.yaml) - `cowen config set <key> <value> --global`", global_fields);

    for p in profiles_to_show {
        let profile_fields = cfg_mgr.get_profile_display(&p).await.map_err(|e| anyhow::anyhow!(e))?;
        print_block(&format!("👤 Profile Configuration ({}.yaml) - `cowen config set <key> <value>`", p), profile_fields);
    }
    
    println!("\n  (🔒 Indicates fields that are read-only or managed via other commands)\n");

    Ok(())
}

pub async fn reset(
    target_profile: Option<&str>,
    _cfg_mgr: &ConfigManager,
    dry_run: bool,
) -> Result<()> {
    let port_path = cowen_common::ipc::get_ipc_port_path();
    let ipc = IpcDaemonService::new(port_path);

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
        Err(_) => {
            true
        }
        _ => {
            eprintln!("❌ Unexpected response");
            true
        }
    };

    if is_ipc_error {
        if !dry_run {
            println!("⚠️  Daemon not reachable. Performing local reset...");
        }
        let app_dir = cowen_common::config::get_app_dir();
        
        let config_task = cowen_config::reset::ConfigResetTask::new(app_dir.clone(), target_profile.map(|s| s.to_string()));
        let telemetry_task = cowen_monitor::reset::TelemetryResetTask::new(app_dir.clone(), target_profile.map(|s| s.to_string()));
        
        use cowen_common::reset::ResetEngine;
        let engine = ResetEngine::new()
            .with(Box::new(config_task))
            .with(Box::new(telemetry_task));
        
        engine.run(dry_run).await?;
        
        if !dry_run {
            if target_profile.is_none() {
                // If reset all, remove the cowen.db file as well
                let db_file = app_dir.join("cowen.db");
                let _ = std::fs::remove_file(&db_file);
                let _ = std::fs::remove_file(app_dir.join("cowen.db-wal"));
                let _ = std::fs::remove_file(app_dir.join("cowen.db-shm"));
            } else if let Some(p) = target_profile {
                // Also manually remove the profile config in case ConfigResetTask missed it
                let config_file = app_dir.join("profiles").join(format!("{}.yaml", p));
                if config_file.exists() {
                    let _ = std::fs::remove_file(config_file);
                }
            }
            println!("✅ Local reset complete.");
        }
    }
    
    Ok(())
}

pub async fn rename_profile(
    old: &str,
    new: &str,
    _cfg_mgr: &ConfigManager,
) -> Result<()> {
    let port_path = cowen_common::ipc::get_ipc_port_path();
    let ipc = IpcDaemonService::new(port_path);

    match ipc.rename_profile(old, new).await {
        Ok(DaemonResponse::Success { message }) => println!("✅ {}", message),
        Ok(DaemonResponse::Error { message, .. }) => eprintln!("❌ Rename failed: {}", message),
        Err(e) => eprintln!("❌ IPC Error: {}", e),
        _ => eprintln!("❌ Unexpected response"),
    }
    Ok(())
}

pub async fn ensure_daemon_running(
    _profile: &str,
    _config: &cowen_common::Config,
    _cfg_mgr: &ConfigManager,
) -> Result<()> {
    let app_dir = cowen_common::config::get_app_dir();
    let stopped_file = app_dir.join("master_daemon.stopped");
    if stopped_file.exists() {
        return Ok(());
    }

    let port_path = cowen_common::ipc::get_ipc_port_path();
    if !port_path.exists() {
        println!("⚠️  Daemon not running, triggering auto-recovery...");
    }
    if let Err(e) = cowen_common::ipc::client::ensure_daemon(&port_path).await {
        eprintln!("❌ ensure_daemon failed: {}", e);
    }
    
    Ok(())
}

pub async fn enforce_daemon_version_sync(
    _profile: &str,
    _cfg_mgr: &ConfigManager,
) -> Result<()> {
    let app_dir = cowen_common::config::get_app_dir();
    let version_file = app_dir.join("master_daemon.version");
    let pid_file = app_dir.join("master_daemon.pid");

    if !version_file.exists() || !pid_file.exists() {
        return Ok(());
    }

    let running_version = std::fs::read_to_string(&version_file).unwrap_or_default().trim().to_string();
    let current_version = env!("CARGO_PKG_VERSION");

    if !running_version.is_empty() && running_version != current_version {
        println!("⚠️  发现后台进程版本已过时 (运行中: v{}, 当前CLI: v{})", running_version, current_version);
        println!("🔄 正在自动重启守护进程...");
        
        let pid_str = std::fs::read_to_string(&pid_file).unwrap_or_default();
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            println!("🛑 Stopping master daemon (PID: {})...", pid);
            #[cfg(windows)]
            let _ = std::process::Command::new("taskkill").arg("/F").arg("/PID").arg(pid.to_string()).status();
            #[cfg(unix)]
            let _ = std::process::Command::new("kill").arg("-9").arg(pid.to_string()).status();
        }

        let _ = std::fs::remove_file(&pid_file);
        let _ = std::fs::remove_file(&version_file);
        let port_file = cowen_common::ipc::get_ipc_port_path();
        let _ = std::fs::remove_file(&port_file);

        // It will be auto-started by ensure_daemon_running later
    }
    
    Ok(())
}
