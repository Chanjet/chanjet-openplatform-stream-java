use crate::core::vault::Vault;
use crate::core::config::ConfigManager;
use anyhow::Result;
use serde::Serialize;
use sysinfo::System;
use std::sync::Arc;

use crate::core::status::{StatusEntry, StatusLevel, StatusContext, StatusCollector};
use crate::auth::client::Client;

#[derive(Serialize)]
pub struct SystemStatus {
    pub profile: String,
    pub entries: Vec<StatusEntry>,
}

pub async fn status(
    active_profile: &str,
    cfg_mgr: &crate::core::config::ConfigManager,
    vault: Arc<dyn Vault>,
    format: &str,
    all: bool,
) -> Result<()> {
    let profiles = if all {
        cfg_mgr.list_profiles().await?
    } else {
        vec![active_profile.to_string()]
    };

    // Trigger self-healing BEFORE collection to ensure consistent report
    let active_cfg = cfg_mgr.load(active_profile).await?;
    let auth_cli = crate::auth::create_auth_client_with_vault(vault.clone());
    let _ = ensure_daemon_running(active_profile, &active_cfg, cfg_mgr, vault.clone(), &auth_cli).await;

    let mut statuses = Vec::new();
    let mut errors = Vec::new();
    for profile in &profiles {
        match get_system_status(profile, cfg_mgr, vault.clone()).await {
            Ok(s) => statuses.push(s),
            Err(e) if e.to_string().contains("SKIPPED:") => {
                // Ignore skipped profiles in bulk status
                continue;
            }
            Err(e) => errors.push((profile.clone(), e)),
        }
    }

    if format == "json" || format == "yaml" {
        if let Some(s) = statuses.first() {
            if !all {
                return crate::core::utils::render(s, format);
            }
        }
        return crate::core::utils::render(&statuses, format);
    }

    let bin_name = crate::core::utils::get_bin_name().to_uppercase();
    if !all {
        if let Some(s) = statuses.first() {
            print_single_status(&bin_name, s, false);
        }
    } else {
        println!("🔍 {} System Status Diagnostics (All Profiles)", bin_name);
        println!("==================================================");
        for full_status in statuses {
            print_single_status(&bin_name, &full_status, true);
            println!();
        }
    }

    if all && !errors.is_empty() {
        println!("⚠️  Profiles with Errors:");
        for (profile, err) in errors {
            println!("  - {}: {}", profile, err);
        }
    }
    Ok(())
}

async fn get_system_status(
    profile: &str,
    cfg_mgr: &crate::core::config::ConfigManager,
    vault: Arc<dyn Vault>,
) -> Result<SystemStatus> {
    let cfg = cfg_mgr.load(profile).await?;
    let app_cfg = cfg_mgr.load_app_config().await?;
    let ctx = StatusContext {
        profile: profile.to_string(),
        config: &cfg,
        app_config: &app_cfg,
        vault,
    };

    let collectors: Vec<Box<dyn StatusCollector>> = vec![
        Box::new(ConfigCollector),
        Box::new(ProviderCollector),
    ];

    let mut entries = Vec::new();
    for collector in collectors {
        match collector.collect(&ctx).await {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                entries.push(StatusEntry {
                    name: collector.name().to_string(),
                    icon: "❌".to_string(),
                    level: StatusLevel::ERROR,
                    message: "采集引擎内部故障".to_string(),
                    reason: Some(format!("执行失败: {}", e)),
                    details: vec![],
                    children: vec![],
                });
            }
        }
    }

    Ok(SystemStatus {
        profile: profile.to_string(),
        entries,
    })
}

fn print_single_status(bin_name: &str, full_status: &SystemStatus, all: bool) {
    if !all {
        println!("🔍 {} System Status Diagnostics (Profile: '{}')", bin_name, full_status.profile);
        println!("--------------------------------------------------");
    } else {
        println!("▶ Profile: '{}'", full_status.profile);
    }

    // Render entries, skipping NONE level (hidden panels)
    for entry in &full_status.entries {
        if entry.level == StatusLevel::NONE {
            continue;
        }
        render_entry(entry, 0);
    }
    
    if !all {
        println!("--------------------------------------------------");
    }
}

