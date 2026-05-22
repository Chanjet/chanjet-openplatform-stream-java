use anyhow::Result;
use clap::Parser;
use tracing::{info, error};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

use cowen_common::ipc::{DaemonRequest, DaemonResponse};
use cowen_common::daemon::DaemonService;
use cowen_common::vault::Vault;
use cowen_server::ServerDaemonService;
use cowen_config::ConfigManager;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Unix Domain Socket path
    #[arg(long)]
    uds: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // CAPTURE PANICS: Ensure background crashes are recorded
    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload().downcast_ref::<&str>().cloned()
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("no message");
        tracing::error!("FATAL DAEMON PANIC: {}", payload);
    }));

    // We'll initialize tracing shortly after setting up the DB
    let args = Args::parse();

    #[cfg(unix)]
    {
        if args.uds.exists() {
            std::fs::remove_file(&args.uds)?;
        }

        let listener = UnixListener::bind(&args.uds)?;

        let app_dir = cowen_common::config::get_app_dir();
        let pid_file = app_dir.join("master_daemon.pid");
        let current_pid = std::process::id();
        let _ = std::fs::write(&pid_file, format!("{}\nBUILD_ID={}\nBUILD_TIME={}", current_pid, cowen_common::BUILD_ID, cowen_common::BUILD_TIME));

        // Set permissions to 0600 (owner only)
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&args.uds)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&args.uds, perms)?;

        let cfg_mgr = ConfigManager::new().expect("Failed to init ConfigManager");
        let app_dir = cowen_common::config::get_app_dir();
        let telemetry_db = Arc::new(
            cowen_monitor::telemetry_db::TelemetryDb::new(&app_dir.join("telemetry.db"))
                .await
                .expect("Failed to init telemetry db")
        );
        
        let _ = telemetry_db.run_gc().await;

        let mut app_cfg = if let Ok(content) = std::fs::read_to_string(app_dir.join("app.yaml")) {
            serde_yaml::from_str::<cowen_common::config::AppConfig>(&content).unwrap_or_default()
        } else {
            cowen_common::config::AppConfig::default()
        };

        if let (Ok(st), Ok(url)) = (std::env::var("COWEN_STORE_TYPE"), std::env::var("COWEN_DB_URL")) {
            app_cfg.storage.store = st;
            app_cfg.storage.db_url = Some(url);
        }

        let fingerprint = cowen_common::security::get_machine_fingerprint().unwrap_or_default();
        let vault = cowen_store::create_vault(&app_cfg, &app_dir, &fingerprint).await.expect("Failed to init Vault");

        use tracing_subscriber::prelude::*;
        let console_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")));
        
        let (_vault_tx, vault_rx) = tokio::sync::watch::channel(Some(vault.clone()));
        let vault_audit_layer = cowen_monitor::audit::VaultAuditLayer::new(vault_rx);

        tracing_subscriber::registry()
            .with(console_layer)
            .with(vault_audit_layer)
            .init();
            
        info!("Starting cowen-daemon...");
        info!("Listening on UDS: {}", args.uds.display());
        
        let daemon_svc = Arc::new(ServerDaemonService::new(cfg_mgr.clone(), Some(telemetry_db.clone())));

        let m_port = cfg_mgr.find_free_port().await;
        let m_server = cowen_monitor::MonitorServer::new(m_port, daemon_svc.clone());
        tokio::spawn(async move {
            if let Err(e) = m_server.start(None).await {
                tracing::error!("Monitor server error: {}", e);
            }
        });
        let _ = std::fs::write(&pid_file, format!("{}\nMONITOR_PORT={}\nBUILD_ID={}\nBUILD_TIME={}", current_pid, m_port, cowen_common::BUILD_ID, cowen_common::BUILD_TIME));

        // Signal-aware accept loop
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        
        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            let svc = daemon_svc.clone();
                            let v = vault.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(stream, svc, v).await {
                                    error!("Connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Accept error: {}", e);
                        }
                    }
                }
                _ = sigterm.recv() => {
                    info!("SIGTERM received, initiating graceful shutdown...");
                    break;
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Ctrl+C received, initiating graceful shutdown...");
                    break;
                }
            }
        }

        // Graceful shutdown: stop all workers and wait for drain
        use cowen_common::daemon::DaemonService;
        let _ = daemon_svc.stop_all().await;
        
        // Clean up PID file and UDS socket
        let _ = std::fs::remove_file(&pid_file);
        let _ = std::fs::remove_file(&args.uds);
        info!("cowen-daemon shutdown complete.");
    }
    
    #[cfg(not(unix))]
    {
        anyhow::bail!("Non-Unix platforms are not yet supported for UDS IPC");
    }

    #[cfg(unix)]
    Ok(())
}

#[cfg(unix)]
async fn handle_connection(mut stream: UnixStream, svc: Arc<ServerDaemonService>, vault: Arc<dyn Vault>) -> Result<()> {
    let mut len_buf = [0u8; 4];
    if stream.read_exact(&mut len_buf).await.is_err() {
        return Ok(()); // Connection closed
    }
    
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await?;

    let req: DaemonRequest = serde_json::from_slice(&payload)?;
    
    let res = match req {
        DaemonRequest::Ping => {
            DaemonResponse::Pong
        }
        DaemonRequest::StartWorker { profile, config } => {
            info!("StartWorker requested for {}", profile);
            match svc.start_daemon(&profile, &config, vault.clone()).await {
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
    };

    let res_payload = serde_json::to_vec(&res)?;
    let res_len = res_payload.len() as u32;
    stream.write_all(&res_len.to_be_bytes()).await?;
    stream.write_all(&res_payload).await?;

    Ok(())
}

