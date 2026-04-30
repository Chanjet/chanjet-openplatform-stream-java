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
        let token_pool = crate::auth::VaultTokenPool::new(vault.clone());
        let auth_cli = crate::auth::AuthClient::new(&token_pool);
        let requested_key = auth_cli.provider(&requested_mode).get_default_app_key()
            .unwrap_or_else(|| app_key.clone().unwrap_or_default());

        if config.app_mode == requested_mode && config.app_key == requested_key {
            println!("✅ Profile '{}' is already initialized with these settings.", profile);
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
    } else {
        // Logic Fix: Save early for new profiles to anchor the identity
        cfg_mgr.save(profile, &config).await?;
    }

    // Determine Auth Mode
    let mode_str = app_mode.as_deref().unwrap_or("oauth2");
    let mode = match mode_str {
        "self-built" => AuthMode::SelfBuilt,
        "store-app" => AuthMode::StoreApp,
        _ => AuthMode::Oauth2,
    };
    config.app_mode = mode;

    // 1. Generic Parameter Assignment (Personality-agnostic)
    if let Some(ak) = app_key {
        config.app_key = ak.clone();
    }
    if let Some(as_val) = app_secret {
        vault.set(profile, "app_secret", as_val).await?;
        config.app_secret = as_val.clone();
    }
    if let Some(cert) = certificate {
        vault.set(profile, "certificate", cert).await?;
        config.certificate = cert.clone();
    }
    if let Some(ek) = encrypt_key {
        vault.set(profile, "encrypt_key", ek).await?;
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

    // Assign a unique port if this is a new profile or it's currently 0 or 8080 (the old default)
    if config.proxy_port == 0 || config.proxy_port == 8080 {
        config.proxy_port = cfg_mgr.find_free_port().await;
    }

    // 2. Persist configurations (Port, Mode, URLs, etc.)
    cfg_mgr.save_app_config(app_config).await?;
    cfg_mgr.save(profile, &config).await?;

    // 3. Delegate Mode-Specific Initialization (Personality) to Provider
    let token_pool = crate::auth::VaultTokenPool::new(vault.clone());
    let auth_cli = crate::auth::AuthClient::new(&token_pool);
    let provider = auth_cli.provider(&config.app_mode);
    
    provider.initialize(profile, &config, vault.clone(), cfg_mgr).await?;

    // Automatically attempt to install shell completion
    println!("⚙️ Configuring auto-completion...");
    let _ = crate::cmd::completion::install_completion(None);

    // Automatically set the new profile as the active one
    let _ = cfg_mgr.set_default_profile(profile);
    println!("✅ Active profile switched to '{}'", profile);

    Ok(())
}
