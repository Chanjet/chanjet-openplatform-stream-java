#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use clap::Parser;
use tracing::{info, error};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tokio::net::{TcpListener, TcpStream};

use cowen_common::ipc::{DaemonRequest, DaemonResponse, ApiResponseDto};
use cowen_common::daemon::DaemonService;
use cowen_common::vault::Vault;
use cowen_server::ServerDaemonService;
use cowen_config::ConfigManager;
use cowen_auth::client::Client;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// IPC port file path
    #[arg(long)]
    ipc_port_file: Option<PathBuf>,

    /// Run as Windows Service
    #[arg(long)]
    run_as_service: bool,

    /// Automatically start all profiles on startup
    #[arg(long)]
    auto_start_all: bool,

    /// Force specific app directory (useful for Windows Service running as SYSTEM)
    #[arg(long)]
    app_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if let Some(dir) = &args.app_dir {
        std::env::set_var("COWEN_HOME", dir);
    }

    if args.run_as_service {
        let pid_file_clone = cowen_common::config::get_app_dir().join("master_daemon.pid");
        let auto_start = args.auto_start_all;
        return cowen_sys::get_process_manager().run_as_service(Box::new(move || {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(run_main(&pid_file_clone, None, auto_start))
        })).await;
    }

    let app_dir = cowen_common::config::get_app_dir();
    let pid_file = app_dir.join("master_daemon.pid");

    let result = run_main(&pid_file, args.ipc_port_file.clone(), args.auto_start_all).await;
    if let Err(e) = &result {
        // FATAL CRASH: Write LAST_ERROR to PID file so CLI can report it synchronously
        let current_pid = std::process::id();
        let error_msg = e.to_string().replace('\n', " ");
        let start_time = chrono::Utc::now().to_rfc3339();
        let _ = cowen_common::utils::secure_write(&pid_file, format!("{}\nSTART_TIME={}\nLAST_ERROR={}\nBUILD_ID={}\nBUILD_TIME={}", current_pid, start_time, error_msg, cowen_common::BUILD_ID, cowen_common::BUILD_TIME));
    }
    result
}

