use cowen_common::config::Config;
use cowen_config::ConfigManager;
pub use cowen_server::cmd::service;
use anyhow::Result;
use std::process::Command;

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

async fn sync_feedback(original_port: u16) -> Result<()> {
    let mut loop_count = 0;
    let mut recovered_port = None;

    let mut last_error = None;

    while loop_count < 10 { // 500ms total
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        // The profile param doesn't matter for unified daemon, we pass empty string
        if let Some(info) = cowen_common::status::get_active_daemon_info("") {
            if let Some(p) = info.monitor_port {
                if p != original_port && p != 0 {
                    recovered_port = Some(p);
                }
            }
            break;
        } else {
            // Check if crashed
            let app_dir = cowen_common::config::get_app_dir();
            let pid_file = app_dir.join("master_daemon.pid");
            if pid_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&pid_file) {
                    for line in content.lines() {
                        if let Some(le) = line.strip_prefix("LAST_ERROR=") {
                            let err_msg = le.trim();
                            if !err_msg.is_empty() {
                                last_error = Some(err_msg.to_string());
                            }
                        }
                    }
                }
            }
        }
        loop_count += 1;
    }

    if cowen_common::status::get_active_daemon_info("").is_some() {
        // Connected!
    } else if let Some(err_msg) = last_error {
        eprintln!("❌ Failed: Daemon crashed on startup (Error: {})", err_msg);
        return Err(anyhow::anyhow!("Daemon crashed on startup (Error: {})", err_msg));
    }

    if let Some(p) = recovered_port {
        eprintln!("🚀 Daemon recovered on port {} (Note: default {} was occupied)", p, original_port);
    }
    Ok(())
}

