#![allow(dead_code)]
use cowen_common::Config;
use cowen_auth::client::Client as AuthClientTrait;
use anyhow::Result;

pub async fn login(
    profile: &str,
    cfg: &Config,
    auth_cli: &dyn AuthClientTrait,
    force: bool,
    finalize: Option<&str>,
    daemon_service: Option<std::sync::Arc<dyn cowen_common::daemon::DaemonService>>,
) -> Result<()> {
    auth_cli.perform_login(profile, cfg, force, finalize, daemon_service).await.map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

pub async fn token(
    _profile: &str,
    config: &Config,
    auth_cli: &dyn AuthClientTrait,
    format: &str,
    refresh: bool,
) -> Result<()> {
    let res = if refresh {
        auth_cli.refresh_token(_profile, config, &reqwest::header::HeaderMap::new()).await
    } else {
        auth_cli.get_token(_profile, config, &reqwest::header::HeaderMap::new()).await
    };

    match res {
        Ok(t) => {
            if format != "text" {
                cowen_common::utils::render(&t, format).map_err(|e| anyhow::anyhow!(e))?;
                return Ok(());
            }
            println!("\n🔑 Access Token Information");
            println!("--------------------------------------------------");
            println!("  Profile:    {}", _profile);
            println!("  Identity:   {:?}", t.extract_identity());
            println!("  Expires At: {}", t.expires_at);
            if t.is_expired() {
                println!("  Status:     \x1b[31mExpired\x1b[0m");
            } else {
                println!("  Status:     \x1b[32mActive\x1b[0m");
            }
            println!("\nFull Token Value:");
            if std::env::var("COWEN_RAW_OUTPUT").unwrap_or_default() == "true" {
                println!("\x1b[1;36m{}\x1b[0m", t.value);
            } else {
                println!("\x1b[1;36m{}\x1b[0m", cowen_common::utils::mask_string(&t.value));
            }
            println!();
        }
        Err(e) => {
            if format != "text" {
                cowen_common::utils::render(&serde_json::json!({"error": e.to_string()}), format).map_err(|e| anyhow::anyhow!(e))?;
                return Ok(());
            }
            eprintln!("❌ Failed to retrieve token: {}", e);
        }
    }

    Ok(())
}

pub async fn logout(_profile: &str, _cfg: &Config, auth_cli: &dyn AuthClientTrait) -> Result<()> {
    auth_cli.clear_token(_profile, _cfg).await.map_err(|e| anyhow::anyhow!(e))?;
    println!("✅ Successfully logged out from profile '{}'.", _profile);
    println!("💡 All session credentials (Tokens/Tickets) have been cleared.");
    Ok(())
}
