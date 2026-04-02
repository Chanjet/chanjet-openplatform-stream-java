use crate::core::vault::Vault;
use crate::core::config::ConfigManager;
use anyhow::Result;
use serde::Serialize;
use crate::auth::models::{Token, Ticket};
use chrono::{Local, DateTime, Utc};
use sysinfo::{ProcessExt, System, SystemExt, PidExt};

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
}

pub async fn status(
    profile: &str,
    cfg_mgr: &crate::core::config::ConfigManager,
    vault: &dyn Vault,
    format: &str,
) -> Result<()> {
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
    let token_status = if let Ok(token_raw) = vault.get(profile, "access_token") {
        if let Ok(token) = serde_json::from_str::<Token>(&token_raw) {
            Some(TokenStatus {
                status: if token.is_expired() { "EXPIRED".to_string() } else { "VALID".to_string() },
                expires_at: token.expires_at,
                real_expires_at: token.real_expires_at(),
            })
        } else { None }
    } else { None };

    // 4. Ticket
    let ticket_status = if let Ok(ticket_raw) = vault.get(profile, "app_ticket") {
        if let Ok(ticket) = serde_json::from_str::<Ticket>(&ticket_raw) {
            Some(TicketStatus {
                status: "CACHED".to_string(),
                created_at: ticket.created_at,
            })
        } else { None }
    } else { None };

    // 5. Daemon detection: PID file based (More reliable for renamed binaries like owenc-test)
    let app_dir = crate::core::config::get_app_dir();
    let pid_file = app_dir.join(format!("{}_daemon.pid", profile));
    let mut found_daemon_pid = None;

    if pid_file.exists() {
        if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
            if let Ok(pid_val) = pid_str.trim().parse::<u32>() {
                let mut s = System::new_all();
                s.refresh_processes();
                
                // Check if process exists and is actually a cowen daemon
                if let Some(process) = s.process(sysinfo::Pid::from_u32(pid_val)) {
                    let cmdline = process.cmd().join(" ");
                    let name = process.name().to_lowercase();
                    // Match if name contains 'cowen' (base identity) OR command line contains 'daemon'
                    if name.contains("cowen") || cmdline.contains("daemon") {
                        found_daemon_pid = Some(pid_val);
                    }
                }
            }
        }
    }

    let daemon_status = DaemonStatus {
        running: found_daemon_pid.is_some(),
        pid: found_daemon_pid,
        log_path: found_daemon_pid.map(|_| {
            app_dir.join("logs").join(format!("{}.log", profile)).to_string_lossy().to_string()
        }),
    };

    let full_status = SystemStatus {
        profile: profile.to_string(),
        config: config_status,
        security: security_status,
        token: token_status,
        ticket: ticket_status,
        daemon: daemon_status,
    };

    if format == "json" || format == "yaml" {
        return crate::core::utils::render(&full_status, format);
    }

    // Default Text Output
    println!("🔍 CJTC System Status Diagnostics (Profile: '{}')", profile);
    println!("--------------------------------------------------");

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

    if let Some(token) = full_status.token {
        let real_expiry = token.real_expires_at.with_timezone(&Local);
        println!("  🔑 AccessToken: [{}] (Expires: {})", token.status, real_expiry.format("%Y-%m-%d %H:%M:%S"));
    } else {
        println!("  🔑 AccessToken: [NONE] (未获取到有效令牌)");
    }

    if let Some(ticket) = full_status.ticket {
        let created = ticket.created_at.with_timezone(&Local);
        println!("  🎫 AppTicket:   [{}] (Received: {})", ticket.status, created.format("%Y-%m-%d %H:%M:%S"));
    } else {
        println!("  🎫 AppTicket:   [NONE] (等待 Daemon 接收推送)");
    }

    if full_status.daemon.running {
        println!("  📟 Daemon Process: [RUNNING] (PID: {})", full_status.daemon.pid.unwrap());
        if let Some(log) = full_status.daemon.log_path {
            println!("     - Logs: {}", log);
        }
    } else {
        println!("  📟 Daemon Process: [OFFLINE] (未检测到活跃后台进程)");
    }
    
    println!("--------------------------------------------------");
    Ok(())
}

pub async fn config(
    _profile: &str,
    cfg_mgr: &ConfigManager,
    format: &str,
) -> Result<()> {
    let cfg = cfg_mgr.load(_profile)?;
    if format == "json" || format == "yaml" {
        crate::core::utils::render(&cfg, format)?;
    } else {
        println!("{:#?}", cfg);
    }
    Ok(())
}

pub async fn reset(
    _profile: &str,
    cfg_mgr: &ConfigManager,
    vault: Option<&dyn Vault>,
) -> Result<()> {
    println!("Resetting profile '{}'...", _profile);
    
    // 1. Stop daemon if running
    if let Err(e) = crate::cmd::daemon::stop(_profile).await {
        tracing::warn!(target: "sys", profile = %_profile, error = %e, "Failed to stop daemon during reset");
    }

    // 2. Clear Vault if available
    if let Some(v) = vault {
        if let Err(e) = v.clear(_profile) {
            eprintln!("⚠️ Warning: Failed to clear vault for profile '{}': {}", _profile, e);
        } else {
            println!("✅ Vault secrets cleared.");
        }
    }

    // 3. Physical file cleanup
    let app_dir = crate::core::config::get_app_dir();
    
    // Files to remove
    let targets = vec![
        app_dir.join(format!("{}.yaml", _profile)),
        app_dir.join(format!("{}_openapi.json", _profile)),
        app_dir.join(format!("{}_openapi.idx", _profile)),
        app_dir.join(format!("{}_daemon.pid", _profile)),
        app_dir.join("logs").join(format!("{}.log", _profile)),
    ];

    for path in targets {
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                eprintln!("⚠️ Failed to delete {:?}: {}", path, e);
            } else {
                println!("✅ Deleted {:?}", path.file_name().unwrap_or_default());
            }
        }
    }

    println!("✨ Profile '{}' reset complete. You can run 'init' to start fresh.", _profile);
    Ok(())
}
