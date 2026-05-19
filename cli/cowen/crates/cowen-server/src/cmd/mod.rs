mod bridge;
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

use cowen_monitor::telemetry::TelemetryControl;

/// 启动守护进程 (主分发器)
pub async fn start(profile: &str, config: &Config, _proxy_port: u16, _enable_proxy: bool, foreground: bool, all: bool, cfg_mgr: &ConfigManager, vault: Arc<dyn Vault>, telemetry: Option<Arc<TelemetryControl>>) -> Result<()> {
    let target_profiles = if all && !foreground {
        cfg_mgr.list_profiles().await?
    } else {
        vec![profile.to_string()]
    };

    for p in target_profiles {
        let mut p_cfg = if p == profile { config.clone() } else { cfg_mgr.load(&p).await.unwrap_or_else(|_| Config::default_with_profile(&p)) };
        
        // 注入 Vault 中的敏感信息 (SPI 委派)
        if p != profile {
            let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
            let _ = auth_cli.provider(&p_cfg.app_mode).hydrate_config(&p, &mut p_cfg, vault.clone()).await;
        }
        
        let _pid_file = cowen_common::config::get_app_dir().join(format!("{}_daemon.pid", p));
        
        if let Err(e) = do_start(&p, &p_cfg, p_cfg.proxy_port, p_cfg.proxy_enabled, foreground, cfg_mgr, vault.clone(), telemetry.clone()).await {
            eprintln!("⚠️ Failed to start daemon for profile '{}': {}", p, e);
            return Err(e);
        }
    }
    Ok(())
}

fn find_ghost_process(profile: &str, proxy_port: u16) -> Option<u32> {
    let mut s = System::new_all();
    s.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let bin_name = cowen_common::utils::get_bin_name().to_lowercase();
    let port_str = proxy_port.to_string();
    
    for (pid, process) in s.processes() {
        let cmdline = process.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>();
        let cmd_str = cmdline.join(" ");
        
        // Identify our daemon: must have the binary name AND --profile <profile> AND daemon start
        let has_bin = process.name().to_string_lossy().to_lowercase().contains(&bin_name) || cmd_str.to_lowercase().contains(&bin_name);
        if !has_bin { continue; }

        let has_profile = cmdline.iter().any(|arg| arg == "--profile") && 
                          cmdline.windows(2).any(|w| w[0] == "--profile" && w[1] == profile);
        let is_daemon = cmdline.iter().any(|arg| arg == "daemon") && cmdline.iter().any(|arg| arg == "start");
        
        // Match proxy port to avoid killing independent nodes in a cluster
        let has_port = cmdline.iter().any(|arg| arg == "--proxy-port") &&
                       cmdline.windows(2).any(|w| w[0] == "--proxy-port" && w[1] == port_str);
        
        if has_profile && is_daemon && has_port && pid.as_u32() != std::process::id() {
            return Some(pid.as_u32());
        }
    }
    None
}

fn kill_process(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill").arg("/F").arg("/PID").arg(pid.to_string()).status();
    }
    
    // Verify death
    let mut s = System::new_all();
    for _ in 0..5 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        s.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        if s.process(sysinfo::Pid::from_u32(pid)).is_none() { return true; }
    }
    false
}

