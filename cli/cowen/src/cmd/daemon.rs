use cowen_common::config::Config;
use cowen_config::ConfigManager;
pub use cowen_server::cmd::service;
use anyhow::Result;
use std::process::Command;
use std::fs;
use std::sync::Arc;
use cowen_common::vault::Vault;
use cowen_common::daemon::DaemonService;

use cowen_monitor::telemetry::TelemetryControl;

fn is_daemon_alive() -> bool {
    let app_dir = cowen_common::config::get_app_dir();
    let pid_file = app_dir.join("master_daemon.pid");
    if pid_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&pid_file) {
            if let Some(pid_str) = content.lines().next() {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    let mut s = sysinfo::System::new();
                    let sys_pid = sysinfo::Pid::from_u32(pid);
                    s.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[sys_pid]), true);
                    return s.process(sys_pid).is_some();
                }
            }
        }
    }
    false
}

/// 启动守护进程 (主分发器)
pub async fn start(
    profile: &str, 
    config: &Config, 
    _proxy_port: u16, 
    _enable_proxy: bool, 
    foreground: bool, 
    all: bool, 
    cfg_mgr: &ConfigManager, 
    vault: Arc<dyn Vault>, 
    _telemetry: Option<Arc<TelemetryControl>>,
    daemon_svc: Arc<dyn DaemonService>,
) -> Result<()> {
    #[cfg(unix)]
    {
        // On Unix, we use the standalone cowen-daemon binary via IPC.
        if !foreground {
            eprintln!("🚀 Triggering standalone daemon for profile '{}'...", profile);
            
            // Check if daemon is running by attempting a ping or checking socket
            let uds_path = cowen_common::ipc::get_uds_path();
            let _ipc_client = cowen_common::ipc::client::IpcDaemonService::new(uds_path.clone());
            
            // Wait, we can't easily ping, so we'll try to spawn if not exists.
            // Actually, if we spawn it detached, we can then send the command.
            if !uds_path.exists() {
                eprintln!("ℹ️ Daemon process not running. Spawning in background...");
                let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
                let daemon_path = std::env::var("COWEN_DAEMON_BIN").map(std::path::PathBuf::from).unwrap_or_else(|_| exe_dir.join("cowen-daemon"));
                
                let app_dir = cowen_common::config::get_app_dir();
                let log_dir = app_dir.join("logs");
                if !log_dir.exists() { let _ = std::fs::create_dir_all(&log_dir); }
                let stdout_file = std::fs::OpenOptions::new().create(true).append(true).open(log_dir.join("daemon.stdout.log"))?;
                let stderr_file = std::fs::OpenOptions::new().create(true).append(true).open(log_dir.join("daemon.stderr.log"))?;

                let _child = Command::new(&daemon_path)
                    .arg("--uds")
                    .arg(&uds_path)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::from(stdout_file))
                    .stderr(std::process::Stdio::from(stderr_file))
                    .spawn()?;
                
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            }

            // We must use the specific ipc client instead of the injected one because the injected one might be ServerDaemonService if not set up correctly in lib.rs for this path.
            // Actually `daemon_svc` is correct.
            let err_res = daemon_svc.start_daemon(profile, config, vault.clone()).await;
            if let Err(e) = err_res {
                // If the initial connection failed, check if the process is actually dead.
                // If the socket exists but the process is dead, we clean up the socket and spawn it.
                if !uds_path.exists() || !is_daemon_alive() {
                     eprintln!("ℹ️ Daemon socket stale or process dead. Spawning in background...");
                     if uds_path.exists() { let _ = std::fs::remove_file(&uds_path); }
                     
                     let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
                     let daemon_path = std::env::var("COWEN_DAEMON_BIN").map(std::path::PathBuf::from).unwrap_or_else(|_| exe_dir.join("cowen-daemon"));
                     
                     let app_dir = cowen_common::config::get_app_dir();
                     let log_dir = app_dir.join("logs");
                     if !log_dir.exists() { let _ = std::fs::create_dir_all(&log_dir); }
                     let stdout_file = std::fs::OpenOptions::new().create(true).append(true).open(log_dir.join("daemon.stdout.log"))?;
                     let stderr_file = std::fs::OpenOptions::new().create(true).append(true).open(log_dir.join("daemon.stderr.log"))?;

                     let _child = Command::new(&daemon_path)
                         .arg("--uds")
                         .arg(&uds_path)
                         .stdin(std::process::Stdio::null())
                         .stdout(std::process::Stdio::from(stdout_file))
                         .stderr(std::process::Stdio::from(stderr_file))
                         .spawn()?;
                     
                     tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                     
                     // Try again
                     let ipc_client2 = cowen_common::ipc::client::IpcDaemonService::new(uds_path.clone());
                     ipc_client2.start_daemon(profile, config, vault).await.map_err(|e2| anyhow::anyhow!(e2))?;
                } else {
                     return Err(anyhow::anyhow!("Daemon is running but failed to respond: {}", e));
                }
            }
            eprintln!("✅ Startup command sent to daemon.");
            return Ok(());
        } else {
            // Foreground mode on Unix: we must spawn cowen-daemon as a child and wait for it,
            // so that launchd/systemd can monitor the process, while still keeping UDS IPC alive.
            let uds_path = cowen_common::ipc::get_uds_path();
            
            // Spawn the daemon in the foreground
            let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
            let daemon_path = std::env::var("COWEN_DAEMON_BIN").map(std::path::PathBuf::from).unwrap_or_else(|_| exe_dir.join("cowen-daemon"));
            
            let mut child = Command::new(&daemon_path)
                .arg("--uds")
                .arg(&uds_path)
                .spawn()?;
            
            let child_id = child.id();
            eprintln!("🚀 Starting cowen-daemon in foreground (PID: {})...", child_id);
            
            // Forward signals (SIGTERM, Ctrl+C) to child process for graceful shutdown
            tokio::spawn(async move {
                if let Ok(mut sigterm) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    tokio::select! {
                        _ = tokio::signal::ctrl_c() => {},
                        _ = sigterm.recv() => {},
                    }
                    eprintln!("ℹ️ Received stop signal, forwarding to child daemon...");
                    let _ = std::process::Command::new("kill").arg("-15").arg(child_id.to_string()).status();
                }
            });
            
            // Wait briefly for UDS to be ready
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            
            // Send the start commands via IPC
            use cowen_common::daemon::DaemonService;
            let ipc_client = cowen_common::ipc::client::IpcDaemonService::new(uds_path.clone());
            let target_profiles = if all { cfg_mgr.list_profiles().await? } else { vec![profile.to_string()] };
            for p in target_profiles {
                let p_cfg = if p == profile { config.clone() } else { cfg_mgr.load(&p).await.unwrap_or_else(|_| Config::default_with_profile(&p)) };
                if let Err(e) = ipc_client.start_daemon(&p, &p_cfg, vault.clone()).await {
                    eprintln!("⚠️ Failed to send start command to daemon: {}", e);
                }
            }
            
            eprintln!("✅ Startup commands sent to foreground daemon. Blocking...");
            
            // Wait for child to exit
            let status = child.wait()?;
            eprintln!("ℹ️ cowen-daemon exited with status: {}", status);
            return Ok(());
        }
    }

    #[cfg(not(unix))]
    if !foreground {
        // Parent process logic: spawn itself with --foreground
        let app_dir = cowen_common::config::get_app_dir();
        let pid_file = app_dir.join("master_daemon.pid");

        // Check for existing master
        if pid_file.exists() {
             if let Ok(content) = fs::read_to_string(&pid_file) {
                 if let Some(pid_str) = content.lines().next() {
                     if let Ok(pid) = pid_str.trim().parse::<u32>() {
                          let mut s = sysinfo::System::new();
                          let sys_pid = sysinfo::Pid::from_u32(pid);
                          s.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[sys_pid]), true);
                          if s.process(sys_pid).is_some() {
                             eprintln!("ℹ️ Master daemon is already running (PID: {}).", pid);
                             return Ok(());
                         }
                     }
                 }
             }
        }

        let exe = std::fs::canonicalize(std::env::current_exe()?)?;
        let mut child_cmd = Command::new(&exe);
        let app_dir = cowen_common::config::get_app_dir();
        let log_dir = app_dir.join("logs");
        if !log_dir.exists() { let _ = fs::create_dir_all(&log_dir); }
        let stdout_path = log_dir.join("master_daemon.stdout.log");
        let stderr_path = log_dir.join("master_daemon.stderr.log");

        let stdout_file = std::fs::OpenOptions::new().create(true).append(true).open(stdout_path)?;
        let stderr_file = std::fs::OpenOptions::new().create(true).append(true).open(stderr_path)?;

        child_cmd.arg("--profile").arg(profile).arg("daemon").arg("start")
            .arg("--foreground")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::from(stdout_file))
            .stderr(std::process::Stdio::from(stderr_file));
        
        if all { child_cmd.arg("--all"); }

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            child_cmd.process_group(0);
        }

        let mut child_proc = child_cmd.spawn()?;
        let pid = child_proc.id();
        eprintln!("🚀 Launching master daemon (PID: {})...", pid);
        
        // Wait for PID file to have content
        let mut ready = false;
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if let Ok(content) = fs::read_to_string(&pid_file) {
                if !content.trim().is_empty() {
                    ready = true;
                    break;
                }
            }
            // If child exited, stop waiting
            if let Ok(Some(status)) = child_proc.try_wait() {
                if !status.success() {
                    anyhow::bail!("Master daemon exited with error.");
                }
                break;
            }
        }
        
        if ready {
            eprintln!("✅ Master daemon started successfully.");
            return Ok(());
        } else {
            anyhow::bail!("Master daemon failed to start within timeout or exited.");
        }
    }

    #[cfg(not(unix))]
    {
        // --- Master Process (Foreground) ---
        let app_dir = cowen_common::config::get_app_dir();
        let pid_file = app_dir.join("master_daemon.pid");
    
    // Acquire exclusive lock
    let pid_file_handle = std::fs::OpenOptions::new().create(true).write(true).open(&pid_file)?;
    use fs2::FileExt;
    if pid_file_handle.try_lock_exclusive().is_err() {
        eprintln!("⚠️ Master daemon is already running (PID file is locked). Exiting.");
        return Ok(());
    }

    // Set process name
    cowen_common::utils::set_process_name("cowen:master");

    // Start Monitor Server
    let app_cfg = cfg_mgr.load_app_config().await?;
    let mut m_port = app_cfg.monitor_port; // might be 0
    let mut allow_fallback = false;
    if m_port == 0 {
        m_port = 1588;
        allow_fallback = true;
    }
    let m_server = cowen_monitor::MonitorServer::new(m_port, daemon_svc.clone(), None);
    let (port_tx, port_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let _ = m_server.start(Some(port_tx), allow_fallback).await;
    });
    
    let actual_m_port = match tokio::time::timeout(tokio::time::Duration::from_secs(5), port_rx).await {
        Ok(Ok(p)) => p,
        Ok(Err(_)) => {
            error!(target: "sys", "Monitor server failed to start (e.g., port occupied). Aborting.");
            return Err(anyhow::anyhow!("Monitor server failed to start. Port may be occupied."));
        }
        Err(_) => {
            error!(target: "sys", "Timed out waiting for monitor server to start. Aborting.");
            return Err(anyhow::anyhow!("Monitor server start timeout"));
        }
    };
    
    if actual_m_port > 0 {
        info!(target: "sys", "Master monitor server started on port {}", actual_m_port);
        // Rewrite PID file with Monitor Port
        fs::write(
            &pid_file, 
            format!(
                "{}\nBUILD_ID={}\nBUILD_TIME={}\nMONITOR_PORT={}", 
                std::process::id(), 
                cowen_common::BUILD_ID, 
                cowen_common::BUILD_TIME,
                actual_m_port
            )
        )?;
    } else {
        fs::write(
            &pid_file, 
            format!(
                "{}\nBUILD_ID={}\nBUILD_TIME={}", 
                std::process::id(), 
                cowen_common::BUILD_ID, 
                cowen_common::BUILD_TIME
            )
        )?;
    }

    // Identify profiles to start
    let target_profiles = if all {
        cfg_mgr.list_profiles().await?
    } else {
        vec![profile.to_string()]
    };

    for p in target_profiles {
        info!(target: "sys", profile = %p, "Master starting worker for profile");
        let mut p_cfg = if p == profile { config.clone() } else { 
            cfg_mgr.load(&p).await.unwrap_or_else(|_| Config::default_with_profile(&p)) 
        };
        
        // Hydrate config from vault
        let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
        let _ = auth_cli.provider(&p_cfg.app_mode).hydrate_config(&p, &mut p_cfg, vault.clone()).await;

        if let Err(e) = daemon_svc.start_daemon(&p, &p_cfg, vault.clone()).await {
            error!(target: "sys", profile = %p, error = %e, "Failed to start worker");
        }
    }

    // Keep master alive
    info!(target: "sys", "Master daemon is running. Press Ctrl+C or send SIGTERM to stop.");
    
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = sigterm.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await?;
    }

    info!(target: "sys", "Master daemon shutting down...");
    
    // Stop all workers gracefully
    let _ = daemon_svc.stop_all().await;
    
    let _ = fs::remove_file(pid_file);
    
    Ok(())
    }
}

