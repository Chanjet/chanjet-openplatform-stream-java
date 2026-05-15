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

    async fn cleanup(&mut self) {
        if self.active && self.is_new {
            self.active = false; // Prevent double cleanup
            let _ = self.cfg_mgr.delete(&self.profile).await;
        }
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

/// Aggregated parameters for the init command.
/// Decouples the execute() signature from individual CLI flags,
/// so adding new parameters only requires changing this struct.
pub struct InitContext {
    pub app_key: Option<String>,
    pub app_secret: Option<String>,
    pub certificate: Option<String>,
    pub encrypt_key: Option<String>,
    pub webhook_target: Option<String>,
    pub openapi_url: Option<String>,
    pub stream_url: Option<String>,
    pub app_mode: Option<String>,
    pub proxy_port: Option<u16>,
    pub auto_start: bool,
}

pub async fn execute(
    profile: &str,
    cfg_mgr: &ConfigManager,
    _app_config: &mut cowen_common::AppConfig,
    vault: Arc<dyn Vault>,
    ctx: InitContext,
    daemon_svc: Option<Arc<dyn cowen_common::daemon::DaemonService>>,
) -> anyhow::Result<()> {
    let is_new = !cfg_mgr.exists(profile).await;
    
    // 1. Resolve Mode — delegates parsing to AuthMode::FromStr, no variant awareness needed
    let mode = if let Some(m_str) = &ctx.app_mode {
        m_str.parse().map_err(|e: String| anyhow::anyhow!(e))?
    } else if !is_new {
        let config = cfg_mgr.load(profile).await.map_err(|e| anyhow::anyhow!(e))?;
        config.app_mode.clone()
    } else {
        Default::default()
    };

    // 2. Obtain provider — all mode-specific logic is delegated through this handle
    let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
    let provider = auth_cli.provider(&mode);

    // 3. Check for duplicate parameters — strategy delegated to provider
    if let Some(ak) = &ctx.app_key {
        let conflict_profile = provider
            .find_conflicting_profile(ak, cfg_mgr)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        if let Some(existing_profile) = conflict_profile {
            if existing_profile != profile {
                println!("💡 Profile with same parameters already exists: \x1b[1;33m{}\x1b[0m", &existing_profile);
                println!("   Switching to existing profile instead of creating '{}'.", profile);
                let _ = cfg_mgr.set_default_profile(&existing_profile);
                
                // 🚀 ENSURE DAEMON RUNNING for the existing profile too
                if ctx.auto_start {
                    if let Ok(existing_cfg) = cfg_mgr.load(&existing_profile).await {
                        if let Some(ds) = &daemon_svc {
                            let _ = ds.start_daemon(&existing_profile, &existing_cfg, vault.clone()).await;
                        }
                    }
                }
                return Ok(());
            }
        }
    }

    println!("\n🚀 Initializing profile: \x1b[1;32m{}\x1b[0m", profile);
    
    let mut _guard = InitCleanupGuard::new(profile, cfg_mgr, is_new);
    let mut config = cfg_mgr.load(profile).await.map_err(|e| anyhow::anyhow!(e))?;
    config.app_mode = mode;

    // 🚀 RESOLVE RANDOM PORT: If port is 0 (default or explicit), pick a stable one now
    // and persist it, so subsequent restarts are consistent.
    let mut resolved_ctx = ctx;
    if resolved_ctx.proxy_port == Some(0) || (resolved_ctx.proxy_port.is_none() && config.proxy_port == 0) {
        let free_port = cfg_mgr.find_free_port().await;
        if free_port != 0 {
            tracing::info!(target: "sys", profile = %profile, port = %free_port, "Assigned random stable port during init");
            resolved_ctx.proxy_port = Some(free_port);
        }
    }

    // 4. Initialize Provider — all parameter validation, vault writes, daemon start delegated
    let params = cowen_auth::provider::InitParams {
        app_key: resolved_ctx.app_key,
        app_secret: resolved_ctx.app_secret,
        certificate: resolved_ctx.certificate,
        encrypt_key: resolved_ctx.encrypt_key,
        webhook_target: resolved_ctx.webhook_target,
        openapi_url: resolved_ctx.openapi_url,
        stream_url: resolved_ctx.stream_url,
        proxy_port: resolved_ctx.proxy_port,
        auto_start: resolved_ctx.auto_start,
        is_new,
    };

    if let Err(e) = provider.initialize(profile, &mut config, vault.clone(), cfg_mgr, params, daemon_svc).await {
        eprintln!("❌ Initialization failed: {}", e);
        _guard.cleanup().await;
        return Err(e.into());
    }

    // 5. Post-init: Install autocomplete if not already
    let _ = crate::cmd::completion::install_completion(None);

    // Automatically set the new profile as the active one
    let _ = cfg_mgr.set_default_profile(profile);
    _guard.cancel();
    println!("✅ Active profile switched to '{}'", profile);

    Ok(())
}
