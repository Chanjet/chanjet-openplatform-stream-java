use crate::core::vault::Vault;
use crate::core::config::ConfigManager;
use anyhow::Result;
use serde::Serialize;
use chrono::{Local, Utc};
use sysinfo::System;
use std::sync::Arc;

use crate::cmd::status_models::{StatusEntry, StatusLevel, StatusContext, StatusCollector};

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
        cfg_mgr.list_profiles()?
    } else {
        vec![active_profile.to_string()]
    };

    // Trigger self-healing BEFORE collection to ensure consistent report
    let active_cfg = cfg_mgr.load(active_profile)?;
    let _ = ensure_daemon_running(active_profile, &active_cfg, cfg_mgr, vault.clone()).await;

    let mut statuses = Vec::new();
    let mut errors = Vec::new();
    for profile in &profiles {
        match get_system_status(profile, cfg_mgr, vault.clone()).await {
            Ok(s) => statuses.push(s),
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
    let cfg = cfg_mgr.load(profile)?;
    let ctx = StatusContext {
        profile: profile.to_string(),
        config: &cfg,
        vault,
    };

    let collectors: Vec<Box<dyn StatusCollector>> = vec![
        Box::new(ConfigCollector),
        Box::new(SecurityCollector),
        Box::new(TokenCollector),
        Box::new(TicketCollector),
        Box::new(DaemonCollector),
    ];

    let mut entries = Vec::new();
    for collector in collectors {
        if let Ok(entry) = collector.collect(&ctx).await {
            entries.push(entry);
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

    println!("{}{} {}: {} ({}{}{})", 
        padding, 
        entry.icon, 
        entry.name, 
        entry.message,
        level_color,
        format!("{:?}", entry.level),
        reset
    );

    for detail in &entry.details {
        println!("{}   - {}", padding, detail);
    }

    for child in &entry.children {
        render_entry(child, indent + 1);
    }
}

fn app_dir() -> std::path::PathBuf {
    crate::core::config::get_app_dir()
}

async fn get_active_daemon_info(profile: &str) -> (Option<u32>, Option<String>) {
    let pid_file = app_dir().join(format!("{}_daemon.pid", profile));
    if !pid_file.exists() {
        return (None, None);
    }

    if let Ok(pid_content) = std::fs::read_to_string(&pid_file) {
        let mut lines = pid_content.lines();
        if let Some(pid_str) = lines.next() {
            if let Ok(pid_val) = pid_str.trim().parse::<u32>() {
                let mut s = System::new_all();
                s.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                if let Some(process) = s.process(sysinfo::Pid::from_u32(pid_val)) {
                    let cmdline = process.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" ");
                    let name = process.name().to_string_lossy().to_lowercase();
                    if name.contains(env!("CARGO_PKG_NAME")) || cmdline.contains("daemon") {
                        let build_id = lines.next().map(|s| s.trim().to_string());
                        return (Some(pid_val), build_id);
                    }
                }
            }
        }
    }
    (None, None)
}

pub async fn ensure_daemon_running(profile: &str, config: &crate::core::config::Config, cfg_mgr: &crate::core::config::ConfigManager, vault: Arc<dyn Vault>) -> Result<()> {
    let profiles = cfg_mgr.list_profiles().unwrap_or_else(|_| vec![profile.to_string()]);
    
    for p in profiles {
        let (pid, _build_id) = get_active_daemon_info(&p).await;
        let mut p_cfg = if p == profile { config.clone() } else { cfg_mgr.load(&p).unwrap_or_else(|_| crate::core::config::Config::default_with_profile(&p)) };
        
        if p != profile {
            if let Ok(as_val) = vault.get(&p, "app_secret") { p_cfg.app_secret = as_val; }
            if let Ok(cert) = vault.get(&p, "certificate") { p_cfg.certificate = cert; }
            if let Ok(ek) = vault.get(&p, "encrypt_key") { p_cfg.encrypt_key = ek; }
        }
        
        if pid.is_none() {
            let p_cfg_inner = if p == profile { config.clone() } else { cfg_mgr.load(&p).unwrap_or_else(|_| crate::core::config::Config::default_with_profile(&p)) };
            let pid_file = app_dir().join(format!("{}_daemon.pid", p));
            
            if should_recover_daemon(p_cfg_inner.app_mode, pid.is_some(), pid_file.exists()) {
                tracing::info!(target: "sys", profile = %p, mode = ?p_cfg_inner.app_mode, "Daemon recovery triggered. Launching background worker...");
                let _ = crate::cmd::daemon::start(&p, &p_cfg_inner, p_cfg_inner.proxy_port, p_cfg_inner.proxy_enabled, false, false, cfg_mgr, vault.clone()).await;
            }
        }
    }
    Ok(())
}

fn should_recover_daemon(_mode: crate::auth::models::AuthMode, has_pid: bool, pid_file_exists: bool) -> bool {
    if has_pid {
        return false;
    }
    
    // Recovery Policy:
    // 1. If it crashed (PID file exists but process missing)
    // 2. OR IF offline (Always-online policy for ALL modes to ensure "秒级 API 响应")
    // Note: p_cfg_inner.app_key check in calling function or daemon::start 
    // ensures we don't start for uninitialized profiles.
    true
}

pub async fn config(_profile: &str, cfg_mgr: &ConfigManager, format: &str) -> Result<()> {
    let cfg = cfg_mgr.load(_profile)?;
    if format == "json" || format == "yaml" {
        crate::core::utils::render(&cfg, format)?;
    } else {
        println!("{:#?}", cfg);
    }
    Ok(())
}

pub async fn reset(_profile: &str, vault: Option<&dyn Vault>, cfg_mgr: &ConfigManager) -> Result<()> {
    eprintln!("Resetting profile '{}'...", _profile);
    if let Err(e) = crate::cmd::daemon::stop(_profile, false, cfg_mgr).await {
        tracing::warn!(target: "sys", profile = %_profile, error = %e, "Failed to stop daemon during reset");
    }
    if let Some(v) = vault {
        let _ = v.clear_profile(_profile);
    }
    let app_dir = app_dir();
    
    let targets = vec![
        app_dir.join(format!("{}.yaml", _profile)),
        app_dir.join(format!("{}_openapi.json", _profile)),
        app_dir.join(format!("{}_openapi.yaml", _profile)),
        app_dir.join(format!("{}_openapi.idx", _profile)),
        app_dir.join(format!("{}_daemon.pid", _profile)),
    ];
    for path in targets {
        if path.exists() { let _ = std::fs::remove_file(&path); }
    }

    let dlq_dir = app_dir.join("dlq").join(_profile);
    if dlq_dir.exists() { let _ = std::fs::remove_dir_all(&dlq_dir); }

    let log_dir = app_dir.join("logs");
    if log_dir.exists() {
        let prefix = format!("{}_", _profile);
        if let Ok(entries) = std::fs::read_dir(log_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(&prefix) { let _ = std::fs::remove_file(entry.path()); }
                }
            }
        }
    }
    
    eprintln!("✨ Profile '{}' reset complete.", _profile);
    Ok(())
}

