use crate::core::config::ConfigManager;
use crate::core::vault::Vault;
use anyhow::Result;

pub async fn execute(
    profile: &str,
    cfg_mgr: &ConfigManager,
    vault: &dyn Vault,
    app_key: &Option<String>,
    app_secret: &Option<String>,
    certificate: &Option<String>,
    encrypt_key: &Option<String>,
    webhook_target: &Option<String>,
    openapi_url: &Option<String>,
    stream_url: &Option<String>,
) -> Result<()> {
    if app_key.is_none() || app_secret.is_none() || certificate.is_none() {
        println!("Error: --app-key, --app-secret, and --certificate are required for init.");
        println!("Example: owenc init --app-key X --app-secret Y --certificate Z");
        return Ok(());
    }

    let mut config = cfg_mgr.load(profile)?;

    if let Some(ak) = app_key {
        config.app_key = ak.clone();
    }

    if let Some(as_val) = app_secret {
        vault.set(profile, "app_secret", as_val)?;
        config.app_secret = as_val.clone();
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

    println!("Profile '{}' initialized successfully.", profile);
    
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

    // Automatically start the daemon in background
    if let Err(e) = crate::cmd::daemon::start(profile, &config, 8080, false).await {
        eprintln!("⚠️ Failed to auto-start daemon: {}", e);
    }

    Ok(())
}
