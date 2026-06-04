#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#![allow(unused_imports)]

mod controller;

use anyhow::Result;
use clap::Parser;
use tracing::{info, error};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tokio::net::{TcpListener, TcpStream};

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

    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload().downcast_ref::<&str>().cloned()
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("no message");
        eprintln!("FATAL DAEMON PANIC: {}", payload);
        tracing::error!("FATAL DAEMON PANIC: {}", payload);
    }));

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let ipc_port = listener.local_addr()?.port();
    
    let target_ipc_port_file = ipc_port_file.unwrap_or_else(|| cowen_common::config::get_app_dir().join("ipc.port"));
    let target_ipc_token_file = target_ipc_port_file.with_file_name("ipc.token");
    
    // Generate ephemeral JWT secret
    let jwt_secret = cowen_common::jwt::generate_ephemeral_secret();
    cowen_common::jwt::set_global_daemon_secret(jwt_secret.clone());
    
    // Generate Admin JWT for CLI to use
    let admin_claims = cowen_common::jwt::IpcClaims::new(
        "cli".to_string(), 
        cowen_common::jwt::IpcRole::Admin, 
        vec!["*".to_string()], 
        86400 * 365 * 10 // 10 years validity since it's just for this daemon lifetime
    );
    let ipc_token = cowen_common::jwt::sign_jwt(&admin_claims, &jwt_secret).unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
    
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
    let (stop_tx, stop_rx) = tokio::sync::mpsc::channel(1);

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
                info!("Auto-starting worker for profile: {}", p);
                if let Err(e) = daemon_svc.start_daemon(&p).await {
                    error!("Failed to auto-start worker for profile {}: {}", p, e);
                }
            }
        }
    }


    let controller = crate::controller::CowenDaemonController::new(daemon_svc.clone(), vault.clone(), cfg_mgr.clone());
    
    let secret_clone = jwt_secret.clone();
    let auth_interceptor = move |mut req: tonic::Request<()>| -> std::result::Result<tonic::Request<()>, tonic::Status> {
        match req.metadata().get("authorization") {
            Some(t) => {
                let token_str = t.to_str().unwrap_or("").replace("Bearer ", "");
                match cowen_common::jwt::verify_jwt(&token_str, &secret_clone) {
                    Ok(claims) => {
                        req.extensions_mut().insert(claims);
                        Ok(req)
                    }
                    Err(e) => Err(tonic::Status::unauthenticated(format!("Invalid token: {}", e))),
                }
            }
            None => Err(tonic::Status::unauthenticated("Missing authorization header")),
        }
    };

    let service_with_interceptor = cowen_common::grpc::proto::cowen_daemon_service_server::CowenDaemonServiceServer::with_interceptor(controller, auth_interceptor);

    let mut stop_rx_tonic = stop_rx;
    let monitor_tx = monitor_shutdown_tx;

    let server_future = tonic::transport::Server::builder()
        .add_service(service_with_interceptor)
        .serve_with_incoming_shutdown(tokio_stream::wrappers::TcpListenerStream::new(listener), async move {
            let _ = stop_rx_tonic.recv().await;
            info!("Shutdown signal received, initiating graceful shutdown...");
            let _ = monitor_tx.send(());
            let stopped_file = cowen_common::config::get_app_dir().join("master_daemon.stopped");
            let _ = cowen_common::utils::secure_write(stopped_file, "1");
        });

    if let Err(e) = server_future.await {
        error!("gRPC Server error: {}", e);
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
