mod renewer;
mod bridge;

use crate::core::config::{Config, ConfigManager};
use sysinfo::System;
use anyhow::Result;
use std::process::{Command, Stdio};
use std::env;
use std::fs;
use std::sync::Arc;
use crate::core::vault::Vault;
use crate::auth::models::AuthMode;

/// 启动守护进程 (主分发器)
pub async fn start(profile: &str, config: &Config, _proxy_port: u16, _enable_proxy: bool, foreground: bool, all: bool, cfg_mgr: &ConfigManager, vault: Arc<dyn Vault>) -> Result<()> {
    let target_profiles = if all && !foreground {
        cfg_mgr.list_profiles()?
    } else {
        vec![profile.to_string()]
    };

    for p in target_profiles {
        let mut p_cfg = if p == profile { config.clone() } else { cfg_mgr.load(&p).unwrap_or_else(|_| Config::default_with_profile(&p)) };
        
        // 注入 Vault 中的敏感信息
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

        if let Err(e) = do_start(&p, &p_cfg, p_cfg.proxy_port, p_cfg.proxy_enabled, foreground, vault.clone()).await {
            eprintln!("⚠️ Failed to start daemon for profile '{}': {}", p, e);
        }
    }
    Ok(())
}

/// 执行启动的核心逻辑，处理父子进程逻辑
async fn do_start(profile: &str, config: &Config, proxy_port: u16, enable_proxy: bool, foreground: bool, vault: Arc<dyn Vault>) -> Result<()> {
    let app_dir = crate::core::config::get_app_dir();
    let pid_file = app_dir.join(format!("{}_daemon.pid", profile));

    // 基础配置校验
    if config.app_key.trim().is_empty() {
        anyhow::bail!("AppKey is empty for profile '{}'. Please run 'cowen init' first.", profile);
    }
    
    if !foreground {
        // --- 父进程：拉起并监控子进程 ---
        let exe = std::fs::canonicalize(env::current_exe()?)?;
        let mut child_cmd = Command::new(&exe);
        child_cmd.arg("--profile").arg(profile).arg("daemon").arg("start")
            .arg("--proxy-port").arg(proxy_port.to_string())
            .arg("--foreground") // 子进程运行在“前台”模式，但实际 stdio 被重定向
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());

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
    fs::write(&pid_file, format!("{}\n{}", pid, build_id))?;
    
    tracing::info!(target: "sys", "Daemon core logic starting (PID: {}, Mode: {:?})", pid, config.app_mode);

    let result = match config.app_mode {
        AuthMode::Oauth2 => {
            // 执行 OAuth2 专用续约引擎
            tokio::select! {
                res = renewer::run(profile, config, vault) => res,
                _ = wait_for_termination() => { tracing::info!(target: "sys", "Termination signal received"); Ok(()) },
            }
        },
        AuthMode::SelfBuilt => {
            // 执行自建应用专用流桥引擎
            tokio::select! {
                res = bridge::run(profile, config, vault, proxy_port, enable_proxy) => res,
                _ = wait_for_termination() => { tracing::info!(target: "sys", "Termination signal received"); Ok(()) },
            }
        }
    };

    tracing::info!(target: "sys", "Daemon process shutting down...");
    let _ = fs::remove_file(&pid_file);
    result
}

pub async fn stop(profile: &str, all: bool, cfg_mgr: &ConfigManager) -> Result<()> {
    let target_profiles = if all { cfg_mgr.list_profiles()? } else { vec![profile.to_string()] };
    for p in target_profiles {
        let _ = do_stop(&p).await;
    }
    Ok(())
}

async fn do_stop(profile: &str) -> Result<()> {
    let app_dir = crate::core::config::get_app_dir();
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
    let target_profiles = if all { cfg_mgr.list_profiles()? } else { vec![profile.to_string()] };
    for p in target_profiles {
        let _ = do_stop(&p).await;
        let mut p_cfg = if p == profile { config.clone() } else { cfg_mgr.load(&p).unwrap_or_else(|_| Config::default_with_profile(&p)) };
        if p != profile {
            if let Ok(as_val) = vault.get(&p, "app_secret") { p_cfg.app_secret = as_val; }
            if let Ok(cert) = vault.get(&p, "certificate") { p_cfg.certificate = cert; }
            if let Ok(ek) = vault.get(&p, "encrypt_key") { p_cfg.encrypt_key = ek; }
        }
        let _ = do_start(&p, &p_cfg, p_cfg.proxy_port, p_cfg.proxy_enabled, false, vault.clone()).await;
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
