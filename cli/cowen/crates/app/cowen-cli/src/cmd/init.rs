use cowen_common::grpc::client::DaemonResponse;

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
    pub file: Option<String>,
}

fn read_config_file_to_json(file_path: &str) -> anyhow::Result<String> {
    use anyhow::Context;
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read config file at: {}", file_path))?;
    let yaml_val: serde_yaml::Value = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse YAML file at: {}", file_path))?;
    let json_val =
        serde_json::to_value(&yaml_val).context("Failed to convert YAML config to JSON")?;
    let json_str = serde_json::to_string(&json_val).context("Failed to serialize config JSON")?;
    Ok(json_str)
}

pub async fn execute(profile: &str, ctx: InitContext) -> anyhow::Result<()> {
    println!("\n🚀 Initializing profile: \x1b[1;32m{}\x1b[0m", profile);

    // Ensure the daemon is running before initialization so we can start the worker after via IPC
    let port_path = crate::get_ipc_port_path();
    let _ = cowen_common::grpc::client::DaemonClient::new(&port_path)
        .ensure_daemon()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to ensure daemon is running for init: {:#}", e))?;

    let ipc = cowen_common::grpc::client::DaemonClient::new(port_path);

    let mut config_json = None;
    if let Some(ref file_path) = ctx.file {
        match read_config_file_to_json(file_path) {
            Ok(js) => config_json = Some(js),
            Err(e) => {
                eprintln!("❌ Failed to read config file: {}", e);
                return Err(e);
            }
        }
    }

    match ipc
        .init_profile(
            profile,
            ctx.app_key.as_deref(),
            ctx.app_secret.as_deref(),
            ctx.certificate.as_deref(),
            ctx.encrypt_key.as_deref(),
            ctx.webhook_target.as_deref(),
            ctx.openapi_url.as_deref(),
            ctx.stream_url.as_deref(),
            ctx.app_mode.as_deref(),
            ctx.proxy_port.map(|p| p as u32),
            config_json.clone(),
        )
        .await
    {
        Ok(DaemonResponse::Success { message }) => {
            println!("✅ {}", message);

            // Run login flow interactively for supported modes
            let mut final_mode = ctx.app_mode.clone().unwrap_or_else(|| "oauth2".to_string());
            if let Some(ref js_str) = config_json {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(js_str) {
                    if let Some(am) = v.get("app_mode").and_then(|m| m.as_str()) {
                        final_mode = am.to_string();
                    }
                }
            }
            let mode_str = final_mode.to_lowercase().replace("-", "_");
            if mode_str == "oauth2" {
                let mut sigterm =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
                let login_result = tokio::select! {
                    res = crate::cmd::auth::login(profile, false) => res,
                    _ = tokio::signal::ctrl_c() => {
                        eprintln!("\n❌ Initialization cancelled (SIGINT). Cleaning up...");
                        Err(anyhow::anyhow!("Initialization cancelled"))
                    }
                    _ = sigterm.recv() => {
                        eprintln!("\n❌ Initialization cancelled (SIGTERM). Cleaning up...");
                        Err(anyhow::anyhow!("Initialization cancelled"))
                    }
                };

                if login_result.is_err() {
                    // Clean up the profile if initialization failed or was cancelled
                    let _ = ipc.stop_daemon(profile).await;
                    let _ = ipc.system_reset(Some(profile), false).await;
                    std::process::exit(130);
                }
            }

            // Start the worker since it's a new profile

            let _ = ipc.start_daemon(profile).await;

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
            Err(e)
        }
        _ => {
            eprintln!("❌ Unexpected response from daemon");
            Err(anyhow::anyhow!("Unexpected response"))
        }
    }
}
