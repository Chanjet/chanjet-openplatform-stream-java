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
use crate::core::vault::{MultiVault, Vault};
use crate::core::security;
use crate::auth::{VaultTokenPool, AuthClient, pool::TokenPool, client::Client as AuthTrait};
use crate::auth::models::Ticket;
use chrono::Utc;

pub async fn start(profile: &str, config: &Config, proxy_port: u16, foreground: bool) -> Result<()> {
    let app_dir = crate::core::config::get_app_dir();
    let pid_file = app_dir.join(format!("{}_daemon.pid", profile));

    if !foreground {
        // Ensure logs directory exists
        let log_dir = app_dir.join("logs");
        fs::create_dir_all(&log_dir)?;
        let log_path = log_dir.join(format!("{}.log", profile));
        
        let log_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

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
            .stdout(Stdio::from(log_file.try_clone()?))
            .stderr(Stdio::from(log_file)) 
            .spawn()?;

        let pid = child.id();
        fs::write(&pid_file, pid.to_string())?;

        println!("🚀 Stream Bridge daemon started successfully!");
        println!("📡 Process running in background (PID: {}).", pid);
        println!("📝 Logs: {}", log_path.display());
        println!("💡 Use `tail -f {}` to monitor real-time activity.", log_path.display());
        
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

    let fingerprint = security::get_machine_fingerprint()?;
    let seal_path = app_dir.join(".seal");
    let vault: Arc<dyn Vault> = Arc::new(MultiVault::new(seal_path, &fingerprint)?);
    let pool = Arc::new(VaultTokenPool::new(vault.clone()));
    let auth_cli = AuthClient::new(pool.as_ref());

    let p_profile_proxy = profile.to_string();
    let p_config_proxy = config.clone();
    tokio::spawn(async move {
        if let Err(e) = start_proxy(&p_profile_proxy, &p_config_proxy, proxy_port).await {
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

        let p_pool = pool.clone();
        let p_profile = profile.to_string();
        let p_config = config.clone();
        
        dispatcher.on_app_ticket(move |msg| {
            let ticket_val = msg.biz_content.app_ticket.trim();
            let prefix = if ticket_val.len() > 8 { &ticket_val[..8] } else { ticket_val };
            println!("🎫 [Bridge] Received AppTicket: {}... (at {})", prefix, Utc::now());
            
            let ticket = Ticket {
                value: ticket_val.to_string(),
                created_at: Utc::now(),
            };
            
            if let Err(e) = p_pool.set_app_ticket(&p_profile, &ticket) {
                eprintln!("❌ Failed to save ticket to vault: {}", e);
            } else {
                println!("✅ [Bridge] AppTicket saved to vault correctly.");
                // If we get a ticket, immediately try to refresh the token
                let inner_pool = p_pool.clone();
                let inner_profile = p_profile.clone();
                let inner_config = p_config.clone();
                tokio::spawn(async move {
                    let auth = AuthClient::new(inner_pool.as_ref());
                    if let Err(e) = auth.get_app_access_token(&inner_profile, &inner_config).await {
                        eprintln!("[{}] ❌ Automatic token refresh failed: {}", Utc::now(), e);
                    } else {
                        println!("[{}] 🔑 [Bridge] AccessToken proactively refreshed.", Utc::now());
                    }
                });
            }
            true
        });

        dispatcher.on_ent_auth_code(|msg| {
            println!("🔑 [Bridge] Received TempAuthCode: {}", msg.biz_content.temp_auth_code);
            true
        });
    }

    client.start().await?;
    println!("🚀 [Bridge] Stream started. Waiting for connection...");
    
    // Brief delay to ensure WebSocket is connected before triggering push
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Proactive check: if ticket missing, trigger push
    match pool.get_app_ticket(profile) {
        Ok(_ticket) => {
            println!("🎫 [Bridge] Found existing AppTicket. Attempting proactive Token refresh...");
            let inner_pool = pool.clone();
            let inner_profile = profile.to_string();
            let inner_config = config.clone();
            tokio::spawn(async move {
                let auth = AuthClient::new(inner_pool.as_ref());
                match auth.get_app_access_token(&inner_profile, &inner_config).await {
                    Ok(token) => println!("🔑 [Bridge] AccessToken verified/refreshed: {}...", &token.value[..10]),
                    Err(e) => {
                        eprintln!("⚠️ [Bridge] Proactive refresh with existing ticket failed: {}. Triggering push check...", e);
                        if let Err(e) = auth.trigger_push(&inner_profile, &inner_config).await {
                            eprintln!("❌ [Bridge] Failed to trigger platform push: {}", e);
                        }
                    }
                }
            });
        }
        Err(_) => {
            println!("📡 [Bridge] No valid AppTicket found, triggering platform push...");
            if let Err(e) = auth_cli.trigger_push(profile, config).await {
                eprintln!("❌ [Bridge] Failed to trigger platform push: {}", e);
            }
        }
    }

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
    let app_dir = crate::core::config::get_app_dir();
    let pid_file = app_dir.join(format!("{}_daemon.pid", profile));
    
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
