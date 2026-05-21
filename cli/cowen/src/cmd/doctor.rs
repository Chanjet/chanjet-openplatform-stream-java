use anyhow::Result;
use cowen_common::config::Config;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;
use std::sync::Arc;
use colored::*;
use std::time::Instant;
use crate::core::DaemonManager;

pub async fn execute(profile: &str, config: &Config, verbose: bool, fix: bool, vault: Arc<dyn Vault>, cfg_mgr: &ConfigManager) -> Result<()> {
    println!("\n{} {} (Profile: {})", "🩺".bold(), "Cowen Doctor - 环境诊断工具".bold(), profile.cyan());
    println!("{}\n", "=".repeat(60).dimmed());

    let mut all_ok = true;

    // 1. 系统与配置检查
    if !check_system_info(config, verbose) { all_ok = false; }
    
    // 2. 存储后端与 Schema 检查
    if !check_storage(cfg_mgr, profile, verbose, fix).await { all_ok = false; }

    // 3. 守护进程与 IPC 检查
    if !check_daemon(profile, cfg_mgr, verbose).await { all_ok = false; }

    // 4. 网络连通性检查
    if !check_network(config, verbose).await { all_ok = false; }

    // 5. 凭据与认证检查
    if !check_credentials(profile, config, vault, verbose).await { all_ok = false; }

    println!("\n{}", "=".repeat(60).dimmed());
    if all_ok {
        println!("{} {}", "✅".bold(), "诊断完成，环境运行状况良好。".green().bold());
    } else {
        println!("{} {}", "⚠️".bold(), "诊断发现部分问题，请参考上方建议进行修复。".yellow().bold());
    }

    Ok(())
}

fn check_system_info(config: &Config, verbose: bool) -> bool {
    println!("{} {}", "[1/5]".dimmed(), "系统与配置检查...".bold());
    
    println!("  • Cowen Version: {}", env!("CARGO_PKG_VERSION").cyan());
    println!("  • OS:            {} {}", std::env::consts::OS, std::env::consts::ARCH);
    println!("  • App Mode:      {:?}", config.app_mode);
    
    if verbose {
        println!("  • App Key:       {}", cowen_common::utils::mask_string(&config.app_key));
        println!("  • Stream URL:    {}", config.stream_url);
    }

    let app_dir = cowen_common::config::get_app_dir();
    if app_dir.exists() {
        let permissions = std::fs::metadata(&app_dir).map(|m| format!("{:?}", m.permissions())).unwrap_or_default();
        println!("  • Home Dir:      {} {} {}", app_dir.display(), "OK".green(), permissions.dimmed());
    } else {
        println!("  • Home Dir:      {} {}", app_dir.display(), "MISSING".red());
        return false;
    }

    true
}

async fn check_storage(cfg_mgr: &ConfigManager, profile: &str, _verbose: bool, fix: bool) -> bool {
    println!("\n{} {}", "[2/5]".dimmed(), "存储后端与 Schema 检查...".bold());
    
    if fix {
        println!("  🛠️  正在检查并执行配置文件物理迁移...");
        let _ = cfg_mgr.auto_migrate().await;
    }

    let app_cfg = match cfg_mgr.load_app_config().await {
        Ok(c) => c,
        Err(e) => {
            println!("  • AppConfig:     {} ({})", "ERROR".red(), e);
            return false;
        }
    };

    println!("  • Storage Store: {}", app_cfg.storage.store.cyan());
    
    match crate::core::create_store(cfg_mgr).await {
        Ok(store) => {
            println!("  • Connectivity:  {}", "OK".green());
            
            match store.list_dlq_paged(profile, 0, 1).await {
                Ok(_) => {
                    println!("  • DLQ Schema:    {}", "OK (v0.3.2 ready)".green());
                }
                Err(e) => {
                    println!("  • DLQ Schema:    {} ({})", "OUTDATED or ERROR".red(), e);
                    if fix {
                        println!("    🛠️  正在尝试自动修复 Schema...");
                        match store.migrate().await {
                            Ok(_) => {
                                println!("    ✅ Schema 修复成功。");
                                return true;
                            }
                            Err(me) => {
                                println!("    ❌ Schema 修复失败: {}", me);
                            }
                        }
                    } else {
                        println!("    {} 存储 Schema 可能需要更新。建议运行 'cowen doctor --fix'。", "建议:".yellow());
                    }
                    return false;
                }
            }
        }
        Err(e) => {
            println!("  • Connectivity:  {} ({})", "FAILED".red(), e);
            return false;
        }
    }
    true
}

