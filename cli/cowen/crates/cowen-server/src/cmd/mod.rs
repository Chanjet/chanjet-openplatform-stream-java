pub mod bridge;
pub mod service;
pub mod renewer;

use cowen_common::config::Config;
use cowen_config::ConfigManager;
use sysinfo::System;
use anyhow::Result;
use std::process::{Command, Stdio};
use std::env;
use std::fs;
use std::sync::Arc;
use cowen_common::vault::Vault;
use cowen_common::daemon::DaemonService;
use crate::service_impl::ServerDaemonService;
use tracing::{info, error};

use cowen_monitor::telemetry::TelemetryControl;

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
    if !foreground {
        // Parent process logic: spawn itself with --foreground
        let app_dir = cowen_common::config::get_app_dir();
        let pid_file = app_dir.join("master_daemon.pid");

        // Check for existing master
        if pid_file.exists() {
             if let Ok(content) = fs::read_to_string(&pid_file) {
                 if let Some(pid_str) = content.lines().next() {
                     if let Ok(pid) = pid_str.trim().parse::<u32>() {
                          let mut s = System::new();
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

        let exe = std::fs::canonicalize(env::current_exe()?)?;
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
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file));
        
        if all { child_cmd.arg("--all"); }

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            child_cmd.process_group(0);
        }

        let spawned = child_cmd.spawn()?;
        let pid = spawned.id();
        eprintln!("🚀 Launching master daemon (PID: {})...", pid);
        
        // Wait for PID file
        let mut ready = false;
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if pid_file.exists() { ready = true; break; }
        }
        
        if ready {
            eprintln!("✅ Master daemon started successfully.");
            return Ok(());
        } else {
            anyhow::bail!("Master daemon failed to start within timeout.");
        }
    }

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

    fs::write(
        &pid_file, 
        format!(
            "{}\nBUILD_ID={}\nBUILD_TIME={}", 
            std::process::id(), 
            cowen_common::BUILD_ID, 
            cowen_common::BUILD_TIME
        )
    )?;

    // Set process name
    cowen_common::utils::set_process_name("cowen:master");

    // Start Monitor Server
    let app_cfg = cfg_mgr.load_app_config().await?;
    let m_port = app_cfg.monitor_port; // might be 0
    let m_server = cowen_monitor::MonitorServer::new(m_port, daemon_svc.clone());
    let (port_tx, port_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let _ = m_server.start(Some(port_tx)).await;
    });
    
    let actual_m_port = match tokio::time::timeout(tokio::time::Duration::from_secs(5), port_rx).await {
        Ok(Ok(p)) => p,
        _ => {
            error!(target: "sys", "Timed out or failed waiting for monitor server to start");
            0
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

        use cowen_common::daemon::DaemonService;
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

pub async fn stop(_profile: &str, _all: bool, _cfg_mgr: &ConfigManager) -> Result<()> {
    let app_dir = cowen_common::config::get_app_dir();
    let pid_file = app_dir.join("master_daemon.pid");
    
    if let Ok(content) = fs::read_to_string(&pid_file) {
        if let Ok(pid) = content.trim().parse::<u32>() {
            eprintln!("🛑 Stopping master daemon (PID: {})...", pid);
            kill_process(pid);
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
