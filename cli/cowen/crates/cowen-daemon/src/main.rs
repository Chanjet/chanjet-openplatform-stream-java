use anyhow::Result;
use clap::Parser;
use tracing::{info, error};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tokio::net::{TcpListener, TcpStream};

use cowen_common::ipc::{DaemonRequest, DaemonResponse};
use cowen_common::daemon::DaemonService;
use cowen_common::vault::Vault;
use cowen_server::ServerDaemonService;
use cowen_config::ConfigManager;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// IPC port file path
    #[arg(long)]
    ipc_port_file: Option<PathBuf>,

    /// Run as Windows Service
    #[arg(long)]
    run_as_service: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    #[cfg(windows)]
    if args.run_as_service {
        return run_windows_service();
    }

    let app_dir = cowen_common::config::get_app_dir();
    let pid_file = app_dir.join("master_daemon.pid");

    let result = run_main(&pid_file, args.ipc_port_file.clone()).await;
    if let Err(e) = &result {
        // FATAL CRASH: Write LAST_ERROR to PID file so CLI can report it synchronously
        let current_pid = std::process::id();
        let error_msg = e.to_string().replace('\n', " ");
        let start_time = chrono::Utc::now().to_rfc3339();
        let _ = std::fs::write(&pid_file, format!("{}\nSTART_TIME={}\nLAST_ERROR={}\nBUILD_ID={}\nBUILD_TIME={}", current_pid, start_time, error_msg, cowen_common::BUILD_ID, cowen_common::BUILD_TIME));
    }
    result
}

async fn run_main(pid_file: &PathBuf, ipc_port_file: Option<PathBuf>) -> Result<()> {
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
    
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true).mode(0o600);
        if let Ok(mut f) = opts.open(&target_ipc_token_file) {
            use std::io::Write;
            let _ = f.write_all(ipc_token.as_bytes());
        }
    }
    #[cfg(not(unix))]
    {
        let _ = std::fs::write(&target_ipc_token_file, &ipc_token);
    }
    
    std::fs::write(&target_ipc_port_file, ipc_port.to_string())?;

    let current_pid = std::process::id();
    let start_time = chrono::Utc::now().to_rfc3339();
    let _ = std::fs::write(pid_file, format!("{}\nSTART_TIME={}\nBUILD_ID={}\nBUILD_TIME={}", current_pid, start_time, cowen_common::BUILD_ID, cowen_common::BUILD_TIME));

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

    if let (Ok(st), Ok(url)) = (std::env::var("COWEN_STORE_TYPE"), std::env::var("COWEN_DB_URL")) {
        app_cfg.storage.store = st;
        app_cfg.storage.db_url = Some(url);
    }

    let fingerprint = cowen_common::security::get_machine_fingerprint().unwrap_or_default();
    let vault = cowen_store::create_vault(&app_cfg, &app_dir, &fingerprint).await.map_err(|e| anyhow::anyhow!("Failed to init Vault: {}", e))?;

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
    info!("Listening on TCP IPC port: {}", ipc_port);
    
    let daemon_svc: Arc<dyn DaemonService> = Arc::new(ServerDaemonService::new(cfg_mgr.clone()));

    let mut m_port = app_cfg.monitor_port;
    let mut allow_fallback = false;
    if m_port == 0 {
        m_port = 1588;
        allow_fallback = true;
    }
    let m_server = cowen_monitor::MonitorServer::new(m_port, daemon_svc.clone(), Some(telemetry_db.clone()));
    let (port_tx, port_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        if let Err(e) = m_server.start(Some(port_tx), allow_fallback).await {
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

    if actual_m_port > 0 && actual_m_port != app_cfg.monitor_port {
        app_cfg.monitor_port = actual_m_port;
        let _ = cfg_mgr.save_app_config(&app_cfg).await;
    }

    let _ = std::fs::write(pid_file, format!("{}\nMONITOR_PORT={}\nSTART_TIME={}\nBUILD_ID={}\nBUILD_TIME={}", current_pid, actual_m_port, start_time, cowen_common::BUILD_ID, cowen_common::BUILD_TIME));

    // Signal-aware accept loop
    let (stop_tx, mut stop_rx) = tokio::sync::mpsc::channel(1);

    #[cfg(unix)]
    {
        let stop_tx_unix = stop_tx.clone();
        tokio::spawn(async move {
            let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
            sigterm.recv().await;
            let _ = stop_tx_unix.send(()).await;
        });
    }

    let stop_tx_ctrl_c = stop_tx.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = stop_tx_ctrl_c.send(()).await;
    });

    // We also support stop from Windows service control
    set_global_stop_channel(stop_tx.clone());

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let svc = daemon_svc.clone();
                        let v = vault.clone();
                        let exp_token = ipc_token.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, svc, v, exp_token).await {
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
                let stopped_file = cowen_common::config::get_app_dir().join("master_daemon.stopped");
                let _ = std::fs::write(stopped_file, "1");
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

async fn handle_connection(mut stream: TcpStream, svc: Arc<dyn DaemonService>, vault: Arc<dyn Vault>, expected_token: String) -> Result<()> {
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
    };

    let res_payload = serde_json::to_vec(&res)?;
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
static STOP_TX: std::sync::OnceLock<tokio::sync::mpsc::Sender<()>> = std::sync::OnceLock::new();

fn set_global_stop_channel(tx: tokio::sync::mpsc::Sender<()>) {
    let _ = STOP_TX.set(tx);
}

#[allow(dead_code)]
fn trigger_global_stop() {
    if let Some(tx) = STOP_TX.get() {
        let _ = tx.blocking_send(());
    }
}

// --- Windows Service Entry Point ---
#[cfg(windows)]
fn run_windows_service() -> Result<()> {
    use std::ffi::OsString;
    use windows_service::service_dispatcher;

    let res = service_dispatcher::start("CowenDaemon", ffi_service_main);
    if let Err(e) = res {
        tracing::error!("Failed to start Windows Service dispatcher: {}", e);
        return Err(anyhow::anyhow!("Service dispatcher error: {}", e));
    }
    Ok(())
}

#[cfg(not(windows))]
#[allow(dead_code)]
fn run_windows_service() -> Result<()> {
    anyhow::bail!("Windows Service is not supported on non-Windows platforms.");
}

#[cfg(windows)]
fn ffi_service_main(arguments: Vec<std::ffi::OsString>) {
    if let Err(e) = run_service(arguments) {
        tracing::error!("Windows Service error: {}", e);
    }
}

#[cfg(windows)]
fn run_service(_arguments: Vec<std::ffi::OsString>) -> Result<()> {
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::service::ServiceControl;

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                trigger_global_stop();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register("CowenDaemon", event_handler)?;
    
    use windows_service::service::{ServiceState, ServiceStatus, ServiceType};
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: windows_service::service::ServiceControlAccept::STOP,
        exit_code: windows_service::service::ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    let app_dir = cowen_common::config::get_app_dir();
    let pid_file = app_dir.join("master_daemon.pid");

    // Run the main async daemon
    let rt = tokio::runtime::Runtime::new()?;
    let _ = rt.block_on(run_main(&pid_file, None));

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: windows_service::service::ServiceControlAccept::empty(),
        exit_code: windows_service::service::ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    Ok(())
}
