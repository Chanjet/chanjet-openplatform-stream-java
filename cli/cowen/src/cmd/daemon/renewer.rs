use crate::core::config::Config;
use anyhow::Result;
use crate::auth::client::Client;
use crate::auth::{VaultTokenPool, AuthClient};
use crate::core::vault::Vault;
use std::sync::Arc;
use chrono::Utc;
use tokio::time::{sleep, Duration};

/// OAuth2 专用令牌续约引擎
/// 负责在后台静默监控 AccessToken 的生存期并在临期前主动刷新
pub async fn run(profile: &str, config: &Config, vault: Arc<dyn Vault>) -> Result<()> {
    let pool = Arc::new(VaultTokenPool::new(vault.clone()));
    let auth = AuthClient::new(pool.as_ref());

    tracing::info!(target: "sys", "Token Renewer engine initialized for OAuth2 profile: {}", profile);

    loop {
        tracing::info!(target: "sys", "Running token health check...");
        let mut refresh_performed = false;
        
        match auth.get_app_access_token(profile, config).await {
            Ok(token) => {
                let now = Utc::now();
                let expiry = token.expires_at;
                let remaining = expiry.signed_duration_since(now);
                
                // 提前 15 分钟进行刷新，确保平滑过渡
                if remaining < chrono::Duration::minutes(15) {
                    tracing::info!(target: "sys", "Token expires in {:?}. Proactively refreshing...", remaining);
                    match auth.refresh_app_access_token(profile, config).await {
                        Ok(_) => {
                            tracing::info!(target: "sys", "Proactive token refresh successful");
                            refresh_performed = true;
                            let _ = vault.delete(profile, "last_refresh_error");
                        }
                        Err(e) => {
                            tracing::error!(target: "sys", error = %e, "Proactive token refresh failed");
                            let _ = vault.set(profile, "last_refresh_error", &e.to_string());
                        }
                    }
                } else {
                    tracing::info!(target: "sys", "Token is healthy (expires at {}, remaining: {:?})", expiry, remaining);
                }
            }
            Err(e) => {
                tracing::warn!(target: "sys", error = %e, "Token lookup failed. Check vault integrity.");
            }
        }
        
        // 策略：刷新成功后依然 10 分钟检查一次，确保状态同步
        let sleep_secs = if refresh_performed { 600 } else { 600 };
        sleep(Duration::from_secs(sleep_secs)).await;
    }
}
