
use anyhow::Result;
use std::process::Command;


pub async fn start(
    profile: &str,
    proxy_port: Option<u16>,
    enable_proxy: Option<bool>,
    foreground: bool,
    all: bool,
) -> Result<()> {
    let port_path = cowen_common::config::get_ipc_port_path();

    if !foreground {
        // Just ensure it's started and send start request
        let _ = cowen_common::grpc::client::DaemonClient::new(&port_path).ensure_daemon().await?;
        let ipc_client = cowen_common::grpc::client::DaemonClient::new(port_path);
        
        if let Some(p) = proxy_port {
            let _ = ipc_client.set_config(profile, "proxy_port", &p.to_string()).await;
        }
        if let Some(e) = enable_proxy {
            let _ = ipc_client.set_config(profile, "proxy_enabled", if e { "true" } else { "false" }).await;
        }

        if all {
            if let Err(e) = ipc_client.start_all().await {
                eprintln!("⚠️ Failed to send start_all command to daemon: {}", e);
            }
        } else {
            if let Err(e) = ipc_client.start_daemon(profile).await {
                eprintln!("⚠️ Failed to send start command to daemon: {}", e);
            }
        }
        println!("✅ Startup command sent to daemon.");
    } else {
        // Run in foreground
        let exe_dir = std::env::current_exe()?.parent().unwrap().to_path_buf();
        let bin_name = if cfg!(windows) { "cowen-daemon.exe" } else { "cowen-daemon" };
        let mut daemon_path = std::env::var("COWEN_DAEMON_BIN")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| exe_dir.join(bin_name));
            
        // If not found in the immediate parent directory (e.g., target/debug/deps during tests),
        // try the parent of the parent (e.g., target/debug).
        if !daemon_path.exists() && daemon_path.parent().map(|p| p.ends_with("deps")).unwrap_or(false) {
            if let Some(target_dir) = daemon_path.parent().and_then(|p| p.parent()) {
                daemon_path = target_dir.join(bin_name);
            }
        }
        
        eprintln!("🔥 Trying to spawn daemon at: {:?}", daemon_path);
        eprintln!("🔥 Daemon exists? {}", daemon_path.exists());
        
        let mut child = match Command::new(&daemon_path)
            .arg("--ipc-port-file")
            .arg(&port_path)
            .spawn() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("🔥 Error spawning daemon: {:?}", e);
                return Err(e.into());
            }
        };
        
        let child_id = child.id();
        eprintln!("🚀 Starting cowen-daemon in foreground (PID: {})...", child_id);
        
        // Wait for it to bind port
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        
        let ipc_client = cowen_common::grpc::client::DaemonClient::new(port_path.clone());
        
        // Wait for daemon to become available (up to 15s for heavily loaded CI)
        let mut retries = 30;
        while retries > 0 && ipc_client.ping().await.is_err() {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            retries -= 1;
        }

        if let Some(p) = proxy_port {
            let _ = ipc_client.set_config(profile, "proxy_port", &p.to_string()).await;
        }
        if let Some(e) = enable_proxy {
            let _ = ipc_client.set_config(profile, "proxy_enabled", if e { "true" } else { "false" }).await;
        }

        let target_profiles = if all { vec![] } else { vec![profile.to_string()] }; // if all, daemon already auto started
        for p in target_profiles {
            if let Err(e) = ipc_client.start_daemon(&p).await {
                eprintln!("⚠️ Failed to send start command to daemon: {}", e);
            }
        }
        
        eprintln!("✅ Startup commands sent to foreground daemon. Blocking...");

        #[cfg(unix)]
        {
            tokio::spawn(async move {
                use tokio::signal::unix::{signal, SignalKind};
                if let (Ok(mut sigterm), Ok(mut sigint)) = (signal(SignalKind::terminate()), signal(SignalKind::interrupt())) {
                    tokio::select! {
                        _ = sigterm.recv() => {
                            let _ = std::process::Command::new("kill").arg("-15").arg(child_id.to_string()).status();
                        }
                        _ = sigint.recv() => {
                            let _ = std::process::Command::new("kill").arg("-2").arg(child_id.to_string()).status();
                        }
                    }
                }
            });
        }

        let status = child.wait()?;
        eprintln!("ℹ️ cowen-daemon exited with status: {}", status);
    }

    Ok(())
}

pub async fn stop(profile: &str, all: bool) -> Result<()> {
    let port_path = cowen_common::config::get_ipc_port_path();
    if !port_path.exists() {
        eprintln!("✅ No running daemon found.");
        return Ok(());
    }

    let ipc_client = cowen_common::grpc::client::DaemonClient::new(port_path);
    if all {
        match ipc_client.stop_all().await {
            Ok(cowen_common::grpc::client::DaemonResponse::Success { message }) => eprintln!("✅ {}", message),
            Ok(cowen_common::grpc::client::DaemonResponse::Error { message, .. }) => eprintln!("⚠️ Failed to stop all workers: {}", message),
            Ok(_) => eprintln!("⚠️ Unexpected response type"),
            Err(e) => eprintln!("⚠️ IPC request failed: {}", e),
        }
    } else {
        match ipc_client.stop_daemon(profile).await {
            Ok(cowen_common::grpc::client::DaemonResponse::Success { message }) => eprintln!("✅ {}", message),
            Ok(cowen_common::grpc::client::DaemonResponse::Error { message, .. }) => eprintln!("⚠️ Failed to stop profile {}: {}", profile, message),
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
