use cowen_auth::models::AuthMode;
use crate::core::config::ConfigManager;
use crate::core::vault::Vault;
use anyhow::Result;
use std::sync::Arc;

pub async fn execute(
    profile: &str,
    cfg_mgr: &ConfigManager,
    app_config: &mut crate::core::config::AppConfig,
    vault: Arc<dyn Vault>,
    app_key: &Option<String>,
    app_secret: &Option<String>,
    certificate: &Option<String>,
    encrypt_key: &Option<String>,
    webhook_target: &Option<String>,
    openapi_url: &Option<String>,
    stream_url: &Option<String>,
    app_mode: &Option<String>,
    proxy_port: &Option<u16>,
    auto_start: bool,
) -> Result<()> {
    let is_new = !cfg_mgr.exists(profile).await;
    let mut config = cfg_mgr.load(profile).await?;

    // Validation for existing profiles: Ensure we are not accidentally overwriting a different application
    if !is_new {
        let mode_str = app_mode.as_deref().unwrap_or("oauth2");
        let requested_mode = match mode_str {
            "self-built" => AuthMode::SelfBuilt,
            "store-app" => AuthMode::StoreApp,
            _ => AuthMode::Oauth2,
        };

        // Determine requested key using SPI (Personality check)
        let token_pool = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));
        let auth_cli = cowen_auth::create_auth_client(token_pool.clone());
        let requested_key = auth_cli
            .provider(&requested_mode)
            .get_default_app_key()
            .unwrap_or_else(|| app_key.clone().unwrap_or_default());

        if config.app_mode == requested_mode && config.app_key == requested_key {
            println!(
                "✅ Profile '{}' is already initialized with these settings.",
                profile
            );
            println!("💡 To update configurations (port, webhook, etc.), use the 'cowen config' command.");
            println!("💡 To re-authorize or refresh tokens, use 'cowen auth login'.");
            return Ok(());
        } else {
            anyhow::bail!(
                "Profile '{}' already exists with different settings (Mode: {:?}, AppKey: {}).\n\
                Use a different profile name or 'reset' this one if you want to switch applications.",
                profile, config.app_mode, config.app_key
            );
        }
    }
    // Determine Auth Mode
    let mode_str = app_mode.as_deref().unwrap_or("oauth2");
    let mode = match mode_str {
        "self-built" => AuthMode::SelfBuilt,
        "store-app" => AuthMode::StoreApp,
        _ => AuthMode::Oauth2,
    };
    config.app_mode = mode;

    if is_new {
        // Logic Fix: Save early for new profiles to anchor the identity
        cfg_mgr.save(profile, &mut config).await?;
        config.version += 1;
    }

    // 1. Delegate All Mode-Specific Initialization (Personality) to Provider
    let token_pool = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));
    let auth_cli = cowen_auth::create_auth_client(token_pool.clone());
    
    // Collect all parameters into InitParams
    let params = cowen_auth::provider::InitParams {
        app_key: app_key.clone(),
        app_secret: app_secret.clone(),
        certificate: certificate.clone(),
        encrypt_key: encrypt_key.clone(),
        webhook_target: webhook_target.clone(),
        openapi_url: openapi_url.clone(),
        stream_url: stream_url.clone(),
        proxy_port: proxy_port.clone(),
        auto_start,
        is_new,
    };

    // Assign a unique port if this is a new profile or it's currently 0 or 8080/57612 (defaults)
    if config.proxy_port == 0 || config.proxy_port == 8080 || config.proxy_port == 57612 {
        config.proxy_port = cfg_mgr.find_free_port().await;
    }

    // Generic configurations
    cfg_mgr.save_app_config(app_config).await?;

    // The Provider now handles credential setup, config saving (via cfg_mgr), and daemon startup.
    let init_res = auth_cli.provider(&config.app_mode)
        .initialize(profile, &mut config, vault.clone(), cfg_mgr, params, None)
        .await;

    if let Err(e) = init_res {
        if is_new {
            tracing::warn!(target: "sys", profile = %profile, error = %e, "Initialization failed for new profile, performing cleanup");
            let _ = cfg_mgr.delete(profile).await;
        }
        return Err(e);
    }

    // Automatically attempt to install shell completion
    println!("⚙️ Configuring auto-completion...");
    let _ = crate::cmd::completion::install_completion(None);

    // Automatically set the new profile as the active one
    let _ = cfg_mgr.set_default_profile(profile);
    println!("✅ Active profile switched to '{}'", profile);

    Ok(())
}
