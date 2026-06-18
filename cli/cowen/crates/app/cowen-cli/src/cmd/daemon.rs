use anyhow::Result;
use std::process::Command;

async fn configure_proxy(
    ipc_client: &cowen_common::grpc::client::DaemonClient,
    profile: &str,
    proxy_port: Option<u16>,
    enable_proxy: Option<bool>,
) {
    if let Some(p) = proxy_port {
        let _ = ipc_client
            .set_config(profile, "proxy_port", &p.to_string())
            .await;
    }
    if let Some(e) = enable_proxy {
        let _ = ipc_client
            .set_config(profile, "proxy_enabled", if e { "true" } else { "false" })
            .await;
    }
}

async fn start_background(
    profile: &str,
    proxy_port: Option<u16>,
    enable_proxy: Option<bool>,
    all: bool,
    port_path: &str,
) -> Result<()> {
    let _ = cowen_common::grpc::client::DaemonClient::new(port_path)
        .ensure_daemon()
        .await?;
    let ipc_client = cowen_common::grpc::client::DaemonClient::new(port_path);

    configure_proxy(&ipc_client, profile, proxy_port, enable_proxy).await;

    if all {
        if let Err(e) = ipc_client.start_all().await {
            eprintln!("⚠️ Failed to send start_all command to daemon: {}", e);
        }
    } else if let Err(e) = ipc_client.start_daemon(profile).await {
        eprintln!("⚠️ Failed to send start command to daemon: {}", e);
    }
    println!("✅ Startup command sent to daemon.");
    Ok(())
}

fn resolve_daemon_path() -> std::path::PathBuf {
    let exe_dir = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let bin_name = cowen_sys::get_daemon_binary_name();
    let mut daemon_path = std::env::var("COWEN_DAEMON_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| exe_dir.join(bin_name));

    if !daemon_path.exists()
        && daemon_path
            .parent()
            .map(|p| p.ends_with("deps"))
            .unwrap_or(false)
    {
        if let Some(target_dir) = daemon_path.parent().and_then(|p| p.parent()) {
            daemon_path = target_dir.join(bin_name);
        }
    }
    daemon_path
}

async fn start_foreground(
    profile: &str,
    proxy_port: Option<u16>,
    enable_proxy: Option<bool>,
    all: bool,
    port_path: &str,
) -> Result<()> {
    let daemon_path = resolve_daemon_path();
    eprintln!("🔥 Trying to spawn daemon at: {:?}", daemon_path);
    eprintln!("🔥 Daemon exists? {}", daemon_path.exists());

    let mut child = Command::new(&daemon_path)
        .arg("--ipc-port-file")
        .arg(port_path)
        .spawn()
        .map_err(|e| {
            eprintln!("🔥 Error spawning daemon: {:?}", e);
            e
        })?;

    let child_id = child.id();
    eprintln!(
        "🚀 Starting cowen-daemon in foreground (PID: {})...",
        child_id
    );

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let ipc_client = cowen_common::grpc::client::DaemonClient::new(port_path);
    let mut retries = 30;
    while retries > 0 && ipc_client.ping().await.is_err() {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        retries -= 1;
    }

    configure_proxy(&ipc_client, profile, proxy_port, enable_proxy).await;

    let target_profiles = if all {
        vec![]
    } else {
        vec![profile.to_string()]
    };
    for p in target_profiles {
        if let Err(e) = ipc_client.start_daemon(&p).await {
            eprintln!("⚠️ Failed to send start command to daemon: {}", e);
        }
    }

    eprintln!("✅ Startup commands sent to foreground daemon. Blocking...");

    cowen_sys::handle_parent_signals_for_child(child_id);

    let status = child.wait()?;
    eprintln!("ℹ️ cowen-daemon exited with status: {}", status);
    Ok(())
}

pub async fn start(
    profile: &str,
    proxy_port: Option<u16>,
    enable_proxy: Option<bool>,
    foreground: bool,
    all: bool,
) -> Result<()> {
    let port_path = crate::get_ipc_port_path();

    if !foreground {
        start_background(
            profile,
            proxy_port,
            enable_proxy,
            all,
            &port_path.to_string_lossy(),
        )
        .await?;
    } else {
        start_foreground(
            profile,
            proxy_port,
            enable_proxy,
            all,
            &port_path.to_string_lossy(),
        )
        .await?;
    }

    Ok(())
}

pub async fn stop(profile: &str, all: bool) -> Result<()> {
    let port_path = crate::get_ipc_port_path();
    let ipc_client = cowen_common::grpc::client::DaemonClient::new(port_path);
    if ipc_client.ping().await.is_err() {
        eprintln!("✅ No running daemon found.");
        return Ok(());
    }
    if all {
        match ipc_client.stop_all().await {
            Ok(cowen_common::grpc::client::DaemonResponse::Success { message }) => {
                eprintln!("✅ {}", message)
            }
            Ok(cowen_common::grpc::client::DaemonResponse::Error { message, .. }) => {
                eprintln!("⚠️ Failed to stop all workers: {}", message)
            }
            Ok(_) => eprintln!("⚠️ Unexpected response type"),
            Err(e) => eprintln!("⚠️ IPC request failed: {}", e),
        }
    } else {
        match ipc_client.stop_daemon(profile).await {
            Ok(cowen_common::grpc::client::DaemonResponse::Success { message }) => {
                eprintln!("✅ {}", message)
            }
            Ok(cowen_common::grpc::client::DaemonResponse::Error { message, .. }) => {
                eprintln!("⚠️ Failed to stop profile {}: {}", profile, message)
            }
            Ok(_) => eprintln!("⚠️ Unexpected response type"),
            Err(e) => eprintln!("⚠️ IPC request failed: {}", e),
        }
    }
    Ok(())
}

pub async fn restart(
    profile: &str,
    proxy_port: Option<u16>,
    enable_proxy: Option<bool>,
    all: bool,
) -> Result<()> {
    stop(profile, all).await?;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    start(profile, proxy_port, enable_proxy, false, all).await
}

pub async fn service_install() -> Result<()> {
    let bin_name = cowen_common::utils::get_bin_name();
    let manager = cowen_sys::get_service_manager();

    let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
    let daemon_bin_name = cowen_sys::get_daemon_binary_name();
    let bin_path = std::env::var("COWEN_DAEMON_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| exe_dir.join(daemon_bin_name));
    let bin_path_str = bin_path.to_string_lossy();
    let app_dir = cowen_common::config::get_app_dir();
    let log_dir = app_dir.join("logs");
    let log_dir_str = log_dir.to_string_lossy();

    manager
        .install(&bin_name, &bin_path_str, &log_dir_str)
        .await?;
    Ok(())
}

pub async fn service_uninstall() -> Result<()> {
    let bin_name = cowen_common::utils::get_bin_name();
    let manager = cowen_sys::get_service_manager();
    manager.uninstall(&bin_name).await?;
    Ok(())
}

pub async fn service_status() -> Result<()> {
    let bin_name = cowen_common::utils::get_bin_name();
    let manager = cowen_sys::get_service_manager();
    let status_msg = manager.status(&bin_name).await?;
    println!("{}", status_msg);
    Ok(())
}
