use crate::core::config::Config;
use connector_sdk::{GatewayClient, ClientOptions};
use anyhow::Result;
use tokio::signal;
use std::process::{Command, Stdio};
use std::env;
use std::fs;
use crate::daemon::dlq::DlqStore;
use crate::daemon::forwarder::Forwarder;
use crate::daemon::proxy::start_proxy;
use std::sync::Arc;
use crate::core::vault::{MultiVault, Vault};
use crate::core::security;
use crate::auth::{VaultTokenPool, AuthClient, pool::TokenPool, client::Client};
use crate::auth::models::Ticket;
use chrono::Utc;
use sysinfo::{System, SystemExt, ProcessExt, PidExt};

pub async fn start(profile: &str, config: &Config, proxy_port: u16, foreground: bool) -> Result<()> {
    let app_dir = crate::core::config::get_app_dir();
    let pid_file = app_dir.join(format!("{}_daemon.pid", profile));

    if !foreground {
        // PARENT PROCESS: Launch detached child
        let exe = std::fs::canonicalize(env::current_exe()?)?;
        
        // Use a temporary boot log to catch early errors
        let boot_log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(app_dir.join("logs").join("boot.log"))?;

        let child = Command::new(&exe)
            .arg("--profile")
            .arg(profile)
            .arg("daemon")
            .arg("start")
            .arg("--proxy-port")
            .arg(proxy_port.to_string())
            .arg("--foreground") // Child logic
            .env("APP_DIR_NAME", env!("APP_DIR_NAME"))
            .env("CARGO_BIN_NAME_OVERRIDE", env!("CARGO_BIN_NAME_OVERRIDE"))
            .stdin(Stdio::null())
            .stdout(Stdio::from(boot_log.try_clone()?))
            .stderr(Stdio::from(boot_log))
            .spawn()?;

        let pid = child.id();
        fs::write(&pid_file, pid.to_string())?;

        println!("🚀 Stream Bridge daemon launched in background (PID: {}).", pid);
        return Ok(());
    }

    // --- CHILD PROCESS LOGIC ---
    tracing::info!(target: "sys", "Daemon core logic starting for profile: {}", profile);

    let options = ClientOptions {
        app_key: config.app_key.clone(),
        app_secret: config.app_secret.clone(),
        encrypt_key: Some(config.encrypt_key.clone()),
        gateway_url: config.stream_url.clone(),
    };

    let client = GatewayClient::new(options);

    let fingerprint = security::get_machine_fingerprint()?;
    let seal_path = app_dir.join(".seal");
    let vault: Arc<dyn Vault> = Arc::new(MultiVault::new(seal_path, &fingerprint)?);
    let pool = Arc::new(VaultTokenPool::new(vault.clone()));

    // 1. Task: Local Proxy
    let p_profile_proxy = profile.to_string();
    let p_config_proxy = config.clone();
    let proxy_task = tokio::spawn(async move {
        if let Err(e) = start_proxy(&p_profile_proxy, &p_config_proxy, proxy_port).await {
            tracing::error!(target: "sys", error = %e, "Local Proxy Server crashed");
        }
    });

    let dlq = Arc::new(DlqStore::new(profile)?);
    let forwarder = Forwarder::new(dlq, &config.webhook_target);

    // Setup Dispatchers
    {
        let d = client.dispatcher();
        let mut dispatcher = d.lock().unwrap();

        let fwd = forwarder.clone();
        dispatcher.set_fallback_handler(Arc::new(move |msg| {
            let fwd_clone = fwd.clone();
            tokio::spawn(async move {
                fwd_clone.forward(msg).await;
            });
            true
        }));

        let p_pool = pool.clone();
        let p_profile = profile.to_string();
        let p_config = config.clone();
        
        dispatcher.on_app_ticket(move |msg| {
            let ticket_val = msg.biz_content.app_ticket.trim();
            tracing::info!(target: "stream", "AppTicket received from platform");
            
            let ticket = Ticket {
                value: ticket_val.to_string(),
                created_at: Utc::now(),
            };
            
            if let Err(e) = p_pool.set_app_ticket(&p_profile, &ticket) {
                tracing::error!(target: "sys", error = %e, "Failed to save ticket to vault");
            } else {
                tracing::info!(target: "sys", "AppTicket saved to vault correctly");
                let inner_pool = p_pool.clone();
                let inner_profile = p_profile.clone();
                let inner_config = p_config.clone();
                tokio::spawn(async move {
                    let auth = AuthClient::new(inner_pool.as_ref());
                    if let Err(e) = auth.get_app_access_token(&inner_profile, &inner_config).await {
                        tracing::error!(target: "sys", error = %e, "Automatic token refresh failed");
                    } else {
                        tracing::info!(target: "sys", "AccessToken proactively refreshed");
                    }
                });
            }
            true
        });

        dispatcher.on_ent_auth_code(|msg| {
            tracing::info!(target: "stream", code = %crate::core::utils::mask_tail(&msg.biz_content.temp_auth_code, 4), "Received TempAuthCode");
            true
        });
    }

    // 2. Task: Stream Bridge
    let stream_task = tokio::spawn(async move {
        match client.start().await {
            Ok(_) => {
                tracing::info!(target: "sys", "Stream Bridge started. Entering message loop...");
                // Keep the task alive as long as the client is running
                std::future::pending::<()>().await;
            }
            Err(e) => {
                tracing::error!(target: "sys", error = %e, "Stream Bridge failed to start");
            }
        }
    });

    // 3. Proactive refresh
    let auth_on_start = AuthClient::new(pool.as_ref());
    let _ = auth_on_start.get_app_access_token(profile, config).await;

    // 4. WAIT FOR ANY TASK OR SIGNAL
    tracing::info!(target: "sys", "All daemon tasks initialized. Entering watchdog mode.");
    
    tokio::select! {
        _ = proxy_task => tracing::error!(target: "sys", "Proxy task exited unexpectedly"),
        _ = stream_task => tracing::error!(target: "sys", "Stream task exited unexpectedly"),
        _ = wait_for_termination() => tracing::info!(target: "sys", "Termination signal received"),
    }

    tracing::info!(target: "sys", "Daemon process shutting down...");
    let _ = fs::remove_file(pid_file);
    Ok(())
}