/// 执行启动的核心逻辑，处理父子进程逻辑
/// 执行启动的核心逻辑，处理父子进程逻辑
async fn do_start(profile: &str, config: &Config, proxy_port: u16, enable_proxy: bool, foreground: bool, cfg_mgr: &ConfigManager, vault: Arc<dyn Vault>, telemetry: Option<Arc<TelemetryControl>>) -> Result<()> {
    let app_dir = cowen_common::config::get_app_dir();
    let pid_file = app_dir.join(format!("{}_daemon.pid", profile));

    if config.app_key.trim().is_empty() {
        anyhow::bail!("AppKey is empty for profile '{}'. Please run 'cowen init' first or provide COWEN_APP_KEY/COWEN_APP_MODE.", profile);
    }

    // --- CHECK FOR ALREADY RUNNING DAEMON OR PORT CONFLICTS ---
    if !foreground {
        if let Some(ghost_pid) = find_ghost_process(profile, proxy_port) {
            if pid_file.exists() {
                 eprintln!("ℹ️ Daemon for profile '{}' is already running (PID: {}). Skipping.", profile, ghost_pid);
                 return Ok(());
            }
            
            // If we reach here, it's a ghost (running but no PID file)
            tracing::warn!(target: "sys", profile = %profile, ghost_pid = %ghost_pid, port = %proxy_port, "Detected ghost process without PID file. Evicting...");
            eprintln!("⚠️ 检测到 Profile '{}' 存在幽灵进程 (PID: {}, Port: {})，正在强制驱逐以释放资源...", profile, ghost_pid, proxy_port);
            let _ = kill_process(ghost_pid);
            std::thread::sleep(std::time::Duration::from_millis(500));
        } else if enable_proxy {
            let bin_name = cowen_infra::get_bin_name();
            if let Some((other_pid, other_name)) = cowen_infra::check_port_occupancy(proxy_port, &bin_name) {
                if other_name.to_lowercase().contains(&bin_name.to_lowercase()) {
                    let other_profile = cowen_infra::extract_profile_from_cmdline(other_pid).unwrap_or_else(|| "unknown".to_string());
                    if other_profile != profile {
                         anyhow::bail!("Proxy port {} is already occupied by another Cowen profile '{}' (PID: {}). Please use --proxy-port to specify a different port or disable proxy.", proxy_port, other_profile, other_pid);
                    }
                } else {
                    anyhow::bail!("Proxy port {} is already occupied by process '{}' (PID: {}). Please free the port or use --proxy-port to specify a different port.", proxy_port, other_name, other_pid);
                }
            }
        }

        // --- 父进程：拉起并监控子进程 ---
        let exe = std::fs::canonicalize(env::current_exe()?)?;
        let mut child_cmd = Command::new(&exe);
        child_cmd.arg("--profile").arg(profile).arg("daemon").arg("start")
            .arg("--proxy-port").arg(proxy_port.to_string())
            .arg("--foreground") // 子进程运行在“前台”模式，但实际 stdio 被重定向
            .stdin(Stdio::null());

        let log_file = std::fs::File::create(app_dir.join(format!("{}_child_crash.log", profile))).unwrap();
        let err_file = log_file.try_clone().unwrap();
        child_cmd.stdout(Stdio::from(log_file)).stderr(Stdio::from(err_file));

        if enable_proxy { child_cmd.arg("--enable-proxy"); }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            child_cmd.process_group(0);
        }

        let spawned = child_cmd.spawn()?;
        let pid = spawned.id();
        
        eprintln!("🚀 Launching background worker for profile '{}' (PID: {})...", profile, pid);
        
        // 同步等待子进程通过存活校验（Watchdog）
        let mut ready = false;
        let check_interval = std::time::Duration::from_millis(100);
        let mut s = System::new_all();

        // 🚀 STABILITY: Increase timeout to 10s (100 * 100ms) to allow child's internal port-retry to complete
        // and monitor server initialization under heavy parallel load.
        for _ in 0..100 { 
            std::thread::sleep(check_interval);
            if pid_file.exists() {
                if let Ok(content) = fs::read_to_string(&pid_file) {
                    if let Some(saved_pid_str) = content.lines().next() {
                        if let Ok(saved_pid) = saved_pid_str.trim().parse::<u32>() {
                            // If the saved PID matches the one we just spawned, we are good.
                            if saved_pid == pid {
                                ready = true;
                                break;
                            }
                        }
                    }
                }
            }
            s.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            if s.process(sysinfo::Pid::from_u32(pid)).is_none() {
                anyhow::bail!("Daemon process exited immediately. Check logs at {}_child_crash.log", profile);
            }
        }

        if ready {
            eprintln!("✅ Daemon successfully started and stabilized.");
            return Ok(());
        } else {
            anyhow::bail!("Daemon failed to stabilize within timeout.");
        }
    }

    // --- 子进程：执行核心引擎逻辑 ---
    // 🚀 NEW: Set process identity to allow easy identification in ps/top
    cowen_common::utils::set_process_name(&format!("cowen:{}", profile));

    std::thread::sleep(std::time::Duration::from_millis(50));
    let pid = std::process::id();
    let build_time = cowen_common::BUILD_TIME;
    let build_id = cowen_common::BUILD_ID;
    fs::write(&pid_file, format!("{}\n{}\n{}", pid, build_id, build_time))?;
    
    // BUG FIX: Hold an exclusive lock on the PID file to allow reliable liveness detection.
    // The lock will be released automatically by the OS if the process crashes or exits.
    let _pid_lock = if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&pid_file) {
        use fs2::FileExt;
        if f.try_lock_exclusive().is_ok() {
            Some(f)
        } else {
            tracing::warn!(target: "sys", "Could not acquire exclusive lock on PID file. Another instance might be running?");
            None
        }
    } else {
        None
    };
    
    let mut current_config = config.clone();
    #[allow(unused_assignments)]
    let mut result: Result<()> = Ok(());

    loop {
        tracing::info!(target: "sys", "Daemon core logic starting (PID: {}, Mode: {:?}, Version: {})", pid, current_config.app_mode, current_config.version);
        
        // 🚀 MONITORING: Initialize monitor server if port configured
        let m_port = current_config.monitor_port;
        if m_port > 0 {
            let (m_tx, m_rx) = tokio::sync::oneshot::channel();
            let m_server = cowen_monitor::MonitorServer::new(m_port);
            tokio::spawn(async move {
                if let Err(e) = m_server.start(Some(m_tx)).await {
                    tracing::error!(target: "sys", error = %e, "Monitor server failed");
                }
            });
            
            // Wait for monitor server to bind successfully
            if let Ok(actual_m_port) = tokio::time::timeout(std::time::Duration::from_secs(5), m_rx).await {
                tracing::info!(target: "sys", "Monitor server ready on port {}", actual_m_port.unwrap_or(m_port));
            } else {
                tracing::warn!(target: "sys", "Monitor server failed to start within timeout, continuing...");
            }
        }
        
        let mut event_rx = cowen_common::events::event_bus().subscribe();
        let mut config_rx = cfg_mgr.subscribe_profile_config(profile).await;
        let mut reload = false;

        let engine = async {
            // 🚀 OCP: Unified Engine for all modes. 
            // The bridge::run now uses generic AuthClient hooks and handles both 
            // streaming events and background maintenance.
            let app_cfg: cowen_common::config::AppConfig = cfg_mgr.load_app_config().await.unwrap_or_default();
            let is_dist = cfg_mgr.is_distributed_storage(&app_cfg);
            bridge::run(profile, &current_config, vault.clone(), proxy_port, enable_proxy, is_dist).await
        };

        tokio::select! {
            res = engine => {
                tracing::info!(target: "sys", "Engine terminated naturally.");
                result = res;
                break;
            },
            _ = wait_for_termination() => { 
                tracing::info!(target: "sys", "Termination signal received"); 
                result = Ok(());
                break; 
            },
            res = event_rx.recv() => {
                if let Ok(event) = res {
                    match event {
                        cowen_common::events::GlobalEvent::ConfigChanged { profile: p, .. } if p == profile || p == "system" => {
                            tracing::info!(target: "sys", "Config change detected via EventBus for profile '{}'. Hot-reloading daemon...", p);
                            reload = true;
                        }
                        _ => {}
                    }
                }
            },
            res = config_rx.changed() => {
                if res.is_ok() {
                    let new_cfg = config_rx.borrow().clone();
                    if new_cfg.log.level != current_config.log.level {
                        if let Some(t) = &telemetry {
                            tracing::info!(target: "sys", profile = %profile, old_level = %current_config.log.level, new_level = %new_cfg.log.level, "Dynamic log level update triggered via file watch");
                            let _ = t.update_level(&new_cfg.log.level);
                        }
                    }
                    if new_cfg.version > current_config.version {
                         tracing::info!(target: "sys", profile = %profile, "Major config version change detected via file watch. Triggering full reload...");
                         reload = true;
                    } else {
                        current_config = new_cfg;
                    }
                }
            }
        }

        if reload {
            tracing::info!(target: "sys", profile = %profile, "Hot-reloading daemon components...");
            // Grace period for previous tasks to clean up (port release, etc)
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            
            tracing::info!(target: "sys", profile = %profile, "Fetching latest configuration...");
            match cfg_mgr.load(profile).await {
                Ok(mut new_config) => {
                    tracing::info!(target: "sys", profile = %profile, version = %new_config.version, "Configuration reloaded successfully");
                    let app_key = new_config.app_key.trim();
                    let global_profile = format!("app:{}", app_key);

                    if let Ok(as_val) = vault.get_secret(profile, "app_secret").await { new_config.app_secret = as_val; }
                    else if let Ok(s) = vault.get_secret(&global_profile, "app_secret").await { new_config.app_secret = s; }

                    if let Ok(cert) = vault.get_secret(profile, "certificate").await { new_config.certificate = cert; }
                    else if let Ok(cert) = vault.get_secret(&global_profile, "certificate").await { new_config.certificate = cert; }

                    if let Ok(ek) = vault.get_secret(profile, "encrypt_key").await { new_config.encrypt_key = ek; }
                    else if let Ok(ek) = vault.get_secret(&global_profile, "encrypt_key").await { new_config.encrypt_key = ek; }

                    // BUG FIX: Re-apply environment variable overrides after reload.
                    // This ensures that settings like COWEN_PROXY_PORT (which are often pod-specific)
                    // are preserved and not overwritten by the shared manifest from the database.
                    new_config.apply_env_overrides();

                    current_config = new_config;
                }
                Err(e) => {
                    tracing::error!(target: "sys", profile = %profile, error = %e, "Failed to reload config. Retrying in 5s...");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    tracing::info!(target: "sys", "Daemon process shutting down...");
    if let Ok(content) = fs::read_to_string(&pid_file) {
        if let Some(saved_pid_str) = content.lines().next() {
            if let Ok(saved_pid) = saved_pid_str.trim().parse::<u32>() {
                if saved_pid == pid {
                    let _ = fs::remove_file(&pid_file);
                }
            }
        }
    }
    result
}

pub async fn stop(profile: &str, all: bool, cfg_mgr: &ConfigManager) -> Result<()> {
    let target_profiles = if all { cfg_mgr.list_profiles().await? } else { vec![profile.to_string()] };
    for p in target_profiles {
        let _ = do_stop(&p).await;
    }
    Ok(())
}

async fn do_stop(profile: &str) -> Result<()> {
    let app_dir = cowen_common::config::get_app_dir();
    let pid_file = app_dir.join(format!("{}_daemon.pid", profile));
    if pid_file.exists() {
        if let Ok(pid_content) = fs::read_to_string(&pid_file) {
            if let Some(pid_str) = pid_content.lines().next() {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    eprintln!("🛑 Stopping daemon (PID: {}) for profile '{}'...", pid, profile);
                    #[cfg(unix)]
                    let _ = Command::new("kill").arg("-15").arg(pid.to_string()).status();
                    #[cfg(windows)]
                    let _ = Command::new("taskkill").arg("/F").arg("/PID").arg(pid.to_string()).status();
                    
                    // 等待退出
                    let mut s = System::new_all();
                    for _ in 0..15 {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        s.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                        if s.process(sysinfo::Pid::from_u32(pid)).is_none() { break; }
                    }
                }
            }
        }
        // 🚀 HARDENING: Always attempt to remove the PID file to avoid blocking subsequent starts
        let _ = fs::remove_file(&pid_file);
        eprintln!("✅ Daemon stopped for profile '{}'.", profile);
    } else {
        // Even if no pid_file exists, try to clean up any potential ghosts using find_ghost_process logic if needed? 
        // For now, just ensure the file is gone.
        let _ = fs::remove_file(&pid_file);
    }
    Ok(())
}

pub async fn restart(profile: &str, config: &Config, _proxy_port: u16, _enable_proxy: bool, all: bool, cfg_mgr: &ConfigManager, vault: Arc<dyn Vault>, telemetry: Option<Arc<TelemetryControl>>) -> Result<()> {
    let target_profiles = if all { cfg_mgr.list_profiles().await? } else { vec![profile.to_string()] };
    for p in target_profiles {
        let _ = do_stop(&p).await;
        let mut p_cfg = if p == profile { config.clone() } else { cfg_mgr.load(&p).await.unwrap_or_else(|_| Config::default_with_profile(&p)) };
        if p != profile {
            if let Ok(as_val) = vault.get_secret(&p, "app_secret").await { p_cfg.app_secret = as_val; }
            if let Ok(cert) = vault.get_secret(&p, "certificate").await { p_cfg.certificate = cert; }
            if let Ok(ek) = vault.get_secret(&p, "encrypt_key").await { p_cfg.encrypt_key = ek; }
        }
        let _ = do_start(&p, &p_cfg, p_cfg.proxy_port, p_cfg.proxy_enabled, false, cfg_mgr, vault.clone(), telemetry.clone()).await;
    }
    Ok(())
}

async fn wait_for_termination() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to register SIGTERM");
        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to register SIGINT");
        tokio::select! {
            _ = sigterm.recv() => {},
            _ = sigint.recv() => {},
        };
    }
    #[cfg(not(unix))]
    { let _ = tokio::signal::ctrl_c().await; }
}
