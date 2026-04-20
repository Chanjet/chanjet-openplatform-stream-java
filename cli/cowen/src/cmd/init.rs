use crate::auth::models::AuthMode;
use crate::core::config::ConfigManager;
use crate::core::vault::Vault;
use anyhow::Result;
use std::sync::Arc;
use std::io::{BufRead, Write};
use std::time::{Duration, Instant};

pub async fn execute(
    profile: &str,
    cfg_mgr: &ConfigManager,
    vault: Arc<dyn Vault>,
    app_key: &Option<String>,
    app_secret: &Option<String>,
    certificate: &Option<String>,
    encrypt_key: &Option<String>,
    webhook_target: &Option<String>,
    openapi_url: &Option<String>,
    stream_url: &Option<String>,
    app_mode: &Option<String>,
) -> Result<()> {
    let mut config = cfg_mgr.load(profile)?;

    // Determine Auth Mode
    let mode_str = app_mode.as_deref().unwrap_or("oauth2");
    let mode = if mode_str == "self-built" { AuthMode::SelfBuilt } else { AuthMode::Oauth2 };
    config.app_mode = mode;

    if mode == AuthMode::Oauth2 {
        if app_key.is_some() || app_secret.is_some() || certificate.is_some() {
            println!("Error: OAuth2 模式使用内置 ClientID，不支持手动指定 --app-key, --app-secret 或 --certificate/-c。");
            println!("提示: 如果您想使用自建应用模式，请显式指定 --app-mode self-built。");
            return Ok(());
        }
        config.app_key = crate::auth::models::BUILTIN_CLIENT_ID.to_string();
    } else {
        // Self-built mode: all credentials are required
        if app_key.is_none() || app_secret.is_none() || certificate.is_none() {
            let bin_name = crate::core::utils::get_bin_name();
            println!("Error: --app-key, --app-secret, and --certificate are required for self-built mode.");
            println!("Example: {} init --app-mode self-built --app-key X --app-secret Y --certificate Z", bin_name);
            return Ok(());
        }
        if let Some(ak) = app_key {
            config.app_key = ak.clone();
        }
    }
    
    // Assign a unique port if this is a new profile or it's currently 0 or 8080 (the old default)
    if config.proxy_port == 0 || config.proxy_port == 8080 {
        config.proxy_port = cfg_mgr.find_free_port();
    }

    // Secrets to vault (only for self-built mode with provided values)
    if mode == AuthMode::SelfBuilt {
        if let Some(as_val) = app_secret {
            vault.set(profile, "app_secret", as_val)?;
            config.app_secret = as_val.clone();
        }
    }

    if let Some(cert) = certificate {
        vault.set(profile, "certificate", cert)?;
        config.certificate = cert.clone();
    }

    if let Some(ek) = encrypt_key {
        vault.set(profile, "encrypt_key", ek)?;
        config.encrypt_key = ek.clone();
    }

    if let Some(wt) = webhook_target {
        config.webhook_target = wt.clone();
    }

    if let Some(ou) = openapi_url {
        config.openapi_url = ou.clone();
    }

    if let Some(su) = stream_url {
        config.stream_url = su.clone();
    }

    cfg_mgr.save(profile, &config)?;

    if mode == AuthMode::Oauth2 {
        println!("\n\x1b[1;34m🔒 Starting OAuth2 Authorization Flow...\x1b[0m");
        
        let token_pool = crate::auth::VaultTokenPool::new(vault.clone());
        let session_manager = crate::auth::lifecycle::AuthSessionManager::new(&token_pool);
        
        // 1. Get a free port for redirect_uri (Port is used by background listener later)
        let port = cfg_mgr.find_free_port();
        
        // 2. Create Session
        let session = session_manager.create_session(profile, port)?;
        
        // 3. Generate Auth URL (Ref: PRD §4.1 / Design §3.1)
        let market_url = obfs!(env!("DEF_MARKET_URL"));
        let auth_url = format!(
            "{}/user/v2/authorize?client_id={}&response_type=code&scope=all&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256",
            market_url.trim_end_matches('/'),
            config.app_key, // Built-in ID was injected earlier
            urlencoding::encode(&session.redirect_uri),
            session.state,
            crate::auth::provider::oauth2::Pkce::generate_challenge(&session.code_verifier),
        );

        println!("\n\x1b[1mPlease authorize in your browser. Opening URL...\x1b[0m");
        
        // 4. Automatically open browser
        if let Err(e) = open::that(&auth_url) {
            tracing::warn!(target: "sys", error = %e, "Failed to open browser automatically");
            println!("\x1b[33m(Failed to open browser automatically. Please copy the URL manually)\x1b[0m");
        }
        
        println!("\x1b[34m{}\x1b[0m", auth_url);
        
        // Render QR Code
        if let Ok(code) = qrcode::QrCode::new(&auth_url) {
            let string = code.render::<qrcode::render::unicode::Dense1x2>().build();
            println!("\n{}", string);
        }

        // 5. Spawn Background Finalizer
        println!("\n\x1b[34m🚀 授权监听已启动。请在浏览器中确认...\x1b[0m");

        spawn_finalizer(profile, &session.state)?;
        
        // 6. Wait for Result (Closed Loop)
        wait_for_token_exchange(profile, vault.clone()).await?;
    } else {
        println!("✅ Profile '{}' initialized successfully.", profile);
    }

    // Automatically attempt to install shell completion
    println!("⚙️ Configuring auto-completion...");
    let _ = crate::cmd::completion::install_completion(None);

    // Set as default profile if no default exists yet
    let app_dir = crate::core::config::get_app_dir();
    let current_profile_path = app_dir.join("current_profile");
    if !current_profile_path.exists() {
        let _ = cfg_mgr.set_default_profile(profile);
        println!("✅ Set default profile to '{}'", profile);
    }

    Ok(())
}

