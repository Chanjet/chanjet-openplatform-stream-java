use cowen_common::vault::Vault;
use cowen_common::{CowenError, CowenResult};
use cowen_config::ConfigManager;
use cowen_common::status::{MonitorClient, AuthStatus};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{BufRead, Seek, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

pub async fn wait_for_token_exchange_ipc(profile: &str, monitor_port: u16) -> CowenResult<()> {
    let client = MonitorClient::new(monitor_port);
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos:>7}% {msg}")
            .map_err(|e| CowenError::api(e.to_string()))?
            .progress_chars("#>-"),
    );

    pb.set_message("Waiting for browser authorization...");
    pb.set_position(5);

    let start_time = Instant::now();
    let timeout = Duration::from_secs(300);

    loop {
        if start_time.elapsed() > timeout {
            pb.finish_with_message("❌ Authorization timed out");
            return Err(CowenError::Auth("Authorization timed out".to_string()));
        }

        match client.get_auth_progress(profile).await {
            Ok(info) => {
                pb.set_position(info.percent as u64);
                pb.set_message(info.message.clone());

                match info.status {
                    AuthStatus::Completed => {
                        pb.finish_with_message("✅ Authorization successful");
                        return Ok(());
                    }
                    AuthStatus::Failed => {
                        let err = info.error.unwrap_or_else(|| "Unknown error".to_string());
                        pb.finish_with_message(format!("❌ Failed: {}", err));
                        return Err(CowenError::Auth(err));
                    }
                    _ => {}
                }
            }
            Err(_) => {
                // If monitor doesn't have progress yet, it might be waiting for the callback
                // Just keep waiting
            }
        }

        sleep(Duration::from_millis(500)).await;
    }
}

pub async fn wait_for_token_exchange(
    profile: &str,
    vault: Arc<dyn Vault>,
    finalizer_pid: u32,
    is_new: bool,
    _cfg_mgr: &ConfigManager,
    session_id: &str,
) -> CowenResult<()> {
    // RAII Guard: Ensures cleanup on return OR cancellation (Drop)
    struct CleanupGuard {
        profile: String,
        vault: Arc<dyn Vault>,
        pid: u32,
        is_new: bool,
        active: bool,
    }
    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            if self.active {
                let profile = self.profile.clone();
                let vault = self.vault.clone();
                let pid = self.pid;
                let is_new = self.is_new;

                tokio::spawn(async move {
                    if let Ok(cfg_mgr) = cowen_config::ConfigManager::new() {
                        perform_failure_cleanup(&profile, vault, pid, is_new, &cfg_mgr).await;
                    }
                });
            }
        }
    }

    let mut guard = CleanupGuard {
        profile: profile.to_string(),
        vault: vault.clone(),
        pid: finalizer_pid,
        is_new,
        active: true,
    };

    let start_time = Instant::now();
    let timeout = Duration::from_secs(300); // 5 minutes
    let log_file = cowen_common::config::get_app_dir()
        .join("logs")
        .join(format!("{}_auth.log", profile));
    let mut last_log_size = if log_file.exists() {
        std::fs::metadata(&log_file)
            .map_err(CowenError::from)?
            .len()
    } else {
        0
    };

    print!("⏳ 正在等待浏览器授权并在后台交换令牌...");
    std::io::stdout().flush().map_err(CowenError::from)?;

    loop {
        let elapsed = start_time.elapsed();
        if elapsed > timeout {
            println!("\n❌ 授权超时 (5 分钟)。请检查网络或重新运行 `init`。");
            render_last_auth_error(profile)?;
            // Guard will handle cleanup
            return Err(CowenError::Auth("Authorization timeout".to_string()));
        }

        let remaining = (timeout.as_secs() as i64 - elapsed.as_secs() as i64).max(0);
        print!(
            "\r⏳ 正在等待浏览器授权并在后台交换令牌... [剩余 {:3}s] ",
            remaining
        );
        std::io::stdout().flush().map_err(CowenError::from)?;

        // 1. Success check: Access token exists in domain storage
        if vault.get_access_token(profile).await.is_ok() {
            println!("\n✅ 授权成功！命令行已就绪。");
            guard.active = false; // Disarm guard on success
            return Ok(());
        }

        // 2. Failure check: Log file growth + ERROR check
        if log_file.exists() {
            let metadata = std::fs::metadata(&log_file).map_err(CowenError::from)?;
            if metadata.len() > last_log_size {
                // Read new content
                let file = std::fs::File::open(&log_file).map_err(CowenError::from)?;
                let mut reader = std::io::BufReader::new(file);
                reader
                    .seek(std::io::SeekFrom::Start(last_log_size))
                    .map_err(CowenError::from)?;

                for line in reader.lines() {
                    if let Ok(l) = line {
                        if l.contains("ERROR") {
                            println!("\n❌ 令牌交换失败！");
                            println!("\x1b[31m🔍 错误原因: {}\x1b[0m", l);
                            // Guard will handle cleanup
                            return Err(CowenError::Auth("Token exchange failed".to_string()));
                        }
                    }
                }
                last_log_size = metadata.len();
            }
        }

        // 3. State check: If session is GONE but no token was produced
        if vault.get_session(session_id).await.is_err() {
            // Give it a tiny bit of time to persist the token if it just happened
            sleep(Duration::from_millis(500)).await;
            if vault.get_access_token(profile).await.is_err() {
                println!(
                    "\n❌ 授权会话已失效且未获取到新令牌。授权过程可能已在其他地方中断或失败。"
                );
                render_last_auth_error(profile)?;
                // Guard will handle cleanup
                return Err(CowenError::Auth(
                    "Authorization state invalid (Session lost)".to_string(),
                ));
            }
        }

        sleep(Duration::from_millis(1000)).await;
    }
}

