use anyhow::Result;
use cowen_common::vault::Vault;
use cowen_common::CowenResult;
use cowen_config::ConfigManager;
use serde::Serialize;
use std::sync::Arc;

use cowen_common::status::{StatusCollector, StatusContext, StatusEntry, StatusLevel};

#[derive(Serialize)]
pub struct SystemStatus {
    pub profile: String,
    pub entries: Vec<StatusEntry>,
}

pub async fn status(
    active_profile: &str,
    cfg_mgr: &ConfigManager,
    vault: Arc<dyn Vault>,
    format: &str,
    all: bool,
) -> Result<()> {
    let mut profiles = if all {
        cfg_mgr
            .list_profiles()
            .await
            .map_err(|e| anyhow::anyhow!(e))?
    } else {
        vec![active_profile.to_string()]
    };

    if !all && profiles.len() == 1 {
        let p = &profiles[0];
        if !cfg_mgr.exists(p).await {
            let all_p = cfg_mgr.list_profiles().await.unwrap_or_default();
            if all_p.is_empty() {
                profiles.clear();
            }
        }
    }

    let mut results = Vec::new();
    let mut broken_profiles = Vec::new();

    let mut handles = Vec::new();

    for p in profiles {
        let p_name = p.clone();
        let cfg_mgr_clone = cfg_mgr.clone();
        let vault_clone = vault.clone();
        
        handles.push(tokio::spawn(async move {
            let res = get_system_status(&p_name, &cfg_mgr_clone, vault_clone).await;
            (p_name, res)
        }));
    }

    let results_raw = futures_util::future::join_all(handles).await;

    for handle_res in results_raw {
        match handle_res {
            Ok((_p, Ok(s))) => results.push(s),
            Ok((p, Err(e))) => {
                broken_profiles.push((p.clone(), e.to_string()));
                // Also push a basic status entry so it shows up in JSON/list
                results.push(SystemStatus {
                    profile: p.to_string(),
                    entries: vec![StatusEntry {
                        name: "System".to_string(),
                        icon: "🚨".to_string(),
                        level: StatusLevel::ERROR,
                        message: format!("Profile load failed: {}", p),
                        reason: Some(e.to_string()),
                        details: vec![],
                        children: vec![],
                    }],
                });
            }
            Err(e) => {
                tracing::error!(target: "sys", "Task join error during status collection: {}", e);
            }
        }
    }
    
    // Sort results by profile name to maintain consistent output
    results.sort_by(|a, b| a.profile.cmp(&b.profile));

    if format == "json" || format == "yaml" {
        if !all && results.len() == 1 {
            cowen_common::utils::render(&results[0], format).map_err(|e| anyhow::anyhow!(e))?;
        } else {
            cowen_common::utils::render(&results, format).map_err(|e| anyhow::anyhow!(e))?;
        }
        return Ok(());
    }

    // 1. Print Global Environment Header (Once)
    println!("\n🔍 COWEN System Status Diagnostics");
    println!("----------------------------------");
    println!("Build ID:      {}", cowen_common::BUILD_ID);
    println!("Build Time:    {}", cowen_common::BUILD_TIME);
    
    let store = vault.primary_store();
    let storage_entry = StatusEntry {
        name: "Storage".to_string(),
        icon: "📦".to_string(),
        level: StatusLevel::OK,
        message: format!("Mode: {}", store.name()),
        reason: None,
        details: vec![store.description()],
        children: vec![],
    };
    render_entry(&storage_entry, 0);
    println!();

    if results.is_empty() {
        println!("👤 Profile: Not Initialized");
        println!("----------------------------------");
        println!("⚙️  System is not initialized. Please run `cowen auth login` or `cowen init` to configure a profile.\n");
    } else {
        for s in results {
            println!("👤 Profile: '{}'", s.profile);
            println!("----------------------------------");
            for entry in s.entries {
                render_entry(&entry, 0);
            }
            println!();
        }
    }

    if !broken_profiles.is_empty() {
        println!("\n❌ Profiles with Errors:");
        for (p, e) in broken_profiles {
            println!("  - {}: {}", p, e);
        }
    }

    Ok(())
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

fn render_entry(entry: &StatusEntry, indent: usize) {
    let prefix = "  ".repeat(indent);
    let level_str = match entry.level {
        StatusLevel::OK => "\x1b[32m(OK)\x1b[0m",
        StatusLevel::WARN => "\x1b[33m(WARN)\x1b[0m",
        StatusLevel::ERROR => "\x1b[31m(ERROR)\x1b[0m",
        _ => "(UNKNOWN)",
    };

    println!(
        "{}{} {}: {} {}",
        prefix, entry.icon, entry.name, entry.message, level_str
    );
    if let Some(reason) = &entry.reason {
        println!("{}   \x1b[31m╰─ Reason: {}\x1b[0m", prefix, reason);
    }
    for detail in &entry.details {
        println!("{}   - {}", prefix, detail);
    }
    for child in &entry.children {
        render_entry(child, indent + 1);
    }
}


async fn get_system_status(
    profile: &str,
    cfg_mgr: &ConfigManager,
    vault: Arc<dyn Vault>,
) -> CowenResult<SystemStatus> {
    let cfg = cfg_mgr.load(profile).await?;
    
    let app_cfg = cfg_mgr.load_app_config().await?;

    let ctx = StatusContext {
        profile: profile.to_string(),
        config: &cfg,
        app_config: &app_cfg,
        vault: vault.clone(),
    };

    let mut collectors: Vec<Box<dyn StatusCollector>> = vec![
        Box::new(ConfigCollector),
    ];

    if !cfg.app_key.trim().is_empty() {
        collectors.push(Box::new(ProviderCollector));
    }

    let mut entries = Vec::new();
    for c in collectors {
        if let Ok(e) = c.collect(&ctx).await {
            entries.push(e);
        }
    }

    Ok(SystemStatus {
        profile: profile.to_string(),
        entries,
    })
}

// --- Collectors ---

use cowen_auth::client::Client;

struct ConfigCollector;
#[async_trait::async_trait]
impl StatusCollector for ConfigCollector {
    fn name(&self) -> &str {
        "Configuration"
    }
    async fn collect(&self, ctx: &StatusContext<'_>) -> CowenResult<StatusEntry> {
        use cowen_common::status::CommonTemplate;

        // 0.2.x Style Configuration Entry
        let mode_str = format!("{:?}", ctx.config.app_mode).to_lowercase();
        let mut details = vec![];
        details.push(format!("Build ID:   {}", cowen_common::BUILD_ID));
        details.push(format!("Build Time: {}", cowen_common::BUILD_TIME));
        details.push(format!("OpenAPI:    {}", ctx.app_config.openapi_url));
        details.push(format!("Stream:     {}", ctx.app_config.stream_url));

        let ak_level = if ctx.config.app_key.trim().is_empty() {
            StatusLevel::ERROR
        } else {
            StatusLevel::OK
        };
        let ak_msg = if ak_level == StatusLevel::OK {
            format!("AppKey: {} (Mode: {})", ctx.config.app_key, mode_str)
        } else {
            "AppKey is missing".to_string()
        };

        Ok(StatusEntry::new(
            CommonTemplate::Custom("Configuration".to_string(), "⚙️".to_string()),
            ak_level,
            ak_msg,
        )
        .with_details(details))
    }
}



struct ProviderCollector;
#[async_trait::async_trait]
impl StatusCollector for ProviderCollector {
    fn name(&self) -> &str {
        "Provider"
    }
    async fn collect(&self, ctx: &StatusContext<'_>) -> CowenResult<StatusEntry> {
        use cowen_common::status::CommonTemplate;
        let auth_cli = cowen_auth::create_auth_client_with_vault(ctx.vault.clone());
        let children = auth_cli.get_diagnostics(ctx).await?;

        let mut max_level = StatusLevel::OK;
        for c in &children {
            if c.level as i32 > max_level as i32 {
                max_level = c.level;
            }
        }

        let mode_str = format!("{:?}", ctx.config.app_mode);
        Ok(StatusEntry::new(
            CommonTemplate::ProviderSummary(
                format!("{} Mode Diagnostics", mode_str),
                "💎".to_string(),
            ),
            max_level,
            format!("Collected {} status indicators", children.len()),
        )
        .with_children(children))
    }
}

pub async fn reset(
    profile: &str,
    _vault: Option<&dyn Vault>,
    _cfg_mgr: &ConfigManager,
    event_bus: Option<&cowen_common::events::EventBus>,
    dry_run: bool,
) -> Result<()> {
    use cowen_common::reset::ResetEngine;
    use cowen_config::reset::ConfigResetTask;
    use cowen_monitor::reset::TelemetryResetTask;
    
    let app_dir = cowen_common::config::get_app_dir();
    
    let engine = ResetEngine::new()
        .with(Box::new(ConfigResetTask::new(app_dir.clone())))
        .with(Box::new(TelemetryResetTask::new(app_dir.clone())));

    engine.run(dry_run).await?;

    if !dry_run {
        if let Some(bus) = event_bus {
            bus.publish(cowen_common::events::GlobalEvent::ProfileDeleted {
                name: profile.to_string(),
            });
        }
    }

    Ok(())
}

pub async fn rename_profile(
    old: &str,
    new: &str,
    cfg_mgr: &ConfigManager,
    _vault: Arc<dyn Vault>,
    event_bus: &cowen_common::events::EventBus,
) -> Result<()> {
    if !cfg_mgr.exists(old).await {
        return Err(anyhow::anyhow!("Source profile '{}' does not exist", old));
    }
    if cfg_mgr.exists(new).await {
        return Err(anyhow::anyhow!("Target profile '{}' already exists", new));
    }

    // 1. Rename files and Vault data (cfg_mgr.rename handles both)
    cfg_mgr
        .rename(old, new)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    // 3. Update default if needed
    if cfg_mgr.get_default_profile() == old {
        cfg_mgr
            .set_default_profile(new)
            .map_err(|e| anyhow::anyhow!(e))?;
    }

    // 4. Broadcast event
    event_bus.publish(cowen_common::events::GlobalEvent::ProfileRenamed {
        old: old.to_string(),
        new: new.to_string(),
    });

    println!("✅ Profile '{}' has been renamed to '{}'", old, new);
    Ok(())
}

pub async fn ensure_daemon_running(
    profile: &str,
    config: &cowen_common::Config,
    _cfg_mgr: &ConfigManager,
    vault: Arc<dyn Vault>,
    auth_cli: &dyn cowen_auth::client::Client,
) -> Result<()> {
    // 0. Check intentional stop marker
    let app_dir = cowen_common::config::get_app_dir();
    let stopped_file = app_dir.join("master_daemon.stopped");
    if stopped_file.exists() {
        return Ok(());
    }

    // 1. Check if already running
    if cowen_common::status::get_active_daemon_info(profile).is_some() {
        return Ok(());
    }

    // 🚀 STABILITY FIX: If we just performed a version sync restart, there might be a brief delay 
    // before the new PID file is visible or the process is registered in the OS table.
    // We add a tiny grace period and one re-check to avoid "double-starting" or recovery conflicts.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    if cowen_common::status::get_active_daemon_info(profile).is_some() {
        return Ok(());
    }

    // 2. Check if recovery is recommended by provider
    let provider = auth_cli
        .get_provider(&config.app_mode)
        .ok_or_else(|| anyhow::anyhow!("No provider found for profile '{}'", profile))?;

    // We check pid file existence for extra safety (should_auto_recover might care)
    let pid_file = cowen_common::config::get_app_dir().join(format!("{}_daemon.pid", profile));
    let pid_file_exists = pid_file.exists();

    let app_cfg = _cfg_mgr.load_app_config().await.unwrap_or_default();
    let is_distributed = _cfg_mgr.is_distributed_storage(&app_cfg);

    if provider
        .should_auto_recover(profile, config, false, pid_file_exists, is_distributed)
        .await
    {
        tracing::info!(target: "sys", profile = %profile, "Daemon not running, triggering auto-recovery...");

        #[cfg(unix)]
        let daemon_svc: std::sync::Arc<dyn cowen_common::daemon::DaemonService> = std::sync::Arc::new(cowen_common::ipc::client::IpcDaemonService::new(cowen_common::ipc::get_ipc_port_path()));
        #[cfg(not(unix))]
        let daemon_svc: std::sync::Arc<dyn cowen_common::daemon::DaemonService> = std::sync::Arc::new(cowen_server::ServerDaemonService::new(_cfg_mgr.clone()));
        crate::cmd::daemon::start(profile, config, config.proxy_port, config.proxy_enabled, false, false, _cfg_mgr, vault, None, daemon_svc).await?;
    }

    Ok(())
}

