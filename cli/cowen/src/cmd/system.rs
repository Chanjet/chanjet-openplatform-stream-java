use crate::core::vault::Vault;
use crate::core::config::ConfigManager;
use anyhow::Result;
use serde::Serialize;
use chrono::{Local, DateTime, Utc};
use sysinfo::System;

#[derive(Serialize)]
pub struct SystemStatus {
    pub profile: String,
    pub config: ConfigStatus,
    pub security: SecurityStatus,
    pub token: Option<TokenStatus>,
    pub ticket: Option<TicketStatus>,
    pub daemon: DaemonStatus,
}

#[derive(Serialize)]
pub struct ConfigStatus {
    pub app_key: String,
    pub app_secret: String,
    pub certificate: String,
    pub encrypt_key: String,
    pub openapi_url: String,
    pub stream_url: String,
}

#[derive(Serialize)]
pub struct SecurityStatus {
    pub vault_ok: bool,
    pub missing_secrets: Vec<String>,
}

#[derive(Serialize)]
pub struct TokenStatus {
    pub status: String,
    pub expires_at: DateTime<Utc>,
    pub real_expires_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct TicketStatus {
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub log_path: Option<String>,
    pub build_id: Option<String>,
}

pub async fn status(
    active_profile: &str,
    cfg_mgr: &crate::core::config::ConfigManager,
    vault: &dyn Vault,
    format: &str,
    all: bool,
) -> Result<()> {
    let profiles = if all {
        cfg_mgr.list_profiles()?
    } else {
        vec![active_profile.to_string()]
    };

    let mut statuses = Vec::new();
    let mut errors = Vec::new();
    for profile in &profiles {
        match get_system_status(profile, cfg_mgr, vault).await {
            Ok(s) => statuses.push(s),
            Err(e) => errors.push((profile.clone(), e)),
        }
    }

    if format == "json" || format == "yaml" {
        if all {
            // Include errors in JSON/YAML if needed, but for now just the successful ones
            return crate::core::utils::render(&statuses, format);
        } else if let Some(s) = statuses.first() {
            return crate::core::utils::render(s, format);
        }
        return Ok(());
    }

    let bin_name = crate::core::utils::get_bin_name().to_uppercase();
    if all {
        println!("🔍 {} System Status Diagnostics (All Profiles)", bin_name);
        println!("==================================================");
    }
    
    for full_status in statuses {
        print_single_status(&bin_name, &full_status, all);
        if all {
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
    vault: &dyn Vault,
) -> Result<SystemStatus> {
    let cfg = cfg_mgr.load(profile)?;
    
    // 1. Config
    let config_status = ConfigStatus {
        app_key: cfg.app_key.clone(),
        app_secret: crate::core::utils::mask_string(&cfg.app_secret),
        certificate: crate::core::utils::mask_string(&cfg.certificate),
        encrypt_key: crate::core::utils::mask_string(&cfg.encrypt_key),
        openapi_url: cfg.openapi_url.clone(),
        stream_url: cfg.stream_url.clone(),
    };

    // 2. Vault
    let mut missing_secrets = Vec::new();
    if vault.get(profile, "app_secret").is_err() { missing_secrets.push("app_secret".to_string()); }
    if vault.get(profile, "certificate").is_err() { missing_secrets.push("certificate".to_string()); }
    if vault.get(profile, "encrypt_key").is_err() { missing_secrets.push("encrypt_key".to_string()); }
    
    let security_status = SecurityStatus {
        vault_ok: missing_secrets.is_empty(),
        missing_secrets,
    };

    // 3. Token
    let token_status = if let (Ok(val), Ok(exp_str), Ok(created_str)) = (
        vault.get(profile, "access_token"),
        vault.get(profile, "access_token_expires"),
        vault.get(profile, "access_token_created")
    ) {
        let expires_at = chrono::DateTime::parse_from_rfc3339(&exp_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(Utc::now());
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(Utc::now());
            
        let token = crate::auth::models::Token {
            value: val,
            expires_at,
            created_at,
        };
        
        Some(TokenStatus {
            status: if token.is_expired() { "EXPIRED".to_string() } else { "VALID".to_string() },
            expires_at: token.expires_at,
            real_expires_at: token.real_expires_at(),
        })
    } else { None };

    // 4. Ticket
    let ticket_status = if let Ok(_ticket_raw) = vault.get(profile, "app_ticket") {
        let created_at = if let Ok(ts_str) = vault.get(profile, "app_ticket_created") {
            chrono::DateTime::parse_from_rfc3339(&ts_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or(Utc::now())
        } else {
            Utc::now()
        };
        Some(TicketStatus {
            status: "CACHED".to_string(),
            created_at,
        })
    } else { None };

    // 5. Daemon detection
    let (found_daemon_pid, found_build_id) = get_active_daemon_info(profile).await;

    let daemon_status = DaemonStatus {
        running: found_daemon_pid.is_some(),
        pid: found_daemon_pid,
        log_path: found_daemon_pid.map(|_| {
            app_dir().join("logs").join(format!("{}_sys.log", profile)).to_string_lossy().to_string()
        }),
        build_id: found_build_id,
    };

    Ok(SystemStatus {
        profile: profile.to_string(),
        config: config_status,
        security: security_status,
        token: token_status,
        ticket: ticket_status,
        daemon: daemon_status,
    })
}

fn print_single_status(bin_name: &str, full_status: &SystemStatus, all: bool) {
    if !all {
        println!("🔍 {} System Status Diagnostics (Profile: '{}')", bin_name, full_status.profile);
        println!("--------------------------------------------------");
    } else {
        println!("▶ Profile: '{}'", full_status.profile);
    }

    if !full_status.config.app_key.is_empty() {
        println!("  ⚙️  Configuration: [OK] AppKey: {}", full_status.config.app_key);
    } else {
        println!("  ⚙️  Configuration: [MISSING] Profile not initialized or AppKey empty.");
    }
    println!("     - OpenAPI: {}", full_status.config.openapi_url);
    println!("     - Stream:  {}", full_status.config.stream_url);

    if full_status.security.vault_ok {
        println!("  🛡️  Security (Vault): [OK] All core secrets are securely stored.");
    } else {
        println!("  🛡️  Security (Vault): [PARTIAL] Missing: {}", full_status.security.missing_secrets.join(", "));
    }

    if let Some(token) = &full_status.token {
        let real_expiry = token.real_expires_at.with_timezone(&Local);
        println!("  🔑 AccessToken: [{}] (Expires: {})", token.status, real_expiry.format("%Y-%m-%d %H:%M:%S"));
    } else {
        println!("  🔑 AccessToken: [NONE] (未获取到有效令牌)");
    }

    if let Some(ticket) = &full_status.ticket {
        let created = ticket.created_at.with_timezone(&Local);
        println!("  🎫 AppTicket:   [{}] (Received: {})", ticket.status, created.format("%Y-%m-%d %H:%M:%S"));
    } else {
        println!("  🎫 AppTicket:   [NONE] (等待 Daemon 接收推送)");
    }

    if full_status.daemon.running {
        println!("  📟 Daemon Process: [RUNNING] (PID: {})", full_status.daemon.pid.unwrap());
        if let Some(log) = &full_status.daemon.log_path {
            println!("     - Logs: {}", log);
        }
    } else {
        println!("  📟 Daemon Process: [OFFLINE] (未检测到活跃后台进程)");
    }
    
    if !all {
        println!("--------------------------------------------------");
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

pub async fn ensure_daemon_running(profile: &str, config: &crate::core::config::Config, cfg_mgr: &crate::core::config::ConfigManager) -> Result<()> {
    let profiles = cfg_mgr.list_profiles().unwrap_or_else(|_| vec![profile.to_string()]);
    
    for p in profiles {
        let (pid, build_id) = get_active_daemon_info(&p).await;
        let p_cfg = if p == profile { config.clone() } else { cfg_mgr.load(&p).unwrap_or_else(|_| crate::core::config::Config::default_with_profile(&p)) };
        
        match pid {
            Some(pid_val) => {
                // 进程存在，检查版本
                let current_build_id = env!("BUILD_ID");
                let needs_restart = match build_id {
                    Some(bid) => bid != current_build_id,
                    None => true,
                };

                if needs_restart {
                    eprintln!("🔄 Detecting outdated daemon (PID: {}) for profile '{}'. Automatically restarting...", pid_val, p);
                    let _ = crate::cmd::daemon::restart(&p, &p_cfg, p_cfg.proxy_port, p_cfg.proxy_enabled, false, cfg_mgr).await;
                }
            }
            None => {
                // 进程不存在（如重启后），静默拉起
                let pid_file = app_dir().join(format!("{}_daemon.pid", p));
                if pid_file.exists() || p == profile {
                    if !p_cfg.app_key.is_empty() && !p_cfg.app_secret.is_empty() {
                        tracing::info!(target: "sys", "Daemon offline for profile '{}'. Auto-launching...", p);
                        if p == profile {
                            eprintln!("🚀 Daemon is offline. Automatically launching in background...");
                        } else {
                            eprintln!("🚀 Recovering offline daemon for profile '{}'...", p);
                        }
                        let _ = crate::cmd::daemon::start(&p, &p_cfg, p_cfg.proxy_port, p_cfg.proxy_enabled, false, false, cfg_mgr).await;
                        // 缓冲
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                }
            }
        }
    }
    Ok(())
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
    
    // 1. Clear regular profile-specific files
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

    // 2. Clear DLQ directory for this profile
    let dlq_dir = app_dir.join("dlq").join(_profile);
    if dlq_dir.exists() {
        let _ = std::fs::remove_dir_all(&dlq_dir);
    }

    // 3. Clear all related logs (including rotated ones like prod.log.1)
    let log_dir = app_dir.join("logs");
    if log_dir.exists() {
        let prefix = format!("{}_", _profile);
        if let Ok(entries) = std::fs::read_dir(log_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(&prefix) {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
    
    eprintln!("✨ Profile '{}' reset complete.", _profile);
    Ok(())
}
