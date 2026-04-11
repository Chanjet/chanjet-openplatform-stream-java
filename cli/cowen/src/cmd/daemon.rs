use crate::core::config::Config;
use sysinfo::{System, ProcessRefreshKind};
use connector_sdk::{GatewayClient, ClientOptions};
use anyhow::{Result, Context};
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

use std::io::IsTerminal;

pub async fn start(profile: &str, config: &Config, _proxy_port: u16, _enable_proxy: bool, foreground: bool, all: bool, cfg_mgr: &crate::core::config::ConfigManager, vault: &dyn Vault) -> Result<()> {
    let target_profiles = if all && !foreground {
        cfg_mgr.list_profiles()?
    } else {
        vec![profile.to_string()]
    };

    for p in target_profiles {
        let mut p_cfg = if p == profile { config.clone() } else { cfg_mgr.load(&p).unwrap_or_else(|_| Config::default_with_profile(&p)) };
        
        // VITAL: Inject secrets from vault for non-active profiles
        if p != profile {
            if let Ok(as_val) = vault.get(&p, "app_secret") { p_cfg.app_secret = as_val; }
            if let Ok(cert) = vault.get(&p, "certificate") { p_cfg.certificate = cert; }
            if let Ok(ek) = vault.get(&p, "encrypt_key") { p_cfg.encrypt_key = ek; }
        }
        
        let pid_file = crate::core::config::get_app_dir().join(format!("{}_daemon.pid", p));
        if all && pid_file.exists() {
            println!("ℹ️ Daemon for profile '{}' is already running. Skipping.", p);
            continue;
        }

        if let Err(e) = do_start(&p, &p_cfg, p_cfg.proxy_port, p_cfg.proxy_enabled, foreground).await {
            eprintln!("⚠️ Failed to start daemon for profile '{}': {}", p, e);
        }
        if all && !foreground {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
    Ok(())
}

async fn do_start(profile: &str, config: &Config, proxy_port: u16, enable_proxy: bool, foreground: bool) -> Result<()> {
    let app_dir = crate::core::config::get_app_dir();
    let pid_file = app_dir.join(format!("{}_daemon.pid", profile));

    if config.app_key.trim().is_empty() || config.app_secret.trim().is_empty() {
        anyhow::bail!("Cannot start daemon: AppKey or AppSecret is empty for profile '{}'. Please run 'cowen init' first.", profile);
    }

    // Fast-fail: Check if proxy port is available before launching the daemon
    if enable_proxy && !foreground {
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], proxy_port));
        if let Err(e) = std::net::TcpListener::bind(addr) {
            let err_msg = format!("Cannot start daemon: Proxy port {} is already in use or unavailable. Details: {}", proxy_port, e);
            tracing::error!(target: "sys", profile = %profile, port = %proxy_port, error = %e, "Port conflict detected");
            anyhow::bail!(err_msg);
        }
    }

    if !foreground {
        // PARENT PROCESS: Launch detached child
        let exe = std::fs::canonicalize(env::current_exe()?)?;
        
        let mut child_cmd = Command::new(&exe);
        child_cmd.arg("--profile")
            .arg(profile)
            .arg("daemon")
            .arg("start")
            .arg("--proxy-port")
            .arg(proxy_port.to_string());

        if enable_proxy {
            child_cmd.arg("--enable-proxy");
        }

        child_cmd.arg("--foreground") // Child logic
            // EXPLICITLY PASS IDENTITY via environment to ensure child knows its home
            .env("APP_DIR_NAME", option_env!("APP_DIR_NAME").unwrap_or_else(|| concat!(".", env!("CARGO_PKG_NAME"))))
            .env("CARGO_BIN_NAME_OVERRIDE", crate::core::utils::get_bin_name())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            child_cmd.process_group(0);
        }

        let spawned = child_cmd.spawn()?;
        let pid = spawned.id();
        
        // Give the OS a tiny moment to stabilize the session
        std::thread::sleep(std::time::Duration::from_millis(200));
        
        eprintln!("🚀 Stream Bridge daemon launched in background (PID: {}).", pid);
        return Ok(());
    }

    // --- CHILD PROCESS LOGIC ---
    // 1. CRITICAL: Write REAL child PID and BUILD_ID immediately
    let pid = std::process::id();
    // Double check we are in the right directory
    tracing::info!(target: "sys", "Daemon starting in directory: {:?}", app_dir);
    
    let build_id = env!("BUILD_ID");
    fs::write(&pid_file, format!("{}\n{}", pid, build_id))?;
    
    tracing::info!(target: "sys", "Daemon core logic starting for profile: {} (PID: {}, BUILD_ID: {})", profile, pid, build_id);

    let options = ClientOptions {
        app_key: config.app_key.clone(),
        app_secret: config.app_secret.clone(),
        encrypt_key: Some(config.encrypt_key.clone()),
        gateway_url: config.stream_url.clone(),
    };

    let client = Arc::new(GatewayClient::new(options));

    let fingerprint = security::get_machine_fingerprint()?;
    let seal_path = app_dir.join(".seal");
    let vault: Arc<dyn Vault> = Arc::new(MultiVault::new(seal_path, &fingerprint)?);
    let pool = Arc::new(VaultTokenPool::new(vault.clone()));

    // 1. Task: Local Proxy
    let p_profile_proxy = profile.to_string();
    let p_config_proxy = config.clone();
    let proxy_task = if enable_proxy {
        tokio::spawn(async move {
            match start_proxy(&p_profile_proxy, &p_config_proxy, proxy_port).await {
                Ok(_) => {
                    tracing::info!(target: "sys", "Local Proxy Server started. Entering message loop...");
                    std::future::pending::<()>().await;
                }
                Err(e) => {
                    tracing::error!(target: "sys", error = %e, "Local Proxy Server crashed");
                }
            }
        })
    } else {
        tokio::spawn(async move {
            tracing::info!(target: "sys", "Local Proxy Server is disabled.");
            std::future::pending::<()>().await;
        })
    };

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
    let stream_client = client.clone();
    let stream_task = tokio::spawn(async move {
        if let Err(e) = stream_client.start().await {
            tracing::error!(target: "sys", error = %e, "Stream Bridge loop terminated with error");
            Err(e)
        } else {
            tracing::info!(target: "sys", "Stream Bridge loop finished normally");
            Ok(())
        }
    });

    // 3. Task: Proactive maintenance
    let p_pool_task = pool.clone();
    let p_profile_task = profile.to_string();
    let p_config_task = config.clone();
    let maintenance_task = tokio::spawn(async move {
        // Wait a short moment for WebSocket to establish before requesting ticket
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        let auth = AuthClient::new(p_pool_task.as_ref());

        // Proactively request a ticket push if we don't have one cached
        if p_pool_task.get_app_ticket(&p_profile_task).is_err() {
            tracing::info!(target: "sys", "Initial AppTicket missing. Proactively requesting platform push...");
            let _ = auth.trigger_push(&p_profile_task, &p_config_task, false).await;
        }

        loop {
            tracing::info!(target: "sys", "Running daemon credential maintenance check...");
            match auth.get_app_access_token(&p_profile_task, &p_config_task).await {
                Ok(_) => tracing::info!(target: "sys", "Credential check: AccessToken is valid"),
                Err(e) => {
                    tracing::warn!(target: "sys", error = %e, "Credential check failed. Triggering platform push...");
                    let _ = auth.trigger_push(&p_profile_task, &p_config_task, false).await;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
        }
    });

    // 4. WAIT FOR ANY TASK OR SIGNAL
    tracing::info!(target: "sys", "All daemon tasks initialized. Entering watchdog mode.");
    
    let result = if std::io::stdout().is_terminal() {
        eprintln!("🚀 Stream Bridge running in foreground. Press Ctrl+C to stop.");
        tokio::select! {
            res = proxy_task => { 
                tracing::error!(target: "sys", "Proxy task exited unexpectedly"); 
                res.map_err(|e| anyhow::anyhow!("Proxy task panicked: {}", e)).and_then(|_| Err(anyhow::anyhow!("Proxy task stopped")))
            },
            res = stream_task => { 
                tracing::error!(target: "sys", "Stream task exited unexpectedly"); 
                res.map_err(|e| anyhow::anyhow!("Stream task panicked: {}", e))
                   .and_then(|r| r.context("Stream client crashed"))
            },
            res = maintenance_task => { 
                tracing::error!(target: "sys", "Maintenance task exited unexpectedly"); 
                res.map_err(|e| anyhow::anyhow!("Maintenance task panicked: {}", e)).and_then(|_| Err(anyhow::anyhow!("Maintenance task stopped")))
            },
            _ = signal::ctrl_c() => { tracing::info!(target: "sys", "Interrupted by user"); Ok(()) },
        }
    } else {
        // Background child process: Entering persistent loop
        tracing::info!(target: "sys", "Daemon running in managed background mode. Entering persistent loop...");
        tokio::select! {
            res = proxy_task => { 
                tracing::error!(target: "sys", "Proxy task exited unexpectedly"); 
                res.map_err(|e| anyhow::anyhow!("Proxy task panicked: {}", e)).and_then(|_| Err(anyhow::anyhow!("Proxy task stopped")))
            },
            res = stream_task => { 
                tracing::error!(target: "sys", "Stream task exited unexpectedly"); 
                res.map_err(|e| anyhow::anyhow!("Stream task panicked: {}", e))
                   .and_then(|r| r.context("Stream client crashed"))
            },
            res = maintenance_task => { 
                tracing::error!(target: "sys", "Maintenance task exited unexpectedly"); 
                res.map_err(|e| anyhow::anyhow!("Maintenance task panicked: {}", e)).and_then(|_| Err(anyhow::anyhow!("Maintenance task stopped")))
            },
            _ = wait_for_termination() => { tracing::info!(target: "sys", "Termination signal received"); Ok(()) },
        }
    };

    tracing::info!(target: "sys", "Daemon process shutting down...");
    
    // Safety: Ensure client is stopped
    client.stop();
    
    // CRITICAL: Always remove PID file before exit to prevent phantom status
    if let Err(e) = fs::remove_file(&pid_file) {
        tracing::error!(target: "sys", error = %e, "Failed to remove PID file during shutdown");
    } else {
        tracing::info!(target: "sys", "PID file removed safely");
    }

    result
}

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

pub async fn restart(profile: &str, config: &Config, _proxy_port: u16, _enable_proxy: bool, all: bool, cfg_mgr: &crate::core::config::ConfigManager, vault: &dyn Vault) -> Result<()> {
    let target_profiles = if all {
        cfg_mgr.list_profiles()?
    } else {
        vec![profile.to_string()]
    };

    for p in target_profiles {
        let mut p_cfg = if p == profile { config.clone() } else { cfg_mgr.load(&p).unwrap_or_else(|_| Config::default_with_profile(&p)) };
        
        // VITAL: Inject secrets from vault for non-active profiles
        if p != profile {
            if let Ok(as_val) = vault.get(&p, "app_secret") { p_cfg.app_secret = as_val; }
            if let Ok(cert) = vault.get(&p, "certificate") { p_cfg.certificate = cert; }
            if let Ok(ek) = vault.get(&p, "encrypt_key") { p_cfg.encrypt_key = ek; }
        }
        let app_dir = crate::core::config::get_app_dir();
        let pid_file = app_dir.join(format!("{}_daemon.pid", p));
        
        let is_running = pid_file.exists();
        
        if is_running {
            eprintln!("🔄 Restarting daemon for profile '{}'...", p);
            let _ = do_stop(&p).await;
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let _ = do_start(&p, &p_cfg, p_cfg.proxy_port, p_cfg.proxy_enabled, false).await;
        } else if !all {
            eprintln!("📂 Daemon for profile '{}' is not running. Starting it now...", p);
            let _ = do_start(&p, &p_cfg, p_cfg.proxy_port, p_cfg.proxy_enabled, false).await;
        }
        
        if all {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
    }
    
    Ok(())
}

pub async fn stop(profile: &str, all: bool, cfg_mgr: &crate::core::config::ConfigManager) -> Result<()> {
    let target_profiles = if all {
        cfg_mgr.list_profiles()?
    } else {
        vec![profile.to_string()]
    };

    for p in target_profiles {
        // Double punch: Standard stop + Ghost cleanup
        let _ = do_stop(&p).await;
        let _ = kill_ghost_processes(&p).await;
        
        if all {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
    
    Ok(())
}

/// Polls for process exit with timeout and SIGKILL fallback
async fn wait_for_death(pid: u32, timeout_ms: u64) -> bool {
    let mut sys = System::new_all();
    let start = std::time::Instant::now();
    let pid_obj = sysinfo::Pid::from(pid as usize);

    while start.elapsed().as_millis() < timeout_ms as u128 {
        sys.refresh_all();
        if sys.process(pid_obj).is_none() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    false
}

/// Scans the system process list for cowen processes belonging to the profile
async fn kill_ghost_processes(profile: &str) -> Result<()> {
    let mut sys = System::new_all();
    sys.refresh_all();
    
    let pattern = format!("--profile {}", profile);
    let mut killed_any = false;
    
    for (pid, process) in sys.processes() {
        let cmd = process.cmd()
            .iter()
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");

        if cmd.contains("cowen") && cmd.contains(&pattern) && cmd.contains("daemon") && cmd.contains("start") {
            tracing::warn!(target: "sys", profile = %profile, pid = %pid, "Killing ghost/foreground daemon process");
            eprintln!("🧹 Cleaning up ghost/foreground daemon for profile '{}' (PID: {})...", profile, pid);
            
            #[cfg(unix)]
            let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
            
            #[cfg(windows)]
            let _ = Command::new("taskkill").arg("/F").arg("/PID").arg(pid.to_string()).status();

            killed_any = true;
        }
    }
    
    if killed_any {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    
    Ok(())
}

async fn do_stop(profile: &str) -> Result<()> {
    let app_dir = crate::core::config::get_app_dir();
    let pid_file = app_dir.join(format!("{}_daemon.pid", profile));
    if pid_file.exists() {
        if let Ok(pid_content) = fs::read_to_string(&pid_file) {
            if let Some(pid_str) = pid_content.lines().next() {
                let pid_str = pid_str.trim();
                if let Ok(pid) = pid_str.parse::<u32>() {
                    eprintln!("🛑 Stopping daemon (PID: {}) for profile '{}'...", pid, profile);

                    #[cfg(unix)]
                    let _ = Command::new("kill").arg("-15").arg(pid_str).status();

                    #[cfg(windows)]
                    let _ = Command::new("taskkill").arg("/F").arg("/PID").arg(pid_str).status();
                    
                    // Wait for it to die gracefully
                    if !wait_for_death(pid, 2000).await {
                        eprintln!("⚠️  Daemon (PID: {}) timed out. Escaling to SIGKILL...", pid);
                        #[cfg(unix)]
                        let _ = Command::new("kill").arg("-9").arg(pid_str).status();
                        
                        // Final check
                        if !wait_for_death(pid, 1000).await {
                            tracing::error!(target: "sys", profile = %profile, pid = %pid, "Failed to kill process even with SIGKILL");
                        }
                    }
                }
            }
            if let Err(_) = fs::remove_file(&pid_file) {
                // Ignore errors if it was already deleted
            }
            eprintln!("✅ Daemon stopped for profile '{}'.", profile);
            return Ok(());
        }
    }
    
    println!("ℹ️  Daemon is already offline for profile '{}'.", profile);
    Ok(())
}