async fn preflight_check_and_bind_port(cfg_mgr: &ConfigManager) -> Result<()> {
    let app_cfg = cfg_mgr.load_app_config().await.unwrap_or_default();
    let m_port = if app_cfg.monitor_port == 0 { 1588 } else { app_cfg.monitor_port };

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], m_port));
    match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => {
            drop(l);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            // 🚀 STABILITY: Immediate micro-retries for TIME_WAIT or ephemeral OS lock release
            let mut resolved = false;
            for _ in 0..3 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if tokio::net::TcpListener::bind(addr).await.is_ok() {
                    resolved = true;
                    break;
                }
            }
            if resolved {
                return Ok(());
            }

            let app_dir = cowen_common::config::get_app_dir();
            let pid_file = app_dir.join("master_daemon.pid");
            let mut killed_old = false;

            if pid_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&pid_file) {
                    if let Some(pid_str) = content.lines().next() {
                        if let Ok(pid) = pid_str.trim().parse::<u32>() {
                            use sysinfo::{System, Pid, ProcessesToUpdate};
                            let mut s = System::new();
                            let sys_pid = Pid::from_u32(pid);
                            s.refresh_processes(ProcessesToUpdate::Some(&[sys_pid]), true);
                            if let Some(proc) = s.process(sys_pid) {
                                let name = proc.name().to_string_lossy().to_lowercase();
                                if name.contains("cowen") {
                                    // It's a cowen process. Is it healthy?
                                    let port_path = cowen_common::ipc::get_ipc_port_path();
                                    let is_healthy = if port_path.exists() {
                                        let client = cowen_common::ipc::client::IpcDaemonService::new(port_path.clone());
                                        client.ping().await.is_ok()
                                    } else {
                                        false
                                    };

                                    if is_healthy {
                                        tracing::debug!(target: "sys", "Port {} is occupied by healthy cowen daemon (PID: {}). Proceeding via IPC.", m_port, pid);
                                        return Ok(());
                                    }

                                    tracing::warn!(target: "sys", "Port {} seems occupied by unresponsive cowen daemon (PID: {}). Sending SIGTERM...", m_port, pid);
                                    #[cfg(unix)]
                                    let _ = std::process::Command::new("kill").arg("-15").arg(pid.to_string()).status();
                                    #[cfg(windows)]
                                    let _ = std::process::Command::new("taskkill").args(&["/F", "/PID", &pid.to_string()]).status();
                                    
                                    killed_old = true;
                                }
                            }
                        }
                    }
                }
            }

            // Retry binding for up to 3 seconds to allow TIME_WAIT or tokio graceful shutdown to clear
            // (Tokio might take a couple of seconds to fully drop the TcpListener after SIGTERM)
            for _ in 0..15 {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                if tokio::net::TcpListener::bind(addr).await.is_ok() {
                    return Ok(());
                }
            }
            if !killed_old {
                if app_cfg.monitor_port == 0 {
                    tracing::warn!(target: "sys", "Pre-flight check: Default monitor port {} is occupied, but config is set to 0. Allowing fallback to random port.", m_port);
                    return Ok(());
                } else {
                    tracing::warn!(target: "sys", "Pre-flight check: Monitor port {} is occupied by a 3rd party process.", m_port);
                    return Err(anyhow::anyhow!("Monitor port {} is occupied by another process.\n👉 Fix: Run 'cowen config set monitor_port <NEW_PORT> --global'", m_port));
                }
            }
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!("Pre-flight port bind failed: {}", e)),
    }
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
    let app_dir = cowen_common::config::get_app_dir();
    let stopped_file = app_dir.join("master_daemon.stopped");
    if stopped_file.exists() {
        let _ = std::fs::remove_file(&stopped_file);
    }

    // 1. 启动前置预检 (Pre-flight Check)
    preflight_check_and_bind_port(cfg_mgr).await?;

    #[cfg(unix)]
    {
        // On Unix, we use the standalone cowen-daemon binary via IPC.
        if !foreground {
            eprintln!("🚀 Triggering standalone daemon for profile '{}'...", profile);
            
            // Check if daemon is running by attempting a ping or checking socket
            let port_path = cowen_common::ipc::get_ipc_port_path();
            let _ipc_client = cowen_common::ipc::client::IpcDaemonService::new(port_path.clone());
            
            // Wait, we can't easily ping, so we'll try to spawn if not exists.
            // Actually, if we spawn it detached, we can then send the command.
            if !port_path.exists() {
                eprintln!("ℹ️ Daemon process not running. Spawning in background...");
                let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
                let daemon_path = std::env::var("COWEN_DAEMON_BIN").map(std::path::PathBuf::from).unwrap_or_else(|_| exe_dir.join("cowen-daemon"));
                
                let app_dir = cowen_common::config::get_app_dir();
                let log_dir = app_dir.join("logs");
                if !log_dir.exists() { let _ = std::fs::create_dir_all(&log_dir); }
                use std::os::unix::fs::OpenOptionsExt;
                let stdout_file = std::fs::OpenOptions::new().create(true).append(true).mode(0o600).open(log_dir.join("daemon.stdout.log"))?;
                let stderr_file = std::fs::OpenOptions::new().create(true).append(true).mode(0o600).open(log_dir.join("daemon.stderr.log"))?;

                let _child = Command::new(&daemon_path)
                    .arg("--ipc-port-file")
                    .arg(&port_path)
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
                if !port_path.exists() || !is_daemon_alive() {
                     eprintln!("ℹ️ Daemon socket stale or process dead. Spawning in background...");
                     if port_path.exists() { let _ = std::fs::remove_file(&port_path); }
                     
                     let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
                     let daemon_path = std::env::var("COWEN_DAEMON_BIN").map(std::path::PathBuf::from).unwrap_or_else(|_| exe_dir.join("cowen-daemon"));
                     
                     let app_dir = cowen_common::config::get_app_dir();
                     let log_dir = app_dir.join("logs");
                     if !log_dir.exists() { let _ = std::fs::create_dir_all(&log_dir); }
                     use std::os::unix::fs::OpenOptionsExt;
                     let stdout_file = std::fs::OpenOptions::new().create(true).append(true).mode(0o600).open(log_dir.join("daemon.stdout.log"))?;
                     let stderr_file = std::fs::OpenOptions::new().create(true).append(true).mode(0o600).open(log_dir.join("daemon.stderr.log"))?;

                     let _child = Command::new(&daemon_path)
                         .arg("--ipc-port-file")
                         .arg(&port_path)
                         .stdin(std::process::Stdio::null())
                         .stdout(std::process::Stdio::from(stdout_file))
                         .stderr(std::process::Stdio::from(stderr_file))
                         .spawn()?;
                     
                     tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                     
                     // Try again
                     let ipc_client2 = cowen_common::ipc::client::IpcDaemonService::new(port_path.clone());
                     if let Err(e2) = ipc_client2.start_daemon(profile, config, vault.clone()).await {
                         let app_cfg = cfg_mgr.load_app_config().await.unwrap_or_default();
                         let original_port = if app_cfg.monitor_port == 0 { 1588 } else { app_cfg.monitor_port };
                         if let Err(sync_err) = sync_feedback(original_port).await {
                             return Err(sync_err);
                         }
                         return Err(anyhow::anyhow!("IPC connection failed: FATAL: Failed to connect to cowen-daemon after spawning: {}", e2));
                     }
                } else {
                     return Err(anyhow::anyhow!("Daemon is running but failed to respond: {}", e));
                }
            }
            eprintln!("✅ Startup command sent to daemon.");
            let app_cfg = cfg_mgr.load_app_config().await.unwrap_or_default();
            let original_port = if app_cfg.monitor_port == 0 { 1588 } else { app_cfg.monitor_port };
            sync_feedback(original_port).await?;
            return Ok(());
        } else {
            // Foreground mode on Unix: we must spawn cowen-daemon as a child and wait for it,
            // so that launchd/systemd can monitor the process, while still keeping UDS IPC alive.
            let port_path = cowen_common::ipc::get_ipc_port_path();
            
            // Spawn the daemon in the foreground
            let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
            let daemon_path = std::env::var("COWEN_DAEMON_BIN").map(std::path::PathBuf::from).unwrap_or_else(|_| exe_dir.join("cowen-daemon"));
            
            let mut child = Command::new(&daemon_path)
                .arg("--ipc-port-file")
                .arg(&port_path)
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
            let ipc_client = cowen_common::ipc::client::IpcDaemonService::new(port_path.clone());
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
            let app_cfg = cfg_mgr.load_app_config().await.unwrap_or_default();
            let original_port = if app_cfg.monitor_port == 0 { 1588 } else { app_cfg.monitor_port };
            sync_feedback(original_port).await?;
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
        cowen_common::utils::secure_write(
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
        cowen_common::utils::secure_write(
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

pub async fn stop(profile: &str, all: bool, _cfg_mgr: &ConfigManager) -> Result<()> {
    #[cfg(unix)]
    {
        let port_path = cowen_common::ipc::get_ipc_port_path();
        if !port_path.exists() {
            eprintln!("✅ No running daemon found.");
            return Ok(());
        }

        let mut client = match cowen_common::ipc::client::ensure_daemon(&port_path).await {
            Ok(c) => c,
            Err(_) => {
                eprintln!("✅ Daemon is not running (or socket is stale).");
                let _ = std::fs::remove_file(&port_path);
                return Ok(());
            }
        };
        
        let token_path = port_path.with_file_name("ipc.token");
        let token = std::fs::read_to_string(&token_path).unwrap_or_default();

        if all {
            let req = cowen_common::ipc::DaemonRequest::StopAllWorkers;
            match cowen_common::ipc::client::send_request(&mut client, &req, &token).await {
                Ok(cowen_common::ipc::DaemonResponse::Success { message }) => eprintln!("✅ {}", message),
                Ok(cowen_common::ipc::DaemonResponse::Error { message, .. }) => eprintln!("⚠️ Failed to stop all workers: {}", message),
                Ok(_) => eprintln!("⚠️ Unexpected response type"),
                Err(e) => eprintln!("⚠️ IPC request failed: {}", e),
            }
        } else {
            let req = cowen_common::ipc::DaemonRequest::StopWorker { profile: profile.to_string() };
            match cowen_common::ipc::client::send_request(&mut client, &req, &token).await {
                Ok(cowen_common::ipc::DaemonResponse::Success { message }) => eprintln!("✅ {}", message),
                Ok(cowen_common::ipc::DaemonResponse::Error { message, .. }) => eprintln!("⚠️ Failed to stop profile {}: {}", profile, message),
                Ok(_) => eprintln!("⚠️ Unexpected response type"),
                Err(e) => eprintln!("⚠️ IPC request failed: {}", e),
            }
        }
    }

    #[cfg(not(unix))]
    {
        let app_dir = cowen_common::config::get_app_dir();
        let pid_file = app_dir.join("master_daemon.pid");
        let stopped_file = app_dir.join("master_daemon.stopped");
        cowen_common::utils::secure_write(&stopped_file, "1").ok(); // Set intentional stop marker
        
        if all {
            let mut process_dead = true;
            if let Ok(content) = fs::read_to_string(&pid_file) {
                if let Some(pid_str) = content.lines().next() {
                    if let Ok(pid_u32) = pid_str.trim().parse::<u32>() {
                        eprintln!("🛑 Stopping master daemon (PID: {})...", pid_u32);
                        kill_process(pid_u32);

                        process_dead = false;
                        use sysinfo::{System, Pid, ProcessesToUpdate};
                        let mut sys = System::new();
                        let pid = Pid::from_u32(pid_u32);
                        for _ in 0..60 {
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                            sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
                            if sys.process(pid).is_none() {
                                process_dead = true;
                                break;
                            }
                        }
                    }
                }
            }
            if process_dead {
                let _ = fs::remove_file(pid_file);
                eprintln!("✅ Daemon stopped successfully.");
            } else {
                tracing::warn!(target: "sys", "Daemon process did not exit within timeout.");
            }
        } else {
            eprintln!("⚠️ Stopping individual profiles on this OS is not supported yet. Use --all to stop the daemon.");
        }
    }
    Ok(())
}

pub async fn restart(profile: &str, config: &Config, proxy_port: u16, enable_proxy: bool, all: bool, cfg_mgr: &ConfigManager, vault: Arc<dyn Vault>, telemetry: Option<Arc<TelemetryControl>>, daemon_svc: Arc<dyn DaemonService>) -> Result<()> {
    stop(profile, all, cfg_mgr).await?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    start(profile, config, proxy_port, enable_proxy, false, all, cfg_mgr, vault, telemetry, daemon_svc).await
}

