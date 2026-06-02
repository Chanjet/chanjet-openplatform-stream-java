use cowen_common::ipc::client::IpcDaemonService;
use cowen_common::ipc::DaemonResponse;

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
}

pub async fn execute(
    profile: &str,
    ctx: InitContext,
) -> anyhow::Result<()> {
    println!("\n🚀 Initializing profile: \x1b[1;32m{}\x1b[0m", profile);
    
    let mode_str = ctx.app_mode.clone().unwrap_or_else(|| "oauth2".to_string());
    if mode_str == "oauth2" {
        // Run oauth2 flow locally on the CLI side
        use cowen_common::models::AuthMode;
        
        let app_dir = cowen_common::config::get_app_dir();
        let cfg_mgr = cowen_config::ConfigManager::new().map_err(|e| anyhow::anyhow!(e))?;
        
        let mut config = match cfg_mgr.load(profile).await {
            Ok(c) => c,
            Err(_) => cowen_common::Config::default_with_profile(profile),
        };
        config.app_mode = AuthMode::Oauth2;
        
        let app_cfg = cfg_mgr.load_app_config().await?;
        let fingerprint = cowen_common::security::get_machine_fingerprint()?;
        let vault = cowen_store::create_vault(&app_cfg, &app_dir, &fingerprint).await?;
        
        let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
        let provider = auth_cli.provider(&AuthMode::Oauth2);
        
        let params = cowen_auth::provider::InitParams {
            app_key: ctx.app_key,
            app_secret: ctx.app_secret,
            certificate: ctx.certificate,
            encrypt_key: ctx.encrypt_key,
            webhook_target: ctx.webhook_target,
            openapi_url: ctx.openapi_url,
            stream_url: ctx.stream_url,
            proxy_port: ctx.proxy_port,
            auto_start: true,
            is_new: !cfg_mgr.exists(profile).await,
        };
        
        provider.initialize(profile, &mut config, vault.clone(), &cfg_mgr, params, None).await?;
        
        let _ = cfg_mgr.set_default_profile(profile);
        println!("✅ Profile {} initialized", profile);
        println!("✅ Active profile switched to '{}'", profile);
        
        let _ = crate::cmd::completion::install_completion(None);
        return Ok(());
    }

    // Fallback for self_built / sidecar modes using IPC
    let port_path = cowen_common::ipc::get_ipc_port_path();
    let _ = cowen_common::ipc::client::ensure_daemon(&port_path).await
        .map_err(|e| anyhow::anyhow!("Failed to ensure daemon is running for init: {}", e))?;

    let ipc = IpcDaemonService::new(port_path);
    match ipc.init_profile(
        profile,
        ctx.app_key,
        ctx.app_secret,
        ctx.certificate,
        ctx.encrypt_key,
        ctx.webhook_target,
        ctx.openapi_url,
        ctx.stream_url,
        ctx.app_mode,
        ctx.proxy_port,
    ).await {
        Ok(DaemonResponse::Success { message }) => {
            println!("✅ {}", message);
            let _ = crate::cmd::completion::install_completion(None);
            println!("✅ Active profile switched to '{}'", profile);
            Ok(())
        }
        Ok(DaemonResponse::Error { message, .. }) => {
            eprintln!("❌ Initialization failed: {}", message);
            Err(anyhow::anyhow!(message))
        }
        Err(e) => {
            eprintln!("❌ IPC Error: {}", e);
            Err(e.into())
        }
        _ => {
            eprintln!("❌ Unexpected response from daemon");
            Err(anyhow::anyhow!("Unexpected response"))
        }
    }
}
