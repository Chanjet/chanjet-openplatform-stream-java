use crate::core::config::Config;
use crate::auth::client::Client as AuthClientTrait;
use anyhow::Result;

pub async fn login(
    _profile: &str,
    cfg: &Config,
    auth_cli: &dyn AuthClientTrait,
) -> Result<()> {
    println!("Triggering AppTicket resend for profile...");
    auth_cli.trigger_push(_profile, cfg).await?;
    println!("Success. Please check your daemon logs for the new AppTicket.");
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