use std::io::IsTerminal;

async fn wait_for_termination() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to register SIGTERM");
        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to register SIGINT");
        
        tokio::select! {
            _ = sigterm.recv() => tracing::info!(target: "sys", "Received SIGTERM"),
            _ = sigint.recv() => tracing::info!(target: "sys", "Received SIGINT"),
        };
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!(target: "sys", "Received Ctrl+C");
    }
}

pub async fn restart(profile: &str, config: &Config, proxy_port: u16, all: bool, cfg_mgr: &crate::core::config::ConfigManager) -> Result<()> {
    let mut s = System::new_all();
    s.refresh_processes();
    let current_pid = std::process::id();
    
    let mut targets: Vec<(u32, String, u16)> = Vec::new();
    for (pid, process) in s.processes() {
        let pid_u32 = pid.as_u32();
        if pid_u32 == current_pid { continue; }
        
        let cmd = process.cmd();
        let cmdline = cmd.join(" ");
        if cmdline.contains("daemon") && cmdline.contains("start") {
            let mut p_profile = "default".to_string();
            let mut p_port = 8080;
            
            for i in 0..cmd.len() {
                if cmd[i] == "--profile" && i + 1 < cmd.len() {
                    p_profile = cmd[i+1].clone();
                } else if cmd[i] == "--proxy-port" && i + 1 < cmd.len() {
                    p_port = cmd[i+1].parse().unwrap_or(8080);
                }
            }
            
            if all || p_profile == profile {
                targets.push((pid_u32, p_profile, p_port));
            }
        }
    }

    if targets.is_empty() {
        if !all {
            println!("📂 Daemon for profile '{}' is not running. Starting it now...", profile);
            start(profile, config, proxy_port, false).await?;
        }
        return Ok(());
    }

    for (pid, p_profile, p_port) in targets {
        println!("🔄 Restarting daemon for profile '{}' (PID: {}, Port: {})...", p_profile, pid, p_port);
        let _ = stop(&p_profile).await;
        let p_config = cfg_mgr.load(&p_profile).unwrap_or_else(|_| Config::default_with_profile(&p_profile));
        start(&p_profile, &p_config, p_port, false).await?;
    }
    
    Ok(())
}

pub async fn stop(profile: &str) -> Result<()> {
    let app_dir = crate::core::config::get_app_dir();
    let pid_file = app_dir.join(format!("{}_daemon.pid", profile));
    
    if pid_file.exists() {
        if let Ok(pid_str) = fs::read_to_string(&pid_file) {
            let pid_str = pid_str.trim();
            println!("🛑 Stopping daemon (PID: {})...", pid_str);
            
            #[cfg(unix)]
            let _ = Command::new("kill").arg("-15").arg(pid_str).status();
            
            #[cfg(windows)]
            let _ = Command::new("taskkill").arg("/F").arg("/PID").arg(pid_str).status();
            
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let _ = fs::remove_file(&pid_file);
            println!("✅ Daemon stopped for profile '{}'.", profile);
            return Ok(());
        }
    }
    
    println!("⚠️ Daemon is not running for profile '{}'.", profile);
    Ok(())
}
