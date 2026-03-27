use crate::core::config::Config;
use connector_sdk::{GatewayClient, ClientOptions};
use anyhow::Result;
use tokio::signal;
use std::process::{Command, Stdio};
use std::env;
use std::fs;
use crate::daemon::dlq::DlqStore;
use crate::daemon::forwarder::Forwarder;
use crate::daemon::proxy::start_proxy;
use std::sync::Arc;

pub async fn start(profile: &str, config: &Config, proxy_port: u16, foreground: bool) -> Result<()> {
    let home = directories::UserDirs::new().unwrap().home_dir().to_path_buf();
    let pid_file = home.join(".cjtc").join(format!("{}_daemon.pid", profile));

    if !foreground {
        // Spawn daemon process in background
        let exe = env::current_exe()?;
        
        let child = Command::new(exe)
            .arg("--profile")
            .arg(profile)
            .arg("daemon")
            .arg("start")
            .arg("--proxy-port")
            .arg(proxy_port.to_string())
            .arg("--foreground") // Prevent recursive spawn
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()) // Detach output completely
            .spawn()?;

        let pid = child.id();
        fs::write(&pid_file, pid.to_string())?;

        println!("🚀 Stream Bridge daemon started successfully!");
        println!("📡 Process running in background (PID: {}).", pid);
        println!("💡 Use `cjtc status` to check its health or `cjtc daemon stop` to terminate it.");
        
        return Ok(());
    }

    println!("🚀 Starting Stream Bridge for profile: {}...", profile);
    println!("📡 Listening for events and forwarding to :{}", proxy_port);

    let options = ClientOptions {
        app_key: config.app_key.clone(),
        app_secret: config.app_secret.clone(),
        encrypt_key: Some(config.encrypt_key.clone()),
        gateway_url: config.stream_url.clone(),
    };

    let client = GatewayClient::new(options);

    let p_profile = profile.to_string();
    let p_config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = start_proxy(&p_profile, &p_config, proxy_port).await {
            eprintln!("❌ Local Proxy Server error: {}", e);
        }
    });

    let dlq = Arc::new(DlqStore::new(profile)?);
    let forwarder = Forwarder::new(dlq, &config.webhook_target);

    {
        let d = client.dispatcher();
        let mut dispatcher = d.lock().unwrap();

        let fwd = forwarder.clone();
        dispatcher.set_fallback_handler(Arc::new(move |msg| {
            let fwd_clone = fwd.clone();
            tokio::spawn(async move {
                fwd_clone.forward(msg).await;
            });
            true
        }));

        dispatcher.on_app_ticket(|msg| {
            println!("🎫 [Bridge] Received AppTicket: {}", msg.biz_content.app_ticket);
            true
        });

        dispatcher.on_ent_auth_code(|msg| {
            println!("🔑 [Bridge] Received TempAuthCode: {}", msg.biz_content.temp_auth_code);
            true
        });
    }

    client.start().await?;

    let pid = std::process::id();
    // Write PID for foreground mode too, so stop command works
    let _ = fs::write(&pid_file, pid.to_string());

    println!("Press Ctrl+C to stop.");
    signal::ctrl_c().await?;
    
    client.stop();
    println!("Gracefully stopped.");

    let _ = fs::remove_file(pid_file);
    Ok(())
}

pub async fn stop(profile: &str) -> Result<()> {
    let home = directories::UserDirs::new().unwrap().home_dir().to_path_buf();
    let pid_file = home.join(".cjtc").join(format!("{}_daemon.pid", profile));
    
    if pid_file.exists() {
        if let Ok(pid_str) = fs::read_to_string(&pid_file) {
            let pid_str = pid_str.trim();
            println!("🛑 Stopping daemon (PID: {})...", pid_str);
            
            #[cfg(unix)]
            let _ = Command::new("kill").arg("-15").arg(pid_str).status();
            
            #[cfg(windows)]
            let _ = Command::new("taskkill").arg("/F").arg("/PID").arg(pid_str).status();
            
            let _ = fs::remove_file(&pid_file);
            println!("✅ Daemon stopped for profile '{}'.", profile);
            return Ok(());
        }
    }
    
    println!("⚠️ Daemon is not running for profile '{}'.", profile);
    Ok(())
}