async fn check_daemon(profile: &str, cfg_mgr: &ConfigManager, _verbose: bool) -> bool {
    println!("\n{} {}", "[3/5]".dimmed(), "守护进程与 IPC 检查...".bold());
    
    let dm = DaemonManager::new(cfg_mgr.clone());
    let status = dm.get_status(profile).await;
    
    if let Some(info) = status {
        println!("  • Daemon Status: {} (PID: {})", "RUNNING".green(), info.pid);
        
        // 尝试 IPC 通信 (Monitor API)
        if let Some(port) = info.monitor_port {
            let client = reqwest::Client::new();
            let url = format!("http://127.0.0.1:{}/health", port);
            match client.get(&url).timeout(std::time::Duration::from_millis(500)).send().await {
                Ok(res) if res.status().is_success() => {
                    println!("  • IPC Health:    {} (Monitor API OK)", "OK".green());
                }
                Ok(res) => {
                    println!("  • IPC Health:    {} (Status: {})", "UNHEALTHY".yellow(), res.status());
                }
                Err(e) => {
                    println!("  • IPC Health:    {} ({})", "FAILED".red(), e);
                }
            }
        } else {
            println!("  • IPC Health:    {} (Monitor port unknown)", "UNKNOWN".yellow());
        }
    } else {
        println!("  • Daemon Status: {}", "NOT RUNNING".dimmed());
    }
    
    true
}

async fn check_network(config: &Config, _verbose: bool) -> bool {
    println!("\n{} {}", "[4/5]".dimmed(), "网络连通性检查...".bold());
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let targets = vec![
        ("OpenAPI", config.openapi_url.clone()),
        ("Stream Gateway", config.stream_url.clone()),
    ];

    let mut ok = true;
    for (name, url) in targets {
        let start = Instant::now();
        match client.get(&url).send().await {
            Ok(res) => {
                let duration = start.elapsed().as_millis();
                
                // 检查时钟偏移 (Clock Drift)
                let mut drift_msg = "".to_string();
                if let Some(date_str) = res.headers().get("Date").and_then(|v| v.to_str().ok()) {
                    if let Ok(server_date) = chrono::DateTime::parse_from_rfc2822(date_str) {
                        let local_now = chrono::Utc::now();
                        let drift = (local_now.timestamp() - server_date.timestamp()).abs();
                        if drift > 300 {
                            drift_msg = format!(" (Clock Drift: {}s!)", drift).red().to_string();
                            ok = false;
                        } else if drift > 60 {
                            drift_msg = format!(" (Clock Drift: {}s)", drift).yellow().to_string();
                        }
                    }
                }

                println!("  • {:<15} {:<40} {} ({}ms){}", name, url.dimmed(), "PASSED".green(), duration, drift_msg);
            }
            Err(e) => {
                println!("  • {:<15} {:<40} {} ({})", name, url.dimmed(), "FAILED".red(), e);
                ok = false;
            }
        }
    }
    ok
}

async fn check_credentials(profile: &str, _config: &Config, vault: Arc<dyn Vault>, _verbose: bool) -> bool {
    println!("\n{} {}", "[5/5]".dimmed(), "凭据与认证检查...".bold());
    
    let mut ok = true;
    
    match vault.get_secret(profile, "app_secret").await {
        Ok(s) if !s.is_empty() => {
            println!("  • App Secret:    {}", "FOUND".green());
        }
        _ => {
            println!("  • App Secret:    {}", "MISSING".red());
            println!("    {} 请运行 'cowen init'。", "建议:".yellow());
            ok = false;
        }
    }

    match vault.get_secret(profile, "access_token").await {
        Ok(s) if !s.is_empty() => {
            println!("  • Access Token:  {}", "FOUND".green());
        }
        _ => {
            println!("  • Access Token:  {}", "NOT FOUND".yellow());
        }
    }

    ok
}
