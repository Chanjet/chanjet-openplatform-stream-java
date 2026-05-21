use anyhow::Result;
use clap::Parser;
use tracing::{info, error, Level};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

use cowen_common::ipc::{DaemonRequest, DaemonResponse, WorkerStateDto};
use cowen_common::daemon::DaemonService;
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
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let args = Args::parse();
    info!("Starting cowen-daemon...");

    #[cfg(unix)]
    {
        if args.uds.exists() {
            std::fs::remove_file(&args.uds)?;
        }

        let listener = UnixListener::bind(&args.uds)?;
        info!("Listening on UDS: {}", args.uds.display());

        // Set permissions to 0600 (owner only)
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&args.uds)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&args.uds, perms)?;

        let cfg_mgr = ConfigManager::new().expect("Failed to init ConfigManager");
        let daemon_svc = Arc::new(ServerDaemonService::new(cfg_mgr));

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let svc = daemon_svc.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, svc).await {
                            error!("Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                }
            }
        }
    }
    
    #[cfg(not(unix))]
    {
        anyhow::bail!("Non-Unix platforms are not yet supported for UDS IPC");
    }
}

#[cfg(unix)]
async fn handle_connection(mut stream: UnixStream, svc: Arc<ServerDaemonService>) -> Result<()> {
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
            // We need a dummy vault here, or the daemon service needs to instantiate the vault
            // For now, in v0.3.4, daemon service expects vault.
            // But we can let daemon service fetch the vault, or we just pass it from config.
            // Wait, ServerDaemonService::start_daemon requires `Arc<dyn Vault>`.
            // We can create the vault here.
            let app_dir = cowen_common::config::get_app_dir();
            let app_cfg = cowen_common::config::AppConfig::default(); // Simplified
            let fingerprint = cowen_common::security::get_machine_fingerprint().unwrap_or_default();
            match cowen_store::create_vault(&app_cfg, &app_dir, &fingerprint).await {
                Ok(vault) => {
                    match svc.start_daemon(&profile, &config, vault).await {
                        Ok(_) => DaemonResponse::Success { message: format!("Worker {} started", profile) },
                        Err(e) => DaemonResponse::Error { code: 500, message: e.to_string() }
                    }
                }
                Err(e) => DaemonResponse::Error { code: 500, message: format!("Vault init failed: {}", e) }
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

