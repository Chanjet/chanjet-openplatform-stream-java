use anyhow::Result;
use cowen_common::grpc::client::DaemonResponse;

pub async fn login(profile: &str, force: bool) -> Result<()> {
    let ipc = cowen_common::grpc::client::DaemonClient::new(crate::get_ipc_port_path());

    // 1. Get Auth URL
    println!("🔄 Requesting authorization session from Daemon...");
    match ipc.get_auth_url(profile, force).await {
        Ok(DaemonResponse::AuthRotated) => {
            println!("🔄 OAuth2 Token Pair has been rotated.");
        }
        Ok(DaemonResponse::AuthSuccess { .. }) => {
            println!("✅ Login successful! Token is active and ready.");
        }
        Ok(DaemonResponse::AuthUrl { url, state }) => {
            println!("\x1b[1mPlease authorize in the LOCAL browser of this machine. Opening URL...\x1b[0m");
            println!("\x1b[34m{}\x1b[0m", url);

            if std::env::var("COWEN_SKIP_BROWSER").unwrap_or_default() == "true" {
                println!("Browser mock triggered for URL: {}", url);
            } else if open::that(&url).is_err() {
                println!("\x1b[33m(Failed to open browser automatically.)\x1b[0m");
            }

            println!("\x1b[33m💡 Tip: If you are in an SSH or Headless environment:\x1b[0m");
            println!("\x1b[33m   1. Copy the URL above and open it in your local browser manually.\x1b[0m");
            println!("\x1b[33m   2. After authorization, your browser will redirect to a localhost URL.\x1b[0m");
            println!("\x1b[33m   3. Copy that redirected URL and run `curl \"<COPIED_URL>\"` in this terminal to complete the login.\x1b[0m");

            // 2. Wait for callback
            println!("\n\x1b[34m🚀 Daemon is listening for the callback. Waiting...\x1b[0m");
            match ipc.wait_for_auth(profile, &state).await {
                Ok(DaemonResponse::AuthSuccess { .. }) => {
                    println!("✅ Login successful!");
                }
                Ok(DaemonResponse::Error { message, .. }) => {
                    eprintln!("❌ Login failed: {}", message);
                    return Err(anyhow::anyhow!("Login failed: {}", message));
                }
                Err(e) => {
                    eprintln!("❌ IPC Error from daemon during wait_for_auth: {}", e);
                    return Err(e);
                }
                _ => {
                    eprintln!("❌ Unexpected response from daemon during wait_for_auth");
                    return Err(anyhow::anyhow!("Unexpected response"));
                }
            }
        }
        Ok(DaemonResponse::Error { message, .. }) => {
            eprintln!("❌ Failed to get auth URL: {}", message);
            eprintln!("💡 Ensure the cowen daemon is running (`cowen daemon start`).");
            return Err(anyhow::anyhow!("Failed to get auth URL: {}", message));
        }
        _ => {
            eprintln!("❌ Unexpected response from daemon");
            return Err(anyhow::anyhow!("Unexpected response"));
        }
    }

    Ok(())
}

pub async fn token(profile: &str, format: &str, refresh: bool) -> Result<()> {
    let ipc = cowen_common::grpc::client::DaemonClient::new(crate::get_ipc_port_path());
    match ipc.get_token(profile, refresh).await {
        Ok(DaemonResponse::TokenData { token_json }) => {
            if format != "text" {
                let val: serde_json::Value = serde_json::from_str(&token_json)?;
                cowen_common::utils::render(&val, format).map_err(|e| anyhow::anyhow!(e))?;
                return Ok(());
            }
            let t: cowen_common::models::Token = serde_json::from_str(&token_json)?;
            println!("\n🔑 Access Token Information");
            println!("--------------------------------------------------");
            println!("  Profile:    {}", profile);
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
                println!(
                    "\x1b[1;36m{}\x1b[0m",
                    cowen_common::utils::mask_string(&t.value)
                );
            }
            println!();
        }
        Ok(DaemonResponse::Error { message, .. }) => {
            eprintln!("❌ Failed to retrieve token: {}", message);
        }
        Err(e) => eprintln!("❌ IPC Error: {}", e),
        _ => eprintln!("❌ Unexpected response"),
    }
    Ok(())
}

pub async fn logout(profile: &str) -> Result<()> {
    let ipc = cowen_common::grpc::client::DaemonClient::new(crate::get_ipc_port_path());
    match ipc.clear_token(profile).await {
        Ok(DaemonResponse::Success { .. }) => {
            println!("✅ Successfully logged out from profile '{}'.", profile);
            println!("💡 All session credentials (Tokens/Tickets) have been cleared.");
        }
        Ok(DaemonResponse::Error { message, .. }) => {
            eprintln!("❌ Logout failed: {}", message);
            return Err(anyhow::anyhow!("Logout failed: {}", message));
        }
        Err(e) => {
            eprintln!("❌ IPC Error: {}", e);
            return Err(e);
        }
        _ => {
            eprintln!("❌ Unexpected response");
            return Err(anyhow::anyhow!("Unexpected response from daemon"));
        }
    }
    Ok(())
}