async fn run_main(pid_file: &PathBuf, ipc_port_file: Option<PathBuf>, auto_start_all: bool) -> Result<()> {
    // Initialize Rustls Crypto Provider (Mandatory for Rustls 0.23+)
    let _ = rustls::crypto::ring::default_provider().install_default();

    // CAPTURE PANICS: Ensure background crashes are recorded
    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload().downcast_ref::<&str>().cloned()
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("no message");
        tracing::error!("FATAL DAEMON PANIC: {}", payload);
    }));

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let ipc_port = listener.local_addr()?.port();
    
    let target_ipc_port_file = ipc_port_file.unwrap_or_else(|| cowen_common::config::get_app_dir().join("ipc.port"));
    let target_ipc_token_file = target_ipc_port_file.with_file_name("ipc.token");
    let ipc_token = uuid::Uuid::new_v4().to_string();
    let current_pid = std::process::id();
    let start_time = chrono::Utc::now().to_rfc3339();

    let cfg_mgr = ConfigManager::new().map_err(|e| anyhow::anyhow!("Failed to init ConfigManager: {}", e))?;
    let app_dir = cowen_common::config::get_app_dir();
    let telemetry_db = Arc::new(
        cowen_monitor::telemetry_db::TelemetryDb::new(&app_dir.join("telemetry.db"))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to init telemetry db: {}", e))?
    );
    let _ = telemetry_db.run_gc().await;

    let mut app_cfg = if let Ok(content) = std::fs::read_to_string(app_dir.join("app.yaml")) {
        serde_yaml::from_str::<cowen_common::config::AppConfig>(&content).unwrap_or_default()
    } else {
        cowen_common::config::AppConfig::default()
    };
    app_cfg.apply_env_overrides();

    if let (Ok(st), Ok(url)) = (std::env::var("COWEN_STORE_TYPE"), std::env::var("COWEN_DB_URL")) {
        app_cfg.storage.store = st;
        app_cfg.storage.db_url = Some(url);
    }

    let fingerprint = cowen_common::security::get_machine_fingerprint().unwrap_or_default();
    let vault = cowen_store::create_vault(&app_cfg, &app_dir, &fingerprint).await.map_err(|e| anyhow::anyhow!("Failed to init Vault: {}", e))?;
    let _ = cfg_mgr.set_vault(vault.clone());

    use tracing_subscriber::prelude::*;
    use tracing_subscriber::fmt::writer::MakeWriterExt;
    
    let make_writer = std::io::stderr
        .with_max_level(tracing::Level::WARN)
        .or_else(std::io::stdout);

    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(make_writer)
        .with_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")));
    
    let (_vault_tx, vault_rx) = tokio::sync::watch::channel(Some(vault.clone()));
    let vault_audit_layer = cowen_monitor::audit::VaultAuditLayer::new(vault_rx);

    tracing_subscriber::registry()
        .with(console_layer)
        .with(vault_audit_layer)
        .init();
        
    info!("Starting cowen-daemon...");
    info!("Listening on TCP IPC port: {}", ipc_port);
    
    // Write IPC port early so CLI clients can connect (they will block until the accept loop starts)
    let _ = cowen_sys::get_ipc_binder().save_ipc_token(&target_ipc_token_file, &ipc_token).await;
    cowen_common::utils::secure_write(&target_ipc_port_file, ipc_port.to_string())?;
    
    let daemon_svc: Arc<dyn DaemonService> = Arc::new(ServerDaemonService::new(cfg_mgr.clone()));

    let mut m_port = app_cfg.monitor_port;
    let mut allow_fallback = std::env::var("COWEN_ALLOW_PORT_FALLBACK").is_ok();
    if m_port == 0 {
        m_port = 1588;
        allow_fallback = true;
    }
    let m_server = cowen_monitor::MonitorServer::new(m_port, daemon_svc.clone(), Some(telemetry_db.clone()));
    let (port_tx, port_rx) = tokio::sync::oneshot::channel();
    let (monitor_shutdown_tx, monitor_shutdown_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        if let Err(e) = m_server.start(Some(port_tx), allow_fallback, monitor_shutdown_rx).await {
            tracing::error!("Monitor server error: {}", e);
        }
    });
    
    let actual_m_port = match tokio::time::timeout(tokio::time::Duration::from_secs(5), port_rx).await {
        Ok(Ok(p)) => p,
        Ok(Err(_)) => {
            tracing::error!("Monitor server failed to start (e.g., port occupied). Aborting.");
            return Err(anyhow::anyhow!("Monitor server failed to start. Port may be occupied."));
        }
        Err(_) => {
            tracing::error!("Timed out waiting for monitor server to start. Aborting.");
            return Err(anyhow::anyhow!("Monitor server start timeout"));
        }
    };

    // We do not save actual_m_port back to app_cfg to avoid overwriting user configuration.
    // The CLI can discover the fallback port via the pid file's MONITOR_PORT field.

    let _ = cowen_common::utils::secure_write(pid_file, format!("{}\nMONITOR_PORT={}\nSTART_TIME={}\nBUILD_ID={}\nBUILD_TIME={}", current_pid, actual_m_port, start_time, cowen_common::BUILD_ID, cowen_common::BUILD_TIME));

    // Signal-aware accept loop
    let (stop_tx, mut stop_rx) = tokio::sync::mpsc::channel(1);

    cowen_sys::get_process_manager().set_stop_channel(stop_tx.clone());

    let stop_tx_ctrl_c = stop_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::warn!("Ctrl-C listener failed (likely headless): {}", e);
        } else {
            let _ = stop_tx_ctrl_c.send(()).await;
        }
    });

    #[cfg(unix)]
    {
        let stop_tx_sigterm = stop_tx.clone();
        tokio::spawn(async move {
            use tokio::signal::unix::{signal, SignalKind};
            if let Ok(mut stream) = signal(SignalKind::terminate()) {
                stream.recv().await;
                tracing::info!("SIGTERM received, sending shutdown signal...");
                let _ = stop_tx_sigterm.send(()).await;
            }
        });
    }

    if auto_start_all {
        if let Ok(profiles) = cfg_mgr.list_profiles().await {
            for p in profiles {
                let p_cfg = cfg_mgr.load(&p).await.unwrap_or_else(|_| cowen_common::config::Config::default_with_profile(&p));
                info!("Auto-starting worker for profile: {}", p);
                if let Err(e) = daemon_svc.start_daemon(&p, &p_cfg).await {
                    error!("Failed to auto-start worker for profile {}: {}", p, e);
                }
            }
        }
    }

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let svc = daemon_svc.clone();
                        let v = vault.clone();
                        let c = cfg_mgr.clone();
                        let exp_token = ipc_token.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, svc, v, c, exp_token).await {
                                error!("Connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Accept error: {}", e);
                    }
                }
            }
            _ = stop_rx.recv() => {
                info!("Shutdown signal received, initiating graceful shutdown...");
                let _ = monitor_shutdown_tx.send(());
                let stopped_file = cowen_common::config::get_app_dir().join("master_daemon.stopped");
                let _ = cowen_common::utils::secure_write(stopped_file, "1");
                break;
            }
        }
    }

    // Graceful shutdown: stop all workers and wait for drain
    use cowen_common::daemon::DaemonService;
    let _ = daemon_svc.stop_all().await;
    
    // Clean up PID file and UDS socket
    let _ = std::fs::remove_file(pid_file);
    let _ = std::fs::remove_file(&target_ipc_port_file);
    let _ = std::fs::remove_file(&target_ipc_token_file);
    info!("cowen-daemon shutdown complete.");
    
    Ok(())
}

