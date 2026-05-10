use cowen_auth::models::AuthMode;
use cowen_common::ConfigManager;
use cowen_common::vault::Vault;
use std::sync::Arc;

struct InitCleanupGuard {
    profile: String,
    cfg_mgr: ConfigManager,
    is_new: bool,
    active: bool,
}

impl InitCleanupGuard {
    fn new(profile: &str, cfg_mgr: &ConfigManager, is_new: bool) -> Self {
        Self {
            profile: profile.to_string(),
            cfg_mgr: cfg_mgr.clone(),
            is_new,
            active: true,
        }
    }

    fn cancel(&mut self) {
        self.active = false;
    }
}

impl Drop for InitCleanupGuard {
    fn drop(&mut self) {
        if self.active && self.is_new {
            let p = self.profile.clone();
            let m = self.cfg_mgr.clone();
            
            // 🚀 Synchronous deletion of local config file to ensure CLI immediate consistency
            let app_dir = cowen_common::config::get_app_dir();
            let yaml_path = app_dir.join(format!("{}.yaml", p));
            if yaml_path.exists() {
                let _ = std::fs::remove_file(yaml_path);
            }

            // Still try to delete from Vault if possible (best effort in drop)
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let _ = handle.spawn(async move {
                    let _ = m.delete(&p).await;
                });
            }
        }
    }
}

pub async fn execute(
    profile: &str,
    cfg_mgr: &ConfigManager,
    _app_config: &mut cowen_common::AppConfig,
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
    daemon_svc: Option<Arc<dyn cowen_common::daemon::DaemonService>>,
) -> anyhow::Result<()> {
    println!("\n🚀 Initializing profile: \x1b[1;32m{}\x1b[0m", profile);
    
    let is_new = !cfg_mgr.exists(profile).await;
    let mut _guard = InitCleanupGuard::new(profile, cfg_mgr, is_new);

    let mut config = cfg_mgr.load(profile).await.map_err(|e| anyhow::anyhow!(e))?;

    // 1. Resolve Mode
    let mode_str = app_mode.clone().unwrap_or_else(|| {
        if is_new { "oauth2".to_string() } else { format!("{:?}", config.app_mode).to_lowercase() }
    });
    
    let mode = match mode_str.as_str() {
        "self_built" | "self-built" => AuthMode::SelfBuilt,
        "oauth2" => AuthMode::Oauth2,
        "store_app" | "store-app" => AuthMode::StoreApp,
        _ => return Err(anyhow::anyhow!("Invalid app-mode: {}. Supported: self_built, oauth2, store_app", mode_str)),
    };
    config.app_mode = mode.clone();

    // 2. Initialize Provider
    let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
    let provider = auth_cli.provider(&mode);

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

    if let Err(e) = provider.initialize(profile, &mut config, vault.clone(), cfg_mgr, params, daemon_svc).await {
        eprintln!("❌ Initialization failed: {}", e);
        return Err(e.into());
    }

    // 4. Post-init: Install autocomplete if not already
    let _ = crate::cmd::completion::install_completion(None);

    // Automatically set the new profile as the active one
    let _ = cfg_mgr.set_default_profile(profile);
    _guard.cancel();
    println!("✅ Active profile switched to '{}'", profile);

    Ok(())
}
