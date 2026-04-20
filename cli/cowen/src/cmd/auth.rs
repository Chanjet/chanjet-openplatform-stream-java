use crate::core::config::Config;
use crate::auth::client::Client as AuthClientTrait;
use anyhow::Result;

pub async fn login(
    _profile: &str,
    cfg: &Config,
    auth_cli: &dyn AuthClientTrait,
    force: bool,
    finalize: Option<&str>,
) -> Result<()> {
    // 1. Finalizer Implementation (Background flow)
    if let Some(_session_id) = finalize {
        return finalize_login(_profile, cfg, auth_cli).await;
    }

    // 2. Regular Login flow based on AuthMode
    match cfg.app_mode {
        crate::auth::models::AuthMode::Oauth2 => {
            println!("🔄 [OAuth2] Attempting to refresh token pair for profile '{}'...", _profile);
            match auth_cli.refresh_app_access_token(_profile, cfg).await {
                Ok(_) => {
                    println!("✅ Success! OAuth2 Token Pair has been rotated.");
                    Ok(())
                }
                Err(e) => {
                    println!("❌ Refresh failed: {}", e);
                    println!("💡 Suggestion: If the session has expired, please run \x1b[33mowenc init\x1b[0m to re-authorize.");
                    Err(e)
                }
            }
        }
        crate::auth::models::AuthMode::SelfBuilt => {
            if force {
                println!("🔄 Force refresh requested. Attempting immediate Token refresh using existing Ticket...");
            } else {
                println!("📡 Checking current credentials for profile '{}'...", _profile);
            }

            // Attempt immediate refresh
            match auth_cli.refresh_app_access_token(_profile, cfg).await {
                Ok(_) => {
                    println!("✅ Success! AccessToken has been refreshed.");
                    Ok(())
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    if err_msg.contains("Missing app_ticket") {
                        println!("⚠️  Local AppTicket missing or expired. Requesting a new one...");
                    } else {
                        println!("⚠️  Refresh failed: {}", err_msg);
                    }
                    println!("📡 Triggering a fresh platform push...");
                    auth_cli.trigger_push(_profile, cfg, force).await?;
                    
                    println!("⏳ Waiting for platform (AppTicket) push...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    match auth_cli.refresh_app_access_token(_profile, cfg).await {
                        Ok(_) => {
                            println!("✅ Success! AccessToken obtained.");
                            Ok(())
                        }
                        Err(e) => Err(anyhow::anyhow!("Failed to obtain token: {}", e))
                    }
                }
            }
        }
    }
}

async fn finalize_login(profile: &str, cfg: &Config, auth_cli: &dyn AuthClientTrait) -> Result<()> {
    tracing::info!(target: "sys", profile = %profile, "Finalizer started for background auth");
    
    let _vault = crate::core::config::ConfigManager::new()?.load(profile).map(|_| ())?; // Ensure env is OK
    // Note: Re-acquiring vault via wrapper
    let fingerprint = crate::core::security::get_machine_fingerprint()?;
    let seal_path = crate::core::config::get_app_dir().join(".seal");
    let multi_vault = crate::core::vault::MultiVault::new(seal_path, &fingerprint)?;
    
    let token_pool = crate::auth::VaultTokenPool::new(std::sync::Arc::new(multi_vault));
    let session_manager = crate::auth::lifecycle::AuthSessionManager::new(&token_pool);

    // 1. Get Session
    let session = session_manager.get_session(profile)?;
    
    // 2. Start Listener
    let (actual_port, rx) = crate::auth::lifecycle::listener::OAuth2CallbackListener::start(session.redirect_port, profile.to_string()).await;
    tracing::info!(target: "sys", port = %actual_port, "Finalizer listening for callback");

    // 3. Wait for result with timeout
    tokio::select! {
        result = rx => {
            match result {
                Ok(inner_res) => {
                    match inner_res {
                        Ok(res) => {
                            tracing::info!(target: "sys", "Callback received, saving code...");
                            session_manager.save_code(profile, &res.code, &res.state)?;
                            
                            // Trigger exchange
                            match auth_cli.get_app_access_token(profile, cfg).await {
                                Ok(_) => {
                                    tracing::info!(target: "sys", "Token exchange successful");
                                    Ok(())
                                }
                                Err(e) => {
                                    tracing::error!(target: "sys", error = %e, "Token exchange failed");
                                    Err(e)
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(target: "sys", error = %e, "Authorization rejected by provider or invalid state");
                            Err(anyhow::anyhow!("Authorization failed: {}", e))
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(target: "sys", error = %e, "Finalizer channel dropped unexpectedly");
                    Err(anyhow::anyhow!("Internal listener error: {}", e))
                }
            }
        },
        _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
            tracing::error!(target: "sys", "Finalizer timed out waiting for callback");
            Err(anyhow::anyhow!("Timeout waiting for authorization (5 mins)"))
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
pub async fn logout(profile: &str, _cfg: &Config, auth_cli: &dyn AuthClientTrait) -> Result<()> {
    auth_cli.clear_token(profile).await?;
    println!("✅ Successfully logged out from profile '{}'.", profile);
    println!("💡 All session credentials (Tokens/Tickets) have been cleared.");
    Ok(())
}
