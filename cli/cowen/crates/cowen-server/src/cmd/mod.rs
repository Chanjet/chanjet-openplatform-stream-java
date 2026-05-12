mod bridge;
pub mod service;

use cowen_common::config::Config;
use cowen_common::ConfigManager;
use sysinfo::System;
use anyhow::Result;
use std::process::{Command, Stdio};
use std::env;
use std::fs;
use std::sync::Arc;
use cowen_common::vault::Vault;

/// 启动守护进程 (主分发器)
pub async fn start(profile: &str, config: &Config, _proxy_port: u16, _enable_proxy: bool, foreground: bool, all: bool, cfg_mgr: &ConfigManager, vault: Arc<dyn Vault>) -> Result<()> {
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
        
        let pid_file = cowen_common::config::get_app_dir().join(format!("{}_daemon.pid", p));
        if all && pid_file.exists() {
            println!("ℹ️ Daemon for profile '{}' is already running. Skipping.", p);
            continue;
        }

        if let Err(e) = do_start(&p, &p_cfg, p_cfg.proxy_port, p_cfg.proxy_enabled, foreground, cfg_mgr, vault.clone()).await {
            eprintln!("⚠️ Failed to start daemon for profile '{}': {}", p, e);
        }
    }
    Ok(())
}

/// 执行启动的核心逻辑，处理父子进程逻辑
/// 执行启动的核心逻辑，处理父子进程逻辑
async fn do_start(profile: &str, config: &Config, proxy_port: u16, enable_proxy: bool, foreground: bool, cfg_mgr: &ConfigManager, vault: Arc<dyn Vault>) -> Result<()> {
    let app_dir = cowen_common::config::get_app_dir();
    let pid_file = app_dir.join(format!("{}_daemon.pid", profile));

    if config.app_key.trim().is_empty() {
        anyhow::bail!("AppKey is empty for profile '{}'. Please run 'cowen init' first or provide COWEN_APP_KEY/COWEN_APP_MODE.", profile);
    }

    // --- CHECK FOR ALREADY RUNNING DAEMON ---
    if pid_file.exists() {
        if let Ok(content) = fs::read_to_string(&pid_file) {
            if let Some(saved_pid_str) = content.lines().next() {
                if let Ok(saved_pid) = saved_pid_str.trim().parse::<u32>() {
                    let mut s = System::new_all();
                    s.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                    if let Some(process) = s.process(sysinfo::Pid::from_u32(saved_pid)) {
                         let cmdline = process.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" ");
                         let bin_name = cowen_common::utils::get_bin_name().to_lowercase();
                         if cmdline.to_lowercase().contains(&bin_name) {
                             if foreground {
                                 eprintln!("ℹ️ Daemon for profile '{}' is already running (PID: {}). Continuing anyway (foreground mode).", profile, saved_pid);
                             } else {
                                 eprintln!("ℹ️ Daemon for profile '{}' is already running (PID: {}). Skipping.", profile, saved_pid);
                                 return Ok(());
                             }
                         }
                    }
                }
            }
        }
    }
    
    if !foreground {

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
        let check_interval = std::time::Duration::from_millis(50);
        let mut s = System::new_all();

        for _ in 0..20 { // 1秒超时
            std::thread::sleep(check_interval);
            if pid_file.exists() {
                if let Ok(content) = fs::read_to_string(&pid_file) {
                    if let Some(saved_pid_str) = content.lines().next() {
                        if let Ok(saved_pid) = saved_pid_str.trim().parse::<u32>() {
                            s.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                            if s.process(sysinfo::Pid::from_u32(saved_pid)).is_some() {
                                ready = true;
                                break;
                            }
                        }
                    }
                }
            }
            s.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            if s.process(sysinfo::Pid::from_u32(pid)).is_none() {
                anyhow::bail!("Daemon process exited immediately. Check logs.");
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
    std::thread::sleep(std::time::Duration::from_millis(50));
    let pid = std::process::id();
    let build_id = env!("BUILD_ID");
    let build_time = env!("BUILD_TIME");
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
        
        let mut event_rx = cowen_common::events::event_bus().subscribe();
        let mut reload = false;

        let t_profile = profile.to_string();
        let t_cfg_mgr = cfg_mgr.clone();
        let t_current_version = current_config.version;
        let watcher = async move {
            let app_cfg: cowen_common::config::AppConfig = t_cfg_mgr.load_app_config().await.unwrap_or_default();
            if !t_cfg_mgr.is_distributed_storage(&app_cfg) {
                std::future::pending::<()>().await;
            }
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                if let Ok(true) = t_cfg_mgr.check_for_updates(&t_profile, t_current_version).await {
                    tracing::info!(target: "sys", "External config change detected via polling for profile '{}'. Triggering local reload...", t_profile);
                    cowen_common::events::event_bus().publish(cowen_common::events::GlobalEvent::ConfigChanged { 
                        profile: t_profile.clone(), 
                        key: "manifest".to_string() 
                    });
                    break;
                }
            }
        };

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
            _res = watcher => {
                tracing::info!(target: "sys", "Watcher detected config change. Reloading...");
                reload = true;
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
                    for _ in 0..10 {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        s.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                        if s.process(sysinfo::Pid::from_u32(pid)).is_none() { break; }
                    }
                }
            }
        }
        let _ = fs::remove_file(&pid_file);
        eprintln!("✅ Daemon stopped for profile '{}'.", profile);
    }
    Ok(())
}

pub async fn restart(profile: &str, config: &Config, _proxy_port: u16, _enable_proxy: bool, all: bool, cfg_mgr: &ConfigManager, vault: Arc<dyn Vault>) -> Result<()> {
    let target_profiles = if all { cfg_mgr.list_profiles().await? } else { vec![profile.to_string()] };
    for p in target_profiles {
        let _ = do_stop(&p).await;
        let mut p_cfg = if p == profile { config.clone() } else { cfg_mgr.load(&p).await.unwrap_or_else(|_| Config::default_with_profile(&p)) };
        if p != profile {
            if let Ok(as_val) = vault.get_secret(&p, "app_secret").await { p_cfg.app_secret = as_val; }
            if let Ok(cert) = vault.get_secret(&p, "certificate").await { p_cfg.certificate = cert; }
            if let Ok(ek) = vault.get_secret(&p, "encrypt_key").await { p_cfg.encrypt_key = ek; }
        }
        let _ = do_start(&p, &p_cfg, p_cfg.proxy_port, p_cfg.proxy_enabled, false, cfg_mgr, vault.clone()).await;
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