pub fn spawn_finalizer(profile: &str, session_id: &str) -> CowenResult<u32> {
    let exe = std::env::current_exe().map_err(CowenError::from)?;
    let mut cmd = std::process::Command::new(exe);

    // Setup log file
    let log_dir = cowen_common::config::get_app_dir().join("logs");
    std::fs::create_dir_all(&log_dir).map_err(CowenError::from)?;
    let log_file = log_dir.join(format!("{}_auth.log", profile));
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)
        .map_err(CowenError::from)?;

    cmd.args([
        "--profile",
        profile,
        "auth",
        "login",
        "--finalize",
        session_id,
    ])
    .stdin(std::process::Stdio::null())
    .stdout(file.try_clone().map_err(CowenError::from)?)
    .stderr(file);

    let child_id = cowen_sys::get_process_manager().spawn_daemon(&mut cmd)
        .map_err(|e| CowenError::Auth(e.to_string()))?;
    Ok(child_id)
}

pub fn render_last_auth_error(profile: &str) -> CowenResult<()> {
    let log_file = cowen_common::config::get_app_dir()
        .join("logs")
        .join(format!("{}_auth.log", profile));
    if !log_file.exists() {
        return Ok(());
    }

    let file = std::fs::File::open(log_file).map_err(CowenError::from)?;
    let reader = std::io::BufReader::new(file);
    let mut errors = Vec::new();

    for l in reader.lines().flatten() {
        if l.contains("ERROR") {
            errors.push(l);
        }
    }

    if !errors.is_empty() {
        println!("\n\x1b[31m🔍 诊断信息 (来自背景日志):\x1b[0m");
        // Show last 3 errors
        let start = if errors.len() > 3 {
            errors.len() - 3
        } else {
            0
        };
        for err in &errors[start..] {
            println!("  {}", err);
        }
    }

    Ok(())
}

pub async fn perform_failure_cleanup(
    profile: &str,
    vault: Arc<dyn Vault>,
    finalizer_pid: u32,
    is_new: bool,
    _cfg_mgr: &ConfigManager,
) {
    // 1. Kill the finalizer background process
    let mut sys = sysinfo::System::new();
    let sys_pid = sysinfo::Pid::from_u32(finalizer_pid);
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[sys_pid]), true);
    if let Some(process) = sys.process(sys_pid) {
        process.kill_with(sysinfo::Signal::Kill);
        tracing::info!(target: "sys", pid = %finalizer_pid, "Background finalizer killed due to initialization failure");
    }

    // 2. Cleanup session state
    let token_pool = crate::VaultTokenPool::new(vault.clone());
    let session_manager = crate::lifecycle::AuthSessionManager::new(&token_pool);
    let _ = session_manager.clear(profile).await;

    // 3. If this was a new profile, remove the junk .yaml file
    if is_new {
        println!("🧹 检测到这是新创建的 Profile，正在物理移除临时配置文件...");
        tracing::warn!("TODO: modularize system reset"); // (profile, Some(vault.as_ref()), cfg_mgr, None).await;
    }
}