// --- Status Collectors ---

struct ConfigCollector;
#[async_trait::async_trait]
impl StatusCollector for ConfigCollector {
    fn name(&self) -> &str { "Config" }
    async fn collect(&self, ctx: &StatusContext<'_>) -> Result<StatusEntry> {
        let mode_str = serde_json::to_string(&ctx.config.app_mode).unwrap_or_default().trim_matches('"').to_string();
        let details = vec![
            format!("OpenAPI: {}", ctx.config.openapi_url),
            format!("Stream:  {}", ctx.config.stream_url),
        ];
        
        let (level, msg) = if !ctx.config.app_key.is_empty() {
            (StatusLevel::OK, format!("AppKey: {} (Mode: {})", ctx.config.app_key, mode_str))
        } else {
            (StatusLevel::ERROR, "Profile not initialized or AppKey empty.".to_string())
        };

        Ok(StatusEntry {
            name: "Configuration".to_string(),
            icon: "⚙️".to_string(),
            level,
            message: msg,
            details,
            children: vec![],
        })
    }
}

struct SecurityCollector;
#[async_trait::async_trait]
impl StatusCollector for SecurityCollector {
    fn name(&self) -> &str { "Security" }
    async fn collect(&self, ctx: &StatusContext<'_>) -> Result<StatusEntry> {
        let mut missing = Vec::new();
        if ctx.config.app_mode == crate::auth::models::AuthMode::SelfBuilt {
            if ctx.vault.get(&ctx.profile, "app_secret").is_err() { missing.push("app_secret".to_string()); }
            if ctx.vault.get(&ctx.profile, "certificate").is_err() { missing.push("certificate".to_string()); }
            if ctx.vault.get(&ctx.profile, "encrypt_key").is_err() { missing.push("encrypt_key".to_string()); }
        }

        let (level, msg) = if missing.is_empty() {
            (StatusLevel::OK, "All core secrets are securely stored.".to_string())
        } else {
            (StatusLevel::WARN, format!("Missing: {}", missing.join(", ")))
        };

        Ok(StatusEntry {
            name: "Security (Vault)".to_string(),
            icon: "🛡️".to_string(),
            level,
            message: msg,
            details: vec![],
            children: vec![],
        })
    }
}