fn render_entry(entry: &StatusEntry, indent: usize) {
    let padding = "  ".repeat(indent);
    let level_color = match entry.level {
        StatusLevel::OK => "\x1b[32m",     // Green
        StatusLevel::WARN => "\x1b[33m",   // Yellow
        StatusLevel::ERROR => "\x1b[31m",  // Red
        StatusLevel::PENDING => "\x1b[33m",// Yellow
        StatusLevel::NONE => "\x1b[90m",   // Gray
    };
    let reset = "\x1b[0m";

    print!("{}{} {}: {} ({}{}{})", 
        padding, 
        entry.icon, 
        entry.name, 
        entry.message,
        level_color,
        format!("{:?}", entry.level),
        reset
    );

    if (entry.level == StatusLevel::ERROR || entry.level == StatusLevel::WARN) && entry.reason.is_some() {
        print!(" \x1b[90m<- 原因: {}\x1b[0m", entry.reason.as_ref().unwrap());
    }
    println!();

    for detail in &entry.details {
        println!("{}   - {}", padding, detail);
    }

    for child in &entry.children {
        render_entry(child, indent + 1);
    }
}

pub async fn ensure_daemon_running(
    profile: &str, 
    config: &crate::core::config::Config, 
    cfg_mgr: &crate::core::config::ConfigManager, 
    vault: Arc<dyn Vault>,
    auth_cli: &crate::auth::AuthClient,
) -> Result<()> {
    let profiles = cfg_mgr.list_profiles().await.unwrap_or_else(|_| vec![profile.to_string()]);
    let app_dir = crate::core::status::get_app_dir();
    
    for p in profiles {
        let (pid, _build_id) = crate::core::status::get_active_daemon_info(&p).await;
        let mut p_cfg = if p == profile { 
            config.clone() 
        } else { 
            match cfg_mgr.load(&p).await {
                Ok(c) => c,
                Err(e) if e.to_string().contains("SKIPPED:") => continue,
                Err(_) => crate::core::config::Config::default_with_profile(&p),
            }
        };
        
        if p != profile {
            if let Ok(as_val) = vault.get_secret(&p, "app_secret").await { p_cfg.app_secret = as_val; }
            if let Ok(cert) = vault.get_secret(&p, "certificate").await { p_cfg.certificate = cert; }
            if let Ok(ek) = vault.get_secret(&p, "encrypt_key").await { p_cfg.encrypt_key = ek; }
        }
        
        let pid_file = app_dir.join(format!("{}_daemon.pid", p));
        
        // 1. Hanging detection (Process exists but port unresponsive)
        if let Some(pid_val) = pid {
            if !crate::core::status::is_port_responsive(p_cfg.proxy_port).await {
                tracing::warn!(target: "sys", profile = %p, pid = %pid_val, port = %p_cfg.proxy_port, "Daemon process found but port is not responsive (Hanging). Killing and restarting...");
                
                let mut s = System::new_all();
                s.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                if let Some(process) = s.process(sysinfo::Pid::from_u32(pid_val)) {
                    process.kill_with(sysinfo::Signal::Kill);
                }
                let _ = std::fs::remove_file(&pid_file);
                
                // Trigger restart
                let _ = crate::cmd::daemon::start(&p, &p_cfg, p_cfg.proxy_port, p_cfg.proxy_enabled, false, false, cfg_mgr, vault.clone()).await;
                continue;
            }
        }
        
        // 2. Normal missing process detection
        if pid.is_none() {
            let provider = auth_cli.provider(&p_cfg.app_mode);
            if provider.should_auto_recover(&p_cfg, pid.is_some(), pid_file.exists()) {
                tracing::info!(target: "sys", profile = %p, mode = ?p_cfg.app_mode, "Daemon recovery triggered. Launching background worker...");
                let _ = crate::cmd::daemon::start(&p, &p_cfg, p_cfg.proxy_port, p_cfg.proxy_enabled, false, false, cfg_mgr, vault.clone()).await;
            }
        }
    }
    Ok(())
}