fn spawn_finalizer(profile: &str, session_id: &str) -> anyhow::Result<()> {
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

    cmd.spawn()?;
    Ok(())
}

async fn wait_for_token_exchange(profile: &str, vault: Arc<dyn Vault>) -> anyhow::Result<()> {
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
        if start_time.elapsed() > timeout {
            println!("\n❌ 授权超时 (5 分钟)。请检查网络或重新运行 `init`。");
            render_last_auth_error(profile)?;
            return Err(anyhow::anyhow!("Authorization timeout"));
        }

        // 1. Success check: Token pair exists
        if vault.get(profile, "oauth2_token_pair").is_ok() {
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
                reader.seek_relative(last_log_size as i64)?;
                
                for line in reader.lines() {
                    if let Ok(l) = line {
                        if l.contains("ERROR") {
                            println!("\n❌ 令牌交换失败！");
                            println!("\x1b[31m🔍 错误原因: {}\x1b[0m", l);
                            return Err(anyhow::anyhow!("Token exchange failed"));
                        }
                    }
                }
                last_log_size = metadata.len();
            }
        }

        // 3. State check: If code is cleared but no token pair
        if vault.get(profile, "captured_auth_code").is_err() && vault.get(profile, "oauth2_token_pair").is_err() {
            // Wait a small buffer for the final write
            tokio::time::sleep(Duration::from_millis(500)).await;
            if vault.get(profile, "oauth2_token_pair").is_err() {
                println!("\n❌ 授权码已失效且未获取到新令牌。授权过程可能已在其他地方中断或失败。");
                render_last_auth_error(profile)?;
                return Err(anyhow::anyhow!("Authorization state invalid"));
            }
        }
        
        tokio::time::sleep(Duration::from_millis(500)).await;
        print!(".");
        std::io::stdout().flush()?;
    }
}

fn render_last_auth_error(profile: &str) -> anyhow::Result<()> {
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