struct TokenCollector;
#[async_trait::async_trait]
impl StatusCollector for TokenCollector {
    fn name(&self) -> &str { "Token" }
    async fn collect(&self, ctx: &StatusContext<'_>) -> Result<StatusEntry> {
        let vault = &ctx.vault;
        let profile = &ctx.profile;

        if let Ok(pair_raw) = vault.get(profile, "oauth2_token_pair") {
            let pair: crate::auth::models::OAuth2TokenPair = serde_json::from_str(&pair_raw)?;
            let is_expired = Utc::now() > pair.expires_at;
            let ref_expired = Utc::now() > pair.refresh_expires_at;

            let children = vec![
                StatusEntry {
                    name: "AccessToken".to_string(),
                    icon: "🔑".to_string(),
                    level: if is_expired { StatusLevel::ERROR } else { StatusLevel::OK },
                    message: format!("[{}] (Expires: {})", 
                        if is_expired { "EXPIRED" } else { "VALID" },
                        pair.expires_at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S")),
                    details: vec![],
                    children: vec![],
                },
                StatusEntry {
                    name: "RefreshToken".to_string(),
                    icon: "🔄".to_string(),
                    level: if ref_expired { StatusLevel::ERROR } else { StatusLevel::OK },
                    message: format!("[{}] (Expires: {})", 
                        if ref_expired { "EXPIRED" } else { "VALID" },
                        pair.refresh_expires_at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S")),
                    details: vec![],
                    children: vec![],
                }
            ];

            return Ok(StatusEntry {
                name: "Authentication".to_string(),
                icon: "🔐".to_string(),
                level: if is_expired { StatusLevel::WARN } else { StatusLevel::OK },
                message: "OAuth2 tokens are locally managed.".to_string(),
                details: vec![],
                children,
            });
        }

        // Fallback: Check for generic access_token (e.g. Self-Built mode)
        if let Ok(access_token) = vault.get(profile, "access_token") {
            if !access_token.trim().is_empty() {
                let expires_at_str = vault.get(profile, "access_token_expires").unwrap_or_default();
                let expires_at = chrono::DateTime::parse_from_rfc3339(&expires_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok();
                
                let is_expired = expires_at.map(|exp| Utc::now() > exp).unwrap_or(false);
                let exp_msg = expires_at
                    .map(|exp| exp.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                return Ok(StatusEntry {
                    name: "AccessToken".to_string(),
                    icon: "🔑".to_string(),
                    level: if is_expired { StatusLevel::ERROR } else { StatusLevel::OK },
                    message: format!("[{}] (Expires: {})", 
                        if is_expired { "EXPIRED" } else { "VALID" },
                        exp_msg),
                    details: vec![],
                    children: vec![],
                });
            }
        }

        Ok(StatusEntry {
            name: "AccessToken".to_string(),
            icon: "🔑".to_string(),
            level: StatusLevel::NONE,
            message: "[NONE] (未获取到有效令牌)".to_string(),
            details: vec![],
            children: vec![],
        })
    }
}

struct TicketCollector;
#[async_trait::async_trait]
impl StatusCollector for TicketCollector {
    fn name(&self) -> &str { "Ticket" }
    async fn collect(&self, ctx: &StatusContext<'_>) -> Result<StatusEntry> {
        if ctx.config.app_mode == crate::auth::models::AuthMode::Oauth2 {
            return Ok(StatusEntry {
                name: "AppTicket".to_string(),
                icon: "🎫".to_string(),
                level: StatusLevel::NONE,
                message: "OAuth2 模式无需 AppTicket".to_string(),
                details: vec![],
                children: vec![],
            });
        }

        if let Ok(ts_str) = ctx.vault.get(&ctx.profile, "app_ticket_created") {
            let created_at = chrono::DateTime::parse_from_rfc3339(&ts_str).map(|dt| dt.with_timezone(&Utc)).unwrap_or(Utc::now());
            Ok(StatusEntry {
                name: "AppTicket".to_string(),
                icon: "🎫".to_string(),
                level: StatusLevel::OK,
                message: format!("[CACHED] (Received: {})", created_at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S")),
                details: vec![],
                children: vec![],
            })
        } else {
            Ok(StatusEntry {
                name: "AppTicket".to_string(),
                icon: "🎫".to_string(),
                level: StatusLevel::NONE,
                message: "[NONE] (等待 Daemon 接收推送)".to_string(),
                details: vec![],
                children: vec![],
            })
        }
    }
}

struct DaemonCollector;
#[async_trait::async_trait]
impl StatusCollector for DaemonCollector {
    fn name(&self) -> &str { "Daemon" }
    async fn collect(&self, ctx: &StatusContext<'_>) -> Result<StatusEntry> {
        let (found_daemon_pid, found_build_id) = get_active_daemon_info(&ctx.profile).await;
        
        let is_oauth2 = ctx.config.app_mode == crate::auth::models::AuthMode::Oauth2;
        let (level, msg, children) = if let Some(pid) = found_daemon_pid {
            (
                StatusLevel::OK, 
                format!("[RUNNING] (PID: {})", pid),
                vec![
                    StatusEntry {
                        name: "Proactive Refresh".to_string(),
                        icon: "🔄".to_string(),
                        level: StatusLevel::OK,
                        message: "主动续约: [ACTIVE] 令牌环境将保持热启动状态".to_string(),
                        details: vec![],
                        children: vec![],
                    }
                ]
            )
        } else {
            (
                StatusLevel::WARN, 
                "[OFFLINE] (未检测到活跃后台进程)".to_string(),
                vec![
                    StatusEntry {
                        name: "Efficiency Tip".to_string(),
                        icon: "💡".to_string(),
                        level: StatusLevel::WARN,
                        message: if is_oauth2 {
                            "若需实现令牌主动续约与秒级 API 响应，请运行 'cowen daemon start'".to_string()
                        } else {
                            "若需实现实时消息同步与秒级 API 响应，请运行 'cowen daemon start'".to_string()
                        },
                        details: vec![],
                        children: vec![],
                    }
                ]
            )
        };

        let mut details = vec![];
        if let Some(bid) = found_build_id {
            details.push(format!("Build ID: {}", bid));
        }

        let display_name = if is_oauth2 { "Token Renewer (Daemon)" } else { "Stream Bridge (Daemon)" };

        Ok(StatusEntry {
            name: display_name.to_string(),
            icon: "📟".to_string(),
            level,
            message: msg,
            details,
            children,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::models::AuthMode;

    #[test]
    fn test_should_recover_daemon_policy() {
        // Case 1: Already running - should NOT recover
        assert!(!should_recover_daemon(AuthMode::Oauth2, true, true));
        assert!(!should_recover_daemon(AuthMode::SelfBuilt, true, true));

        // Case 2: Crashed (PID file exists but no process) - should ALWAYS recover
        assert!(should_recover_daemon(AuthMode::Oauth2, false, true));
        assert!(should_recover_daemon(AuthMode::SelfBuilt, false, true));

        // Case 3: Offline (No PID, No PID file) - the core issue
        // OAuth2: Always online policy -> should recover
        assert!(should_recover_daemon(AuthMode::Oauth2, false, false));
        
        // SelfBuilt: Should also have always online policy (Fix for user reported issue)
        assert!(should_recover_daemon(AuthMode::SelfBuilt, false, false), "Self-built mode SHOULD automatically start if offline");
    }
    
    #[test]
    fn test_should_recover_daemon_policy_future() {
        // This is the target state for SelfBuilt offline
        let target_state = true; 
        let current_state = should_recover_daemon(AuthMode::SelfBuilt, false, false);
        
        assert_eq!(current_state, target_state, "SelfBuilt recovery policy needs to be enabled");
    }
}