pub async fn enforce_daemon_version_sync(
    _profile: &str,
    cfg_mgr: &ConfigManager,
    vault: Arc<dyn Vault>,
    daemon_svc: Arc<dyn cowen_common::daemon::DaemonService>,
) -> Result<()> {
    let profiles = cfg_mgr.list_profiles().await.unwrap_or_default();
    for p in profiles {
        if let Some(info) = cowen_common::status::get_active_daemon_info(&p) {
            let mut outdated = false;
            let daemon_bid = info.build_id.as_deref().unwrap_or("N/A").trim();
            let daemon_bt = info.build_time.as_deref().unwrap_or("N/A").trim();
            let cli_bid = cowen_common::BUILD_ID.trim();
            let cli_bt = cowen_common::BUILD_TIME.trim();

            // 🚀 ROBUST COMPARISON: Strip common labels if they accidentally leaked in
            let clean_daemon_bid = daemon_bid.strip_prefix("BUILD_ID=").unwrap_or(daemon_bid);
            let clean_daemon_bt = daemon_bt.strip_prefix("BUILD_TIME=").unwrap_or(daemon_bt);
            
            if clean_daemon_bid != cli_bid || clean_daemon_bt != cli_bt {
                outdated = true;
            }

            if outdated {
                tracing::info!(target: "sys", profile = %p, "Daemon version mismatch (CLI: {} / {}, Daemon: {} / {}). Restarting...", cli_bid, cli_bt, clean_daemon_bid, clean_daemon_bt);

                let config = cfg_mgr.load(&p).await.unwrap_or_else(|_| cowen_common::Config::default_with_profile(&p));

                // 🛡️ SECURITY & UX: Only attempt restart and show message if the profile is actually initialized.
                // This prevents "AppKey is empty" warnings for unused default profiles during version sync.
                if !config.app_key.trim().is_empty() {
                    eprintln!("🔄 Profile '{}' 的后台进程版本已过时，正在自动重启以同步最新构建...", p);
                    
                    // Kill the old master daemon process to force a full process restart
                    eprintln!("🛑 Stopping master daemon (PID: {})...", info.pid);
                    #[cfg(unix)]
                    let _ = std::process::Command::new("kill").arg("-15").arg(info.pid.to_string()).status();
                    #[cfg(windows)]
                    let _ = std::process::Command::new("taskkill").args(&["/F", "/PID", &info.pid.to_string()]).status();

                    // Wait for the process to exit
                    for _ in 0..50 {
                        #[cfg(unix)]
                        {
                            let status = std::process::Command::new("kill").arg("-0").arg(info.pid.to_string()).status();
                            if let Ok(st) = status {
                                if !st.success() {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                        #[cfg(windows)]
                        {
                            break;
                        }
                        #[cfg(not(windows))]
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    }

                    // Force kill if still alive
                    #[cfg(unix)]
                    {
                        let status = std::process::Command::new("kill").arg("-0").arg(info.pid.to_string()).status();
                        if let Ok(st) = status {
                            if st.success() {
                                eprintln!("⚠️ Master daemon (PID: {}) is taking too long to shut down. Force killing...", info.pid);
                                let _ = std::process::Command::new("kill").arg("-9").arg(info.pid.to_string()).status();
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            }
                        }
                    }

                    // 🚀 STABILITY: Give the system service manager (e.g. launchd) time to detect the crash 
                    // and restart the daemon BEFORE we try to spawn it manually. 
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

                    crate::cmd::daemon::restart(&p, &config, config.proxy_port, config.proxy_enabled, false, cfg_mgr, vault.clone(), None, daemon_svc.clone()).await?;
                    
                    // 🚀 STABILITY: Brief grace period to allow the new daemon to bind ports and write its PID file
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }        }
    }
    Ok(())
}