async fn handle_connection(
    mut stream: TcpStream,
    svc: Arc<dyn DaemonService>,
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
    expected_token: String,
) -> Result<()> {
    let mut len_buf = [0u8; 4];
    if stream.read_exact(&mut len_buf).await.is_err() {
        return Ok(()); // Connection closed
    }
    
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await?;

    let envelope: cowen_common::ipc::IpcEnvelope = serde_json::from_slice(&payload)?;
    if envelope.token != expected_token {
        let res = DaemonResponse::Error { code: 401, message: "Unauthorized IPC".to_string() };
        let res_payload = serde_json::to_vec(&res)?;
        let res_len = res_payload.len() as u32;
        stream.write_all(&res_len.to_be_bytes()).await?;
        stream.write_all(&res_payload).await?;
        return Ok(());
    }
    
    let req = envelope.request;
    
    let res = match req {
        DaemonRequest::Ping => {
            DaemonResponse::Pong
        }
        DaemonRequest::StartWorker { profile, mut config, app_secret, certificate, encrypt_key } => {
            config.app_secret = app_secret;
            config.certificate = certificate;
            config.encrypt_key = encrypt_key;
            info!("StartWorker requested for {}", profile);
            match svc.start_daemon(&profile, &config).await {
                Ok(_) => DaemonResponse::Success { message: format!("Worker {} started", profile) },
                Err(e) => DaemonResponse::Error { code: 500, message: e.to_string() }
            }
        }
        DaemonRequest::StopWorker { profile } => {
            info!("StopWorker requested for {}", profile);
            match svc.stop_daemon(&profile).await {
                Ok(_) => DaemonResponse::Success { message: format!("Worker {} stopped", profile) },
                Err(e) => DaemonResponse::Error { code: 500, message: e.to_string() }
            }
        }
        DaemonRequest::StopAllWorkers => {
            info!("StopAllWorkers requested");
            match svc.stop_all().await {
                Ok(_) => DaemonResponse::Success { message: "All workers stopped".to_string() },
                Err(e) => DaemonResponse::Error { code: 500, message: e.to_string() }
            }
        }
        DaemonRequest::ReloadWorker { profile } => {
            info!("ReloadWorker requested for {}", profile);
            match svc.reload_daemon(&profile).await {
                Ok(_) => DaemonResponse::Success { message: format!("Worker {} reloaded", profile) },
                Err(e) => DaemonResponse::Error { code: 500, message: e.to_string() }
            }
        }
        DaemonRequest::GetStatus { .. } => {
            DaemonResponse::Status(std::collections::HashMap::new())
        }
        DaemonRequest::InitProfile {
            profile,
            app_key,
            app_secret,
            certificate,
            encrypt_key,
            webhook_target,
            openapi_url,
            stream_url,
            app_mode,
            proxy_port,
        } => {
            info!("InitProfile requested for {}", profile);
            let is_new = !cfg_mgr.exists(&profile).await;
            
            let mode_str = app_mode.unwrap_or_else(|| "self_built".to_string());
            let mode = match mode_str.parse::<cowen_common::models::AuthMode>() {
                Ok(m) => m,
                Err(e) => {
                    send_response(&mut stream, &DaemonResponse::Error { code: 400, message: e }).await?;
                    return Ok(());
                }
            };

            let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
            let provider = auth_cli.provider(&mode);

            if let Some(ak) = &app_key {
                if let Ok(Some(existing_profile)) = provider.find_conflicting_profile(ak, &cfg_mgr).await {
                    if existing_profile != profile {
                        let _ = cfg_mgr.set_default_profile(&existing_profile);
                        send_response(&mut stream, &DaemonResponse::Success { message: format!("CONFLICT_SWITCH:{}", existing_profile) }).await?;
                        return Ok(());
                    }
                }
            }

            let mut config = match cfg_mgr.load(&profile).await {
                Ok(c) => c,
                Err(_) => cowen_common::Config::default_with_profile(&profile),
            };
            config.app_mode = mode;

            let params = cowen_auth::provider::InitParams {
                app_key: app_key.clone(),
                app_secret: app_secret.clone(),
                certificate: certificate.clone(),
                encrypt_key: encrypt_key.clone(),
                webhook_target: webhook_target.clone(),
                openapi_url: openapi_url.clone(),
                stream_url: stream_url.clone(),
                proxy_port: proxy_port,
                auto_start: true,
                is_new,
            };

            let mut app_config = cfg_mgr.load_app_config().await.unwrap_or_default();
            if let Some(url) = &openapi_url {
                app_config.openapi_url = url.clone();
            }
            if let Some(url) = &stream_url {
                app_config.stream_url = url.clone();
            }
            let _ = cfg_mgr.save_app_config(&app_config).await;

            match provider.initialize(&profile, &mut config, vault.clone(), &cfg_mgr, params, Some(svc.clone())).await {
                Ok(_) => {
                    let _ = cfg_mgr.set_default_profile(&profile);
                    DaemonResponse::Success { message: format!("Profile {} initialized", profile) }
                }
                Err(e) => DaemonResponse::Error { code: 500, message: e.to_string() }
            }
        }
        DaemonRequest::CallApi { profile, method, path, data, force } => {
            info!("CallApi requested for profile={} method={} path={}", profile, method, path);
            let config = match cfg_mgr.load(&profile).await {
                Ok(c) => c,
                Err(e) => {
                    send_response(&mut stream, &DaemonResponse::Error { code: 404, message: format!("Profile not found: {}", e) }).await?;
                    return Ok(());
                }
            };
            let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
            if !auth_cli.supports_api_call(&config) {
                send_response(&mut stream, &DaemonResponse::Error { code: 400, message: format!("Auth mode {:?} does not support direct CLI API calls.", config.app_mode) }).await?;
                return Ok(());
            }

            let app_cfg = match cfg_mgr.load_app_config().await {
                Ok(c) => c,
                Err(e) => {
                    send_response(&mut stream, &DaemonResponse::Error { code: 500, message: e.to_string() }).await?;
                    return Ok(());
                }
            };

            let body_option = if data.is_none() || data.as_ref().unwrap().trim() == "{}" || data.as_ref().unwrap().trim().is_empty() {
                None
            } else {
                data.clone()
            };

            let method_upper = method.to_uppercase();

            if !force {
                let spec = match auth_cli.get_openapi_spec(&profile, &config, false).await {
                    Ok(s) => s,
                    Err(e) => {
                        send_response(&mut stream, &DaemonResponse::Error { code: 500, message: e.to_string() }).await?;
                        return Ok(());
                    }
                };
                if let Err(e) = cowen_common::openapi::validate_request(&spec, &method_upper, &path, &body_option) {
                    send_response(&mut stream, &DaemonResponse::Error { code: 400, message: format!("OpenAPI validation failed: {}", e) }).await?;
                    return Ok(());
                }
                let path_no_query = path.split('?').next().unwrap_or(&path);
                if !cowen_auth::client::is_path_in_whitelist(path_no_query, &spec) {
                    send_response(&mut stream, &DaemonResponse::Error { code: 403, message: format!("CLI Rejected: Target path {} is not in the OpenAPI whitelist.", path_no_query) }).await?;
                    return Ok(());
                }
            }

            if path.starts_with("http") && !path.starts_with(&app_cfg.openapi_url) {
                send_response(&mut stream, &DaemonResponse::Error { code: 403, message: "CLI Security Block: Absolute external URLs are not allowed.".to_string() }).await?;
                return Ok(());
            }

            let token = match auth_cli.get_token(&profile, &config, &reqwest::header::HeaderMap::new()).await {
                Ok(t) => t,
                Err(e) => {
                    send_response(&mut stream, &DaemonResponse::Error { code: 500, message: format!("Failed to get token: {}", e) }).await?;
                    return Ok(());
                }
            };

            let ua = cowen_infra::get_user_agent("0.4.0");
            let client = match cowen_infra::create_client(&ua) {
                Ok(c) => c,
                Err(e) => {
                    send_response(&mut stream, &DaemonResponse::Error { code: 500, message: e.to_string() }).await?;
                    return Ok(());
                }
            };
            let url = if path.starts_with("http") {
                path.to_string()
            } else {
                let base = app_cfg.openapi_url.trim_end_matches('/');
                format!("{}{}", base, path)
            };

            let method_enum = match reqwest::Method::from_bytes(method_upper.as_bytes()) {
                Ok(m) => m,
                Err(_) => {
                    send_response(&mut stream, &DaemonResponse::Error { code: 400, message: format!("Invalid HTTP method: {}", method_upper) }).await?;
                    return Ok(());
                }
            };

            let mut req = client.request(method_enum, &url)
                .header("openToken", token.value)
                .header("appKey", config.app_key.trim());

            if let Some(b) = body_option {
                let json_body: serde_json::Value = match serde_json::from_str(&b) {
                    Ok(j) => j,
                    Err(e) => {
                        send_response(&mut stream, &DaemonResponse::Error { code: 400, message: format!("Invalid JSON payload: {}", e) }).await?;
                        return Ok(());
                    }
                };
                req = req.json(&json_body);
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let mut headers_map = std::collections::HashMap::new();
                    for (k, v) in resp.headers().iter() {
                        if let Ok(v_str) = v.to_str() {
                            headers_map.insert(k.to_string(), v_str.to_string());
                        }
                    }
                    let body = resp.text().await.unwrap_or_default();
                    DaemonResponse::ApiResponse(ApiResponseDto {
                        status,
                        headers: headers_map,
                        body,
                    })
                }
                Err(e) => DaemonResponse::Error { code: 520, message: format!("Request failed: {}", e) }
            }
        }
        DaemonRequest::GetAuthUrl { profile, force } => {
            info!("GetAuthUrl requested for profile={}, force={}", profile, force);
            let mut config = match cfg_mgr.load(&profile).await {
                Ok(c) => c,
                Err(e) => {
                    send_response(&mut stream, &DaemonResponse::Error { code: 404, message: format!("Profile not found: {}", e) }).await?;
                    return Ok(());
                }
            };

            let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
            let provider = auth_cli.provider(&config.app_mode);
            if let Err(e) = provider.hydrate_config(&profile, &mut config, vault.clone()).await {
                tracing::warn!("Failed to hydrate config for profile {}: {}", profile, e);
            }

            // If not forced and it's oauth2, try to rotate if refresh token is valid
            if !force && config.app_mode == cowen_common::models::AuthMode::Oauth2 {
                if let Ok(rt) = vault.get_refresh_token(&profile).await {
                    if !rt.is_expired() {
                        info!("Found valid refresh token for OAuth2, triggering rotation.");
                        let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
                        if let Ok(_token) = auth_cli.refresh_token(&profile, &config, &reqwest::header::HeaderMap::new()).await {
                            send_response(&mut stream, &DaemonResponse::AuthRotated).await?;
                            return Ok(());
                        }
                    }
                }
            }

            let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());

            // If it's SelfBuilt, just login directly
            if config.app_mode == cowen_common::models::AuthMode::SelfBuilt {
                match auth_cli.get_token(&profile, &config, &reqwest::header::HeaderMap::new()).await {
                    Ok(t) => {
                        send_response(&mut stream, &DaemonResponse::AuthSuccess { token: t.value }).await?;
                        return Ok(());
                    }
                    Err(e) => {
                        send_response(&mut stream, &DaemonResponse::Error { code: 401, message: format!("Self-built login failed: {}", e) }).await?;
                        return Ok(());
                    }
                }
            }

            let provider = auth_cli.provider(&config.app_mode);
            match provider.generate_auth_url(&profile, &mut config, vault.clone(), &cfg_mgr, cowen_auth::provider::InitParams {
                app_key: None,
                app_secret: None,
                certificate: None,
                encrypt_key: None,
                openapi_url: None,
                stream_url: None,
                webhook_target: None,
                proxy_port: None,
                auto_start: false,
                is_new: false,
            }).await {
                Ok((url, state)) => DaemonResponse::AuthUrl { url, state },
                Err(e) => DaemonResponse::Error { code: 500, message: format!("Generate auth url failed: {}", e) }
            }
        }
        DaemonRequest::WaitForAuth { profile, state } => {
            info!("WaitForAuth requested for profile={} state={}", profile, state);
            let config = match cfg_mgr.load(&profile).await {
                Ok(c) => c,
                Err(e) => {
                    send_response(&mut stream, &DaemonResponse::Error { code: 404, message: format!("Profile not found: {}", e) }).await?;
                    return Ok(());
                }
            };
            let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
            let provider = auth_cli.provider(&config.app_mode);
            match provider.wait_for_auth(&profile, &config, vault.clone(), &cfg_mgr, &state).await {
                Ok(_) => DaemonResponse::AuthSuccess { token: "Success".to_string() },
                Err(e) => DaemonResponse::Error { code: 500, message: format!("Wait for auth failed: {}", e) }
            }
        }
        DaemonRequest::GetToken { profile, refresh } => {
            info!("GetToken requested for profile={} refresh={}", profile, refresh);
            let config = match cfg_mgr.load(&profile).await {
                Ok(c) => c,
                Err(e) => {
                    send_response(&mut stream, &DaemonResponse::Error { code: 404, message: format!("Profile not found: {}", e) }).await?;
                    return Ok(());
                }
            };
            let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
            let res = if refresh {
                auth_cli.refresh_token(&profile, &config, &reqwest::header::HeaderMap::new()).await
            } else {
                auth_cli.get_token(&profile, &config, &reqwest::header::HeaderMap::new()).await
            };
            match res {
                Ok(t) => DaemonResponse::TokenData { token_json: serde_json::to_string(&t).unwrap_or_default() },
                Err(e) => DaemonResponse::Error { code: 500, message: format!("GetToken failed: {}", e) }
            }
        }
        DaemonRequest::ClearToken { profile } => {
            info!("ClearToken requested for profile={}", profile);
            let config = match cfg_mgr.load(&profile).await {
                Ok(c) => c,
                Err(e) => {
                    send_response(&mut stream, &DaemonResponse::Error { code: 404, message: format!("Profile not found: {}", e) }).await?;
                    return Ok(());
                }
            };
            let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
            match auth_cli.clear_token(&profile, &config).await {
                Ok(_) => DaemonResponse::Success { message: "Token cleared".to_string() },
                Err(e) => DaemonResponse::Error { code: 500, message: format!("ClearToken failed: {}", e) }
            }
        }
        DaemonRequest::Doctor { profile } => {
            info!("Doctor requested for profile {}", profile);
            
            let config = match cfg_mgr.load(&profile).await {
                Ok(c) => c,
                Err(e) => {
                    send_response(&mut stream, &DaemonResponse::Error { code: 404, message: format!("Profile not found: {}", e) }).await?;
                    return Ok(());
                }
            };
            
            let ctx = cowen_doctor::DoctorContext {
                profile: profile.clone(),
                config,
                verbose: false,
                fix: false,
                vault: vault.clone(),
                cfg_mgr: cfg_mgr.clone(),
            };

            let start_time = std::time::Instant::now();
            let results = match cowen_doctor::run_all_diagnostics(&ctx).await {
                Ok(res) => res,
                Err(e) => {
                    send_response(&mut stream, &DaemonResponse::Error { code: 500, message: format!("Diagnostics failed: {}", e) }).await?;
                    return Ok(());
                }
            };
            let duration = start_time.elapsed().as_millis();

            let mut report = String::new();
            let mut all_ok = true;
            for (i, res) in results.iter().enumerate() {
                let status_str = match &res.status {
                    cowen_doctor::DiagnosticStatus::Ok => "\x1b[32mOK\x1b[0m".to_string(),
                    cowen_doctor::DiagnosticStatus::Warning(w) => {
                        all_ok = false;
                        format!("\x1b[33mWARNING\x1b[0m ({})", w)
                    }
                    cowen_doctor::DiagnosticStatus::Error(e) => {
                        all_ok = false;
                        format!("\x1b[31mERROR\x1b[0m ({})", e)
                    }
                    cowen_doctor::DiagnosticStatus::Fixed(f) => format!("\x1b[32mFIXED\x1b[0m ({})", f),
                };
                report.push_str(&format!(
                    "\x1b[2m•\x1b[0m [{}] \x1b[1m{:<20}\x1b[0m {} ({}ms)\n",
                    i + 1,
                    res.name,
                    status_str,
                    res.duration_ms
                ));
            }

            report.push_str(&format!("\n\x1b[2m{}\x1b[0m\n", "=".repeat(60)));
            report.push_str(&format!("诊断总耗时: {}ms\n", duration));
            if all_ok {
                report.push_str(&format!("\x1b[1m✅\x1b[0m \x1b[1;32m诊断完成，环境运行状况良好。\x1b[0m\n"));
            } else {
                report.push_str(&format!("\x1b[1m⚠️\x1b[0m \x1b[1;33m诊断发现部分问题，建议运行 'cowen events' 查询详情。\x1b[0m\n"));
            }

            DaemonResponse::DoctorReport { report }
        }
        DaemonRequest::GetGlobalConfig => {
            info!("GetGlobalConfig requested");
            DaemonResponse::ConfigData {
                config_json: "{}".to_string(),
            }
        }
        DaemonRequest::SetGlobalConfig { key, value } => {
            info!("SetGlobalConfig requested for {}={}", key, value);
            DaemonResponse::Success {
                message: "Global config set".to_string(),
            }
        }

        DaemonRequest::SystemStatus { profile, all } => {
            let mut results = Vec::new();
            let list = cfg_mgr.list_profiles().await.unwrap_or_default();
            
            let profiles = if all {
                list
            } else {
                vec![profile.clone()]
            };
            
            if !profiles.is_empty() {
                for prof in profiles {
                let mut entries = Vec::new();
                let config = match cfg_mgr.load(&prof).await {
                    Ok(c) => c,
                    Err(_) => {
                        let mut c = cowen_common::config::Config::default_with_profile(&prof);
                        c.apply_env_overrides();
                        c
                    },
                };
                
                if !cfg_mgr.exists(&prof).await && config.app_key.is_empty() && config.app_secret.is_empty() {
                    continue;
                }
                let app_config = match cfg_mgr.load_app_config().await {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                
                let ctx = cowen_common::status::StatusContext {
                    profile: prof.clone(),
                    config: &config,
                    app_config: &app_config,
                    vault: vault.clone(),
                };
                
                // Add Configuration Status Entry
                let mode_str = format!("{:?}", config.app_mode).to_lowercase();
                let mut details = vec![];
                details.push(format!("Build ID:   {}", cowen_common::BUILD_ID));
                details.push(format!("Build Time: {}", cowen_common::BUILD_TIME));
                details.push(format!("OpenAPI:    {}", app_config.openapi_url));
                details.push(format!("Stream:     {}", app_config.stream_url));

                let ak_level = if config.app_key.trim().is_empty() {
                    cowen_common::status::StatusLevel::ERROR
                } else {
                    cowen_common::status::StatusLevel::OK
                };
                let ak_msg = if ak_level == cowen_common::status::StatusLevel::OK {
                    format!("AppKey: {} (Mode: {})", config.app_key, mode_str)
                } else {
                    "AppKey is missing".to_string()
                };

                let config_entry = cowen_common::status::StatusEntry {
                    name: "Configuration".to_string(),
                    icon: "⚙️".to_string(),
                    level: ak_level,
                    message: ak_msg,
                    reason: if ak_level == cowen_common::status::StatusLevel::ERROR {
                        Some("AppKey is missing".to_string())
                    } else {
                        None
                    },
                    details,
                    children: vec![],
                };
                entries.push(config_entry);

                let daemon_entry = cowen_common::status::collect_daemon_status(&ctx, "Daemon", "Tips", true, None).await;
                if let Ok(e) = daemon_entry {
                    entries.push(e);
                }
                
                let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
                if let Ok(mut diag_entries) = auth_cli.get_diagnostics(&ctx).await {
                    entries.append(&mut diag_entries);
                }
                
                let entry_val = serde_json::json!({
                    "profile": prof,
                    "entries": entries,
                });
                results.push(entry_val);
            }
            }
            
            let json = serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string());
            DaemonResponse::SystemStatusData { json }
        }
        DaemonRequest::SystemReset { profile, dry_run } => {
            if dry_run {
                use cowen_common::reset::ResetTask;
                let app_dir = cowen_common::config::get_app_dir();
                let config_task = cowen_config::reset::ConfigResetTask::new(app_dir.clone(), profile.clone());
                let telemetry_task = cowen_monitor::reset::TelemetryResetTask::new(app_dir.clone(), profile.clone());
                let storage_task = cowen_store::reset::StorageResetTask::new(app_dir.clone(), profile.clone());
                
                let mut out = String::new();
                out.push_str("🔍 [DRY RUN] Reset Execution Plan:\n");
                
                out.push_str(&format!("\n  📦 Module: {}\n", config_task.name()));
                out.push_str(&format!("  ℹ️  {}\n", config_task.description()));
                if let Ok(actions) = config_task.dry_run().await {
                    if actions.is_empty() {
                        out.push_str("      - No actions to perform.\n");
                    } else {
                        for a in actions {
                            out.push_str(&format!("      - {}\n", a));
                        }
                    }
                }
                
                out.push_str(&format!("\n  📦 Module: {}\n", telemetry_task.name()));
                out.push_str(&format!("  ℹ️  {}\n", telemetry_task.description()));
                if let Ok(actions) = telemetry_task.dry_run().await {
                    if actions.is_empty() {
                        out.push_str("      - No actions to perform.\n");
                    } else {
                        for a in actions {
                            out.push_str(&format!("      - {}\n", a));
                        }
                    }
                }
                
                out.push_str(&format!("\n  📦 Module: {}\n", storage_task.name()));
                out.push_str(&format!("  ℹ️  {}\n", storage_task.description()));
                if let Ok(actions) = storage_task.dry_run().await {
                    if actions.is_empty() {
                        out.push_str("      - No actions to perform.\n");
                    } else {
                        for a in actions {
                            out.push_str(&format!("      - {}\n", a));
                        }
                    }
                }
                
                out.push_str("\n💡 This is a dry run. No actual changes were made.");
                DaemonResponse::Success { message: out }
            } else {
                if let Some(prof) = profile {
                    tracing::info!("SystemReset: resetting profile {}", prof);
                    let _ = svc.stop_daemon(&prof).await;
                    if let Ok(config) = cfg_mgr.load(&prof).await {
                        let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
                        let _ = auth_cli.clear_token(&prof, &config).await;
                    }
                    if let Err(e) = cfg_mgr.delete(&prof).await {
                        tracing::error!("Failed to delete profile {}: {:?}", prof, e);
                    }
                    
                    // Actually execute telemetry and config reset
                    use cowen_common::reset::ResetTask;
                    let app_dir = cowen_common::config::get_app_dir();
                    let config_task = cowen_config::reset::ConfigResetTask::new(app_dir.clone(), Some(prof.clone()));
                    let _ = config_task.execute().await;
                    let telemetry_task = cowen_monitor::reset::TelemetryResetTask::new(app_dir.clone(), Some(prof.clone()));
                    let _ = telemetry_task.execute().await;
                } else {
                    tracing::info!("SystemReset: resetting all profiles");
                    let _ = svc.stop_all().await;
                    if let Ok(profiles) = cfg_mgr.list_profiles().await {
                        for prof in profiles {
                            if let Ok(config) = cfg_mgr.load(&prof).await {
                                let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
                                let _ = auth_cli.clear_token(&prof, &config).await;
                            }
                            tracing::info!("Deleting profile {}", prof);
                            if let Err(e) = cfg_mgr.delete(&prof).await {
                                tracing::error!("Failed to delete profile {}: {:?}", prof, e);
                            }
                        }
                    }
                    let _ = cfg_mgr.set_default_profile("default");
                    
                    // Actually execute global telemetry, config, and storage reset
                    use cowen_common::reset::ResetTask;
                    let app_dir = cowen_common::config::get_app_dir();
                    let config_task = cowen_config::reset::ConfigResetTask::new(app_dir.clone(), None);
                    let _ = config_task.execute().await;
                    let telemetry_task = cowen_monitor::reset::TelemetryResetTask::new(app_dir.clone(), None);
                    let _ = telemetry_task.execute().await;
                    let storage_task = cowen_store::reset::StorageResetTask::new(app_dir.clone(), None);
                    let _ = storage_task.execute().await;
                }
                DaemonResponse::Success { message: "Reset complete".to_string() }
            }
        }
        DaemonRequest::RenameProfile { old_name, new_name } => {
            match cfg_mgr.rename(&old_name, &new_name).await {
                Ok(_) => DaemonResponse::Success { message: format!("Profile '{}' renamed to '{}'", old_name, new_name) },
                Err(e) => DaemonResponse::Error { code: 500, message: e.to_string() },
            }
        }
        DaemonRequest::DlqList { profile, page, page_size } => {
            let offset = if page > 0 { (page - 1) * page_size } else { 0 };
            match vault.list_dlq_paged(&profile, offset, page_size).await {
                Ok(messages) => {
                    match serde_json::to_string(&messages) {
                        Ok(json) => DaemonResponse::DlqData { json },
                        Err(e) => DaemonResponse::Error { code: 500, message: format!("Serialization error: {}", e) }
                    }
                }
                Err(e) => DaemonResponse::Error { code: 500, message: e.to_string() }
            }
        }
        DaemonRequest::DlqView { profile: _, id } => {
            if let Ok(id_val) = id.parse::<i64>() {
                match vault.get_dlq_by_id(id_val).await {
                    Ok(Some(msg)) => {
                        match serde_json::to_string(&msg) {
                            Ok(json) => DaemonResponse::DlqData { json },
                            Err(e) => DaemonResponse::Error { code: 500, message: format!("Serialization error: {}", e) }
                        }
                    }
                    Ok(None) => DaemonResponse::Error { code: 404, message: "DLQ entry not found".to_string() },
                    Err(e) => DaemonResponse::Error { code: 500, message: e.to_string() }
                }
            } else {
                DaemonResponse::Error { code: 400, message: "Invalid ID format".to_string() }
            }
        }
        DaemonRequest::DlqRetry { profile, id } => {
            if let Ok(id_val) = id.parse::<i64>() {
                match cfg_mgr.load(&profile).await {
                    Ok(config) => {
                        let app_cfg = cfg_mgr.load_app_config().await.unwrap_or_default();
                        match cowen_server::daemon::forwarder::Forwarder::new(&profile, config, &app_cfg, vault.clone()) {
                            Ok(forwarder) => {
                                match forwarder.retry_message(id_val).await {
                                    Ok(_) => DaemonResponse::Success { message: format!("Successfully retried DLQ message {}", id_val) },
                                    Err(e) => DaemonResponse::Error { code: 500, message: format!("Retry failed: {}", e) }
                                }
                            }
                            Err(e) => DaemonResponse::Error { code: 500, message: format!("Failed to create forwarder: {}", e) }
                        }
                    }
                    Err(e) => DaemonResponse::Error { code: 404, message: format!("Profile not found: {}", e) }
                }
            } else {
                DaemonResponse::Error { code: 400, message: "Invalid ID format".to_string() }
            }
        }
        DaemonRequest::DlqPurge { profile } => {
            match vault.list_all_dlq(&profile).await {
                Ok(messages) => {
                    let mut count = 0;
                    for msg in messages {
                        if let Some(id) = msg.id {
                            if vault.delete_dlq_by_id(id).await.is_ok() {
                                count += 1;
                            }
                        }
                    }
                    DaemonResponse::Success { message: format!("Purged {} DLQ messages for profile {}", count, profile) }
                }
                Err(e) => DaemonResponse::Error { code: 500, message: format!("Failed to list DLQ: {}", e) }
            }
        }
        DaemonRequest::TailAudit { profile, lines } => {
            match vault.list_audit(&profile, lines).await {
                Ok(entries) => {
                    let mut content = String::new();
                    // Audit entries are usually ordered by timestamp desc. We reverse them to print chronological tail.
                    for entry in entries.iter().rev() {
                        content.push_str(&format!("[{}] {}\n", entry.timestamp, entry.message));
                    }
                    if content.is_empty() {
                        content = "No audit logs found.".to_string();
                    }
                    DaemonResponse::AuditData { content }
                }
                Err(e) => DaemonResponse::Error { code: 500, message: format!("Failed to fetch audit logs: {}", e) }
            }
        }
        DaemonRequest::ApiList { profile, search, page, page_size, refresh } => {
            let config = match cfg_mgr.load(&profile).await {
                Ok(c) => c,
                Err(e) => {
                    send_response(&mut stream, &DaemonResponse::Error { code: 404, message: format!("Profile not found: {}", e) }).await?;
                    return Ok(());
                }
            };
            let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
            match auth_cli.get_openapi_spec(&profile, &config, refresh).await {
                Ok(spec) => {
                    let mut ops = Vec::new();
                    if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
                        for (path, methods) in paths {
                            if let Some(methods_obj) = methods.as_object() {
                                for (method, details) in methods_obj {
                                    let summary = details.get("summary").and_then(|v| v.as_str()).unwrap_or("");
                                    let id = format!("{} {}", method.to_uppercase(), path);
                                    ops.push(serde_json::json!({
                                        "id": id,
                                        "method": method.to_uppercase(),
                                        "path": path,
                                        "summary": summary
                                    }));
                                }
                            }
                        }
                    }
                    
                    let mut plugin_used = None;
                    if let Some(q) = search {
                        use cowen_search::SearchProvider;
                        use cowen_search::loader::SidecarSearchProvider;
                        
                        let mut providers: Vec<Box<dyn SearchProvider>> = vec![];
                        let app_cfg = cfg_mgr.load_app_config().await.unwrap_or_default();
                        
                        let plugin_dir = cowen_common::config::get_app_dir().join("plugins");
                        let plugins = cowen_sys::plugin::discover_plugins(&plugin_dir);
                        for p in plugins {
                            if let Ok(loader) = cowen_sys::plugin::PluginLoader::new(&p) {
                                if loader.supports_trait("SearchProvider") {
                                    // If app.yaml enables specific plugins, filter them
                                    if !app_cfg.plugins.is_empty() {
                                        if !app_cfg.plugins.contains(&loader.manifest().name) {
                                            continue;
                                        }
                                    }
                                    providers.push(Box::new(SidecarSearchProvider::new(
                                        &loader.manifest().name, 
                                        p.clone(), 
                                        "default".to_string()
                                    )));
                                }
                            }
                        }

                        if providers.is_empty() && !app_cfg.plugins.is_empty() {
                            cowen_common::ipc::DaemonResponse::Error { message: format!("No active plugin with name {} found", app_cfg.plugins[0]), code: 404 }
                        } else if let Some(provider) = providers.first() {
                            plugin_used = Some(provider.name().to_string());
                            
                            let mut docs = Vec::new();
                            for op in &ops {
                                docs.push(cowen_search::SearchDocument {
                                    id: op["id"].as_str().unwrap_or("").to_string(),
                                    summary: op["summary"].as_str().unwrap_or("").to_string(),
                                    description: op["summary"].as_str().unwrap_or("").to_string(),
                                    vector: vec![],
                                });
                            }
                            
                            provider.update_index(&docs);
                            let results = provider.search(&q, page_size);
                            
                            // Map search results back to original ops
                            let mut paged_ops = Vec::new();
                            for (_, res_doc) in results {
                                if let Some(op) = ops.iter().find(|o| o["id"].as_str().unwrap_or("") == res_doc.id) {
                                    paged_ops.push(op.clone());
                                }
                            }
                            
                            let total = paged_ops.len();
                            DaemonResponse::ApiListData { total, json: serde_json::to_string(&paged_ops).unwrap_or_default(), plugin_used }
                        } else {
                            ops.retain(|o| {
                                o["summary"].as_str().unwrap_or("").to_lowercase().contains(&q.to_lowercase()) 
                                || o["path"].as_str().unwrap_or("").to_lowercase().contains(&q.to_lowercase())
                            });
                            let total = ops.len();
                            DaemonResponse::ApiListData { total, json: serde_json::to_string(&ops).unwrap_or_default(), plugin_used }
                        }
                    } else {
                        let total = ops.len();
                        let start = (page.max(1) - 1) * page_size;
                        let page_ops: Vec<_> = ops.into_iter().skip(start).take(page_size).collect();
                        DaemonResponse::ApiListData { total, json: serde_json::to_string(&page_ops).unwrap_or_default(), plugin_used }
                    }
                }
                Err(e) => DaemonResponse::Error { code: 500, message: format!("Failed to fetch API list: {}", e) }
            }
        }
        DaemonRequest::ApiSpec { profile, method, path } => {
            let config = match cfg_mgr.load(&profile).await {
                Ok(c) => c,
                Err(e) => {
                    send_response(&mut stream, &DaemonResponse::Error { code: 404, message: format!("Profile not found: {}", e) }).await?;
                    return Ok(());
                }
            };
            let auth_cli = cowen_auth::create_auth_client_with_vault(vault.clone());
            match auth_cli.get_openapi_spec(&profile, &config, false).await {
                Ok(spec) => {
                    let method_lower = method.to_lowercase();
                    if let Some(op) = spec.get("paths").and_then(|p| p.get(&path)).and_then(|p| p.get(&method_lower)) {
                        let j = serde_json::to_string(op).unwrap_or_else(|_| "{}".to_string());
                        eprintln!("DAEMON DEBUG OP: {}", j);
                        DaemonResponse::ApiSpecData { json: j }
                    } else {
                        DaemonResponse::Error { code: 404, message: format!("API spec not found for {} {}", method, path) }
                    }
                }
                Err(e) => DaemonResponse::Error { code: 500, message: format!("Failed to fetch API spec: {}", e) }
            }
        }
    };

    send_response(&mut stream, &res).await
}

async fn send_response(stream: &mut TcpStream, res: &DaemonResponse) -> Result<()> {
    let res_payload = serde_json::to_vec(res)?;
    let res_len = res_payload.len() as u32;
    stream.write_all(&res_len.to_be_bytes()).await?;
    stream.write_all(&res_payload).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_rustls_crypto_provider_installed() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        assert!(
            rustls::crypto::CryptoProvider::get_default().is_some(),
            "Rustls CryptoProvider must be installed for secure connections to function!"
        );
    }
}

// --- Global Stop Channel for Windows SCM ---
// Obsolete service functions and static stop channels have been cleanly decoupled and physical isolated to cowen-infra/src/sys/windows.rs.