pub async fn config(_profile: &str, cfg_mgr: &ConfigManager, format: &str) -> Result<()> {
    let cfg = cfg_mgr.load(_profile).await?;
    let app_cfg = cfg_mgr.load_app_config().await?;
    
    #[derive(Serialize)]
    struct CombinedConfig {
        global: crate::core::config::AppConfig,
        profile: crate::core::config::Config,
    }
    
    let combined = CombinedConfig {
        global: app_cfg,
        profile: cfg,
    };

    if format == "json" || format == "yaml" {
        crate::core::utils::render(&combined, format)?;
    } else {
        println!("\n🌐 Global Configuration (app.yaml)");
        println!("----------------------------------");
        println!("Storage Type:  {}", combined.global.storage.store);
        if let Some(url) = &combined.global.storage.db_url {
            println!("Database URL:  {}", url);
        }
        println!("Cache Type:    {}", combined.global.storage.cache);
        if let Some(url) = &combined.global.storage.cache_url {
            println!("Cache URL:     {}", url);
        }

        println!("\n📂 Profile Configuration ({}.yaml)", _profile);
        println!("----------------------------------");
        println!("{:#?}", combined.profile);
    }
    Ok(())
}

pub async fn reset(_profile: &str, vault: Option<&dyn Vault>, cfg_mgr: &ConfigManager) -> Result<()> {
    eprintln!("Resetting profile '{}'...", _profile);
    if let Err(e) = crate::cmd::daemon::stop(_profile, false, cfg_mgr).await {
        tracing::warn!(target: "sys", profile = %_profile, error = %e, "Failed to stop daemon during reset");
    }
    if let Some(v) = vault {
        let _ = v.clear_profile(_profile).await;
    }
    let base_dir = crate::core::status::get_app_dir();
    let targets = vec![
        base_dir.join(format!("{}.yaml", _profile)),
        base_dir.join(format!("{}_openapi.json", _profile)),
        base_dir.join(format!("{}_openapi.yaml", _profile)),
        base_dir.join(format!("{}_daemon.pid", _profile)),
        base_dir.join("dlq").join(_profile),
        base_dir.join("logs").join(_profile),
    ];
    for path in targets {
        if path.exists() {
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    // Also clean up log files with the profile prefix in the logs directory
    let log_dir = base_dir.join("logs");
    if log_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&log_dir) {
            for entry in entries.flatten() {
                if let Some(filename) = entry.file_name().to_str() {
                    if filename.starts_with(&format!("{}_", _profile)) {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
        }
    }

    // Smart reset: switch to another available profile if the active one was deleted
    if cfg_mgr.get_default_profile() == _profile {
        let available_profiles = cfg_mgr.list_profiles().await.unwrap_or_default();
        let next_profile = available_profiles.iter()
            .find(|&p| p != _profile)
            .cloned()
            .unwrap_or_else(|| "default".to_string());
        
        let _ = cfg_mgr.set_default_profile(&next_profile);
        eprintln!("🔄 Active profile deleted. Switched to '{}'.", next_profile);
    }

    eprintln!("✨ Profile '{}' reset complete.", _profile);
    Ok(())
}

pub async fn rename_profile(
    old_name: &str,
    new_name: &str,
    cfg_mgr: &ConfigManager,
    vault: Arc<dyn Vault>,
) -> Result<()> {
    if old_name == new_name {
        return Err(anyhow::anyhow!("Old and new profile names are the same."));
    }
    if !cfg_mgr.exists(old_name).await {
        return Err(anyhow::anyhow!("Profile '{}' does not exist.", old_name));
    }
    if cfg_mgr.exists(new_name).await {
        return Err(anyhow::anyhow!("Profile '{}' already exists. Choose a different name.", new_name));
    }

    // 1. Stop Daemon if running
    let (pid, _) = crate::core::status::get_active_daemon_info(old_name).await;
    let was_running = pid.is_some();
    if was_running {
        eprintln!("🛑 Stopping daemon for '{}' before rename...", old_name);
        let _ = crate::cmd::daemon::stop(old_name, false, cfg_mgr).await;
    }

    // 2. Rename config and cache files
    let base_dir = crate::core::status::get_app_dir();
    let file_map = vec![
        ("", ".yaml"),
        ("_openapi", ".json"),
        ("_openapi", ".yaml"),
        ("_daemon", ".pid"),
    ];

    for (suffix, ext) in file_map {
        let old_path = base_dir.join(format!("{}{}{}", old_name, suffix, ext));
        let new_path = base_dir.join(format!("{}{}{}", new_name, suffix, ext));
        if old_path.exists() {
            std::fs::rename(old_path, new_path)?;
        }
    }


    // 4. Rename log files
    let log_dir = base_dir.join("logs");
    if log_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&log_dir) {
            for entry in entries.flatten() {
                if let Some(filename) = entry.file_name().to_str() {
                    let old_prefix = format!("{}_", old_name);
                    if filename.starts_with(&old_prefix) {
                        let new_filename = filename.replacen(&old_prefix, &format!("{}_", new_name), 1);
                        let _ = std::fs::rename(entry.path(), log_dir.join(new_filename));
                    }
                }
            }
        }
    }

    // 5. Update Vault secrets
    vault.rename_profile(old_name, new_name).await?;

    // 6. Update default profile pointer if it matched
    if cfg_mgr.get_default_profile() == old_name {
        cfg_mgr.set_default_profile(new_name)?;
        eprintln!("✅ Global default profile pointer updated to '{}'.", new_name);
    }

    // 7. Restart daemon if it was running
    if was_running {
        eprintln!("🚀 Restarting daemon under new name '{}'...", new_name);
        let config = cfg_mgr.load(new_name).await?;
        let _ = crate::cmd::daemon::start(new_name, &config, config.proxy_port, config.proxy_enabled, false, false, cfg_mgr, vault).await;
    }

    eprintln!("✨ Profile '{}' successfully renamed to '{}'.", old_name, new_name);
    Ok(())
}

// --- Status Collectors ---

struct ConfigCollector;
#[async_trait::async_trait]
impl StatusCollector for ConfigCollector {
    fn name(&self) -> &str { "Config" }
    async fn collect(&self, ctx: &StatusContext<'_>) -> Result<StatusEntry> {
        use crate::core::status::CommonTemplate;
        let mode_str = serde_json::to_string(&ctx.config.app_mode).unwrap_or_default().trim_matches('"').to_string();
        let details = vec![
            format!("OpenAPI: {}", ctx.config.openapi_url),
            format!("Stream:  {}", ctx.config.stream_url),
        ];
        
        let mut children = vec![];
        
        // COMPATIBILITY: Show storage/cache details as standardized child entries if not default
        if ctx.app_config.storage.store != "local" {
            children.push(StatusEntry::new(CommonTemplate::Storage, StatusLevel::OK, 
                format!("{} ({})", ctx.app_config.storage.store, ctx.app_config.storage.db_url.as_deref().unwrap_or("none"))));
        }
        if ctx.app_config.storage.cache != "none" {
            children.push(StatusEntry::new(CommonTemplate::Cache, StatusLevel::OK, 
                format!("{} ({})", ctx.app_config.storage.cache, ctx.app_config.storage.cache_url.as_deref().unwrap_or("none"))));
        }
        
        let (level, msg) = if !ctx.config.app_key.is_empty() {
            (StatusLevel::OK, format!("AppKey: {} (Mode: {})", ctx.config.app_key, mode_str))
        } else {
            (StatusLevel::ERROR, "Profile not initialized or AppKey empty.".to_string())
        };

        Ok(StatusEntry::new(CommonTemplate::Configuration, level, msg)
            .with_reason(if level == StatusLevel::ERROR { Some("请运行 'cowen init' 进行初始化".to_string()) } else { None })
            .with_details(details)
            .with_children(children))
    }
}

struct ProviderCollector;
#[async_trait::async_trait]
impl StatusCollector for ProviderCollector {
    fn name(&self) -> &str { "Provider" }
    async fn collect(&self, ctx: &StatusContext<'_>) -> Result<StatusEntry> {
        use crate::core::status::CommonTemplate;
        let auth = crate::auth::create_auth_client_with_vault(ctx.vault.clone());
        let mode_str = serde_json::to_string(&ctx.config.app_mode).unwrap_or_default().trim_matches('"').to_uppercase();
        
        let children: Vec<StatusEntry> = auth.get_diagnostics(ctx).await?;
        
        let max_level = children.iter().map(|e| e.level).max_by_key(|l| match l {
            StatusLevel::ERROR => 3,
            StatusLevel::WARN => 2,
            StatusLevel::OK => 1,
            _ => 0,
        }).unwrap_or(StatusLevel::OK);

        Ok(StatusEntry::new(CommonTemplate::
ProviderSummary(format!("{} Mode Diagnostics", mode_str), "💎".to_string()), max_level, format!("Collected {} status indicators", children.len()))
            .with_children(children))
    }
}