pub async fn stop(_profile: &str, _all: bool, _cfg_mgr: &ConfigManager) -> Result<()> {
    let app_dir = cowen_common::config::get_app_dir();
    let pid_file = app_dir.join("master_daemon.pid");
    
    if let Ok(content) = fs::read_to_string(&pid_file) {
        if let Some(pid_str) = content.lines().next() {
            if let Ok(pid_u32) = pid_str.trim().parse::<u32>() {
                eprintln!("🛑 Stopping master daemon (PID: {})...", pid_u32);
                kill_process(pid_u32);

                // Wait for process to exit
                use sysinfo::{System, Pid};
                let mut sys = System::new();
                let pid = Pid::from_u32(pid_u32);
                for _ in 0..20 { // Max 4 seconds
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
                    if sys.process(pid).is_none() {
                        break;
                    }
                }
            }
        }
    }
    let _ = fs::remove_file(pid_file);
    Ok(())
}

pub async fn restart(profile: &str, config: &Config, proxy_port: u16, enable_proxy: bool, all: bool, cfg_mgr: &ConfigManager, vault: Arc<dyn Vault>, telemetry: Option<Arc<TelemetryControl>>, daemon_svc: Arc<dyn DaemonService>) -> Result<()> {
    stop(profile, all, cfg_mgr).await?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    start(profile, config, proxy_port, enable_proxy, false, all, cfg_mgr, vault, telemetry, daemon_svc).await
}

fn kill_process(pid: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill").arg("-15").arg(pid.to_string()).status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill").arg("/PID").arg(pid.to_string()).status();
    }
}
