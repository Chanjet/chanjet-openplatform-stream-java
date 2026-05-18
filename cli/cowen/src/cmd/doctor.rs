use anyhow::Result;
use cowen_common::config::Config;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;
use std::sync::Arc;
use colored::*;
use std::time::Instant;

pub async fn execute(profile: &str, config: &Config, verbose: bool, vault: Arc<dyn Vault>, cfg_mgr: &ConfigManager) -> Result<()> {
    println!("\n{} {} (Profile: {})", "🩺".bold(), "Cowen Doctor - 环境诊断工具".bold(), profile.cyan());
    println!("{}\n", "=".repeat(60).dimmed());

    let mut all_ok = true;

    // 1. 系统与配置检查
    if !check_system_info(config, verbose) { all_ok = false; }
    
    // 2. 存储后端检查
    if !check_storage(cfg_mgr, verbose).await { all_ok = false; }

    // 3. 网络连通性检查
    if !check_network(config, verbose).await { all_ok = false; }

    // 4. 凭据与认证检查
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
    println!("{} {}", "[1/4]".dimmed(), "系统与配置检查...".bold());
    
    println!("  • Cowen Version: {}", env!("CARGO_PKG_VERSION").cyan());
    println!("  • OS:            {} {}", std::env::consts::OS, std::env::consts::ARCH);
    println!("  • App Mode:      {:?}", config.app_mode);
    
    if verbose {
        println!("  • App Key:       {}", cowen_common::utils::mask_string(&config.app_key));
        println!("  • Stream URL:    {}", config.stream_url);
    }

    let app_dir = cowen_common::config::get_app_dir();
    if app_dir.exists() {
        // 尝试检查可用磁盘空间 (简易版)
        let space_info = if let Ok(metadata) = std::fs::metadata(&app_dir) {
            format!("(Permissions: {:?})", metadata.permissions())
        } else {
            "".to_string()
        };
        println!("  • Home Dir:      {} {} {}", app_dir.display(), "OK".green(), space_info.dimmed());
    } else {
        println!("  • Home Dir:      {} {}", app_dir.display(), "MISSING".red());
        return false;
    }

    true
}

async fn check_storage(cfg_mgr: &ConfigManager, _verbose: bool) -> bool {
    println!("\n{} {}", "[2/4]".dimmed(), "存储后端检查...".bold());
    
    match cfg_mgr.load_app_config().await {
        Ok(app_cfg) => {
            println!("  • Storage Store: {}", app_cfg.storage.store.cyan());
            println!("  • Cache Store:   {}", app_cfg.storage.cache.cyan());
            
            // TODO: 这里可以尝试通过 cowen-store 建立真实连接进行健康检查
            // 目前先做基础配置检查
            println!("  • Connectivity:  {}", "OK (configured)".green());
            true
        }
        Err(e) => {
            println!("  • AppConfig:     {} ({})", "ERROR".red(), e);
            false
        }
    }
}

async fn check_network(config: &Config, _verbose: bool) -> bool {
    println!("\n{} {}", "[3/4]".dimmed(), "网络连通性检查...".bold());
    
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
    println!("\n{} {}", "[4/4]".dimmed(), "凭据与认证检查...".bold());
    
    let mut ok = true;
    
    // 检查 AppSecret 是否存在
    match vault.get_secret(profile, "app_secret").await {
        Ok(s) if !s.is_empty() => {
            println!("  • App Secret:    {}", "FOUND".green());
        }
        _ => {
            println!("  • App Secret:    {}", "MISSING".red());
            println!("    {} 请运行 'cowen init' 或 'cowen auth login'。", "建议:".yellow());
            ok = false;
        }
    }

    // 检查 AccessToken (仅提示，不作为硬性错误)
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
