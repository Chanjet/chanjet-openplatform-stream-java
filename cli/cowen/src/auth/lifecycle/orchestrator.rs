use crate::core::config::ConfigManager;
use crate::core::vault::Vault;
use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::io::{BufRead, Write, Seek};
use tokio::time::sleep;

pub async fn wait_for_token_exchange(
    profile: &str, 
    vault: Arc<dyn Vault>, 
    finalizer_pid: u32,
    is_new: bool,
    cfg_mgr: &ConfigManager,
) -> Result<()> {
    let start_time = Instant::now();
    let timeout = Duration::from_secs(300); // 5 minutes
    let log_file = crate::core::config::get_app_dir().join("logs").join(format!("{}_auth.log", profile));
    let mut last_log_size = if log_file.exists() {
        std::fs::metadata(&log_file)?.len()
    } else {
        0
    };
    
    print!("⏳ 正在等待浏览器授权并在后台交换令牌...");
    std::io::stdout().flush()?;

    loop {
        let elapsed = start_time.elapsed();
        if elapsed > timeout {
            println!("\n❌ 授权超时 (5 分钟)。请检查网络或重新运行 `init`。");
            render_last_auth_error(profile)?;
            perform_failure_cleanup(profile, vault, finalizer_pid, is_new, cfg_mgr).await;
            return Err(anyhow::anyhow!("Authorization timeout"));
        }

        let remaining = (timeout.as_secs() as i64 - elapsed.as_secs() as i64).max(0);
        print!("\r⏳ 正在等待浏览器授权并在后台交换令牌... [剩余 {:3}s] ", remaining);
        std::io::stdout().flush()?;

        // 1. Success check: Token pair exists
        if vault.get(profile, "oauth2_token_pair").await.is_ok() {
            println!("\n✅ 授权成功！命令行已就绪。");
            return Ok(());
        }

        // 2. Failure check: Log file growth + ERROR check
        if log_file.exists() {
            let metadata = std::fs::metadata(&log_file)?;
            if metadata.len() > last_log_size {
                // Read new content
                let file = std::fs::File::open(&log_file)?;
                let mut reader = std::io::BufReader::new(file);
                reader.seek(std::io::SeekFrom::Start(last_log_size))?;
                
                for line in reader.lines() {
                    if let Ok(l) = line {
                        if l.contains("ERROR") {
                            println!("\n❌ 令牌交换失败！");
                            println!("\x1b[31m🔍 错误原因: {}\x1b[0m", l);
                            perform_failure_cleanup(profile, vault, finalizer_pid, is_new, cfg_mgr).await;
                            return Err(anyhow::anyhow!("Token exchange failed"));
                        }
                    }
                }
                last_log_size = metadata.len();
            }
        }

        // 3. State check: If session is cleared but no token pair
        if vault.get(profile, "pending_auth_session").await.is_err() 
            && vault.get(profile, "captured_auth_code").await.is_err() 
            && vault.get(profile, "oauth2_token_pair").await.is_err() {
            // Wait a small buffer for the final write
            sleep(Duration::from_millis(500)).await;
            if vault.get(profile, "oauth2_token_pair").await.is_err() {
                println!("\n❌ 授权会话已失效且未获取到新令牌。授权过程可能已在其他地方中断或失败。");
                render_last_auth_error(profile)?;
                perform_failure_cleanup(profile, vault, finalizer_pid, is_new, cfg_mgr).await;
                return Err(anyhow::anyhow!("Authorization state invalid"));
            }
        }
        
        tokio::select! {
            _ = sleep(Duration::from_millis(1000)) => {
                // Continue loop
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\n🛑 收到中断信号 (Ctrl+C)。正在取消授权并关闭监听...");
                perform_failure_cleanup(profile, vault, finalizer_pid, is_new, cfg_mgr).await;
                return Err(anyhow::anyhow!("Operation cancelled by user"));
            }
        }
    }
}

pub fn spawn_finalizer(profile: &str, session_id: &str) -> Result<u32> {
    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    
    // Setup log file
    let log_dir = crate::core::config::get_app_dir().join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let log_file = log_dir.join(format!("{}_auth.log", profile));
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)?;

    cmd.args(&["--profile", profile, "auth", "login", "--finalize", session_id])
       .stdin(std::process::Stdio::null())
       .stdout(file.try_clone()?)
       .stderr(file);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let child = cmd.spawn()?;
    Ok(child.id())
}

pub fn render_last_auth_error(profile: &str) -> Result<()> {
    let log_file = crate::core::config::get_app_dir().join("logs").join(format!("{}_auth.log", profile));
    if !log_file.exists() {
        return Ok(());
    }

    let file = std::fs::File::open(log_file)?;
    let reader = std::io::BufReader::new(file);
    let mut errors = Vec::new();

    for line in reader.lines() {
        if let Ok(l) = line {
            if l.contains("ERROR") {
                errors.push(l);
            }
        }
    }

    if !errors.is_empty() {
        println!("\n\x1b[31m🔍 诊断信息 (来自背景日志):\x1b[0m");
        // Show last 3 errors
        let start = if errors.len() > 3 { errors.len() - 3 } else { 0 };
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
    cfg_mgr: &ConfigManager,
) {
    // 1. Kill the finalizer background process
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    if let Some(process) = sys.process(sysinfo::Pid::from_u32(finalizer_pid)) {
        process.kill_with(sysinfo::Signal::Kill);
        tracing::info!(target: "sys", pid = %finalizer_pid, "Background finalizer killed due to initialization failure");
    }

    // 2. Cleanup session state
    let token_pool = crate::auth::VaultTokenPool::new(vault.clone());
    let session_manager = crate::auth::lifecycle::AuthSessionManager::new(&token_pool);
    let _ = session_manager.clear(profile).await;

    // 3. If this was a new profile, remove the junk .yaml file
    if is_new {
        println!("🧹 检测到这是新创建的 Profile，正在物理移除临时配置文件...");
        let _ = crate::cmd::system::reset(profile, Some(vault.as_ref()), cfg_mgr).await;
    }
}
