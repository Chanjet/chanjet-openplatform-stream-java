use crate::auth::models::AuthMode;
use crate::core::config::ConfigManager;
use crate::core::vault::Vault;
use anyhow::Result;
use std::sync::Arc;

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
            "{}/v2/userAuth/authorize?client_id={}&response_type=code&scope=all&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256",
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

        // 5. Spawn Background Finalizer (Non-blocking)
        println!("\n\x1b[32m⚙️  Background finalizer derived. You can continue using terminal.\x1b[0m");
        println!("⏳ Waiting for browser authorization (Timeout: 5 minutes)...");

        spawn_finalizer(profile, &session.state)?;

    } else {
        println!("Profile '{}' initialized successfully.", profile);
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

    if mode != AuthMode::Oauth2 {
        // Automatically start the daemon in background (only for self-built)
        let _ = crate::cmd::daemon::stop(profile, false, cfg_mgr).await;
        if let Err(e) = crate::cmd::daemon::start(profile, &config, config.proxy_port, false, false, false, cfg_mgr, vault.clone()).await {
            eprintln!("⚠️ Failed to auto-start daemon: {}", e);
        } else {
            println!("💡 Security handshake is running in background. First API call may take a few seconds to authorize.");
        }
    } else {
        println!("\n💡 授权完成后，您可以通过 \x1b[33mowenc daemon start\x1b[0m 启动本地反向代理服务。");
    }

    Ok(())
}

fn spawn_finalizer(profile: &str, session_id: &str) -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    
    cmd.args(&["--profile", profile, "auth", "login", "--finalize", session_id])
       .stdin(std::process::Stdio::null())
       .stdout(std::process::Stdio::null())
       .stderr(std::process::Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    cmd.spawn()?;
    Ok(())
}
