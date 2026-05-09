use cowen_common::Config;
use cowen_auth::client::Client as AuthClientTrait;
use anyhow::Result;

pub async fn login(
    profile: &str,
    cfg: &Config,
    auth_cli: &dyn AuthClientTrait,
    force: bool,
    finalize: Option<&str>,
) -> Result<()> {
    auth_cli.perform_login(profile, cfg, force, finalize).await
}

pub async fn token(
    _profile: &str,
    config: &Config,
    auth_cli: &dyn AuthClientTrait,
    format: &str,
    force_refresh: bool,
) -> Result<()> {
    let detail = if force_refresh {
        auth_cli.refresh_app_access_token(_profile, config).await
    } else {
        auth_cli.get_app_access_token(_profile, config).await
    };
    
    if format == "json" || format == "yaml" {
        match detail {
            Ok(t) => return cowen_common::utils::render(&t, format),
            Err(e) => return cowen_common::utils::render(&serde_json::json!({"error": e.to_string()}), format),
        }
    }

    // Attempt to get token (from pool/vault first)
    match detail {
        Ok(t) => {
            println!("Token status for profile '{}':", _profile);
            println!("  Value:      {}", cowen_common::utils::mask_string(&t.value));
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
pub async fn logout(profile: &str, _cfg: &Config, auth_cli: &dyn AuthClientTrait) -> Result<()> {
    auth_cli.clear_token(profile, _cfg).await?;
    println!("✅ Successfully logged out from profile '{}'.", profile);
    println!("💡 All session credentials (Tokens/Tickets) have been cleared.");
    Ok(())
}
