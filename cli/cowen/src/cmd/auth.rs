use crate::core::config::Config;
use crate::auth::client::Client as AuthClientTrait;
use anyhow::Result;

pub async fn login(
    _profile: &str,
    cfg: &Config,
    auth_cli: &dyn AuthClientTrait,
    force: bool,
) -> Result<()> {
    if force {
        println!("🔄 Force refresh requested. Attempting immediate Token refresh using existing Ticket...");
    } else {
        println!("📡 Checking current credentials for profile '{}'...", _profile);
    }

    // 1. Attempt immediate refresh (it will loop for 30s internally if ticket is missing)
    match auth_cli.refresh_app_access_token(_profile, cfg).await {
        Ok(_) => {
            println!("✅ Success! AccessToken has been refreshed and saved to Vault.");
            return Ok(());
        }
        Err(e) => {
            let err_msg = e.to_string();
            if err_msg.contains("Missing app_ticket") {
                println!("⚠️  Local AppTicket missing or expired. Requesting a new one from platform...");
                // Note: perform_network_refresh already triggers a push on first attempt, 
                // but we can be explicit here if it returned error immediately (which it shouldn't per impl)
            } else {
                println!("⚠️  Refresh failed: {}", err_msg);
                println!("📡 Triggering a fresh platform push to recover...");
                auth_cli.trigger_push(_profile, cfg, force).await?;
            }
        }
    }

    // 2. If we reach here, the internal 30s loop in refresh_app_access_token might have failed,
    // or we are doing a manual retry. Let's give it one more guided wait.
    println!("⏳ Waiting for platform to push security handshake (AppTicket)...");
    println!("(TIP: Ensure the daemon is running to receive the push)");
    
    // We try one more time with a fresh wait
    match auth_cli.refresh_app_access_token(_profile, cfg).await {
        Ok(_) => {
            println!("✅ Success! AccessToken obtained.");
            Ok(())
        }
        Err(e) => {
            Err(anyhow::anyhow!("Failed to obtain token after waiting: {}. \nSuggestion: Check if 'owenc daemon start' is running and your network/firewall allows WebSocket connections to {}", e, cfg.stream_url))
        }
    }
}

pub async fn token(
    _profile: &str,
    config: &Config,
    auth_cli: &dyn AuthClientTrait,
    format: &str,
) -> Result<()> {
    let detail = auth_cli.get_app_access_token(_profile, config).await;
    
    if format == "json" || format == "yaml" {
        match detail {
            Ok(t) => return crate::core::utils::render(&t, format),
            Err(e) => return crate::core::utils::render(&serde_json::json!({"error": e.to_string()}), format),
        }
    }

    // Attempt to get token (from pool/vault first)
    match detail {
        Ok(t) => {
            println!("Token status for profile '{}':", _profile);
            println!("  Value:      {}", crate::core::utils::mask_string(&t.value));
            println!("  Expires At: {}", t.expires_at);
            if t.is_expired() {
                println!("  Status:     \x1b[31mExpired\x1b[0m");
            } else {
                println!("  Status:     \x1b[32mActive\x1b[0m");
            }
        }
        Err(e) => {
            println!("Token status for profile '{}': \x1b[31mNot Found or Error\x1b[0m", _profile);
            println!("  Reason: {}", e);
        }
    }
    Ok(())
}
