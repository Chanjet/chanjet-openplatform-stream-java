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
        match auth_cli.refresh_app_access_token(_profile, cfg).await {
            Ok(_) => {
                println!("✅ Success! AccessToken has been refreshed and saved to Vault.");
                return Ok(());
            }
            Err(e) => {
                println!("⚠️  Immediate refresh failed (likely expired AppTicket): {}", e);
                println!("📡 Falling back to platform push...");
            }
        }
    }

    println!("📡 Triggering AppTicket resend for profile '{}'...", _profile);
    auth_cli.trigger_push(_profile, cfg).await?;
    println!("✅ Push request sent. Platform will push a new AppTicket via Stream Bridge.");
    println!("(TIP: Ensure 'owenc daemon start' is running to receive the push and auto-refresh)");
    Ok(())
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
