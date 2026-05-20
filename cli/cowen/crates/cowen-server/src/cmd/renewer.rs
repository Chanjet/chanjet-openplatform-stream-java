use cowen_common::config::Config;
use anyhow::Result;
use cowen_auth::client::Client;
use cowen_auth::VaultTokenPool;
use cowen_common::vault::Vault;
use std::sync::Arc;
use chrono::Utc;
use tokio::time::{sleep, Duration};


pub fn calculate_next_check_delay(expires_at: chrono::DateTime<chrono::Utc>, now: chrono::DateTime<chrono::Utc>) -> Duration {
    use rand::Rng;
    let remaining = expires_at.signed_duration_since(now).num_seconds();
    
    // Default to 30s if already expired or something is wrong
    if remaining <= 0 {
        return Duration::from_secs(30);
    }

    // Goal: Check at 80% of remaining lifetime
    let mut delay = (remaining as f64 * 0.8) as i64;
    
    // Clamp to [30, 3600]
    if delay < 30 { delay = 30; }
    if delay > 3600 { delay = 3600; }

    // Add jitter: ±60s (but don't go below 10s)
    let jitter = rand::thread_rng().gen_range(-60..60);
    delay = (delay + jitter).max(10);

    Duration::from_secs(delay as u64)
}

/// OAuth2 专用令牌续约引擎
/// 负责在后台静默监控 AccessToken 的生存期并在临期前主动刷新
pub async fn run(profile: &str, config: &Config, vault: Arc<dyn Vault>) -> Result<()> {
    let pool = Arc::new(VaultTokenPool::new(vault.clone()));
    let auth = cowen_auth::create_auth_client(pool.clone());

    tracing::info!(target: "sys", "Token Renewer engine initialized for OAuth2 profile: {}", profile);

    loop {
        tracing::info!(target: "sys", "Running token health check...");
        let next_delay; // Default
        
        match auth.get_app_access_token(profile, config).await {
            Ok(token) => {
                let now = Utc::now();
                let expiry = token.expires_at;
                let remaining = expiry.signed_duration_since(now);

                // 提前 15 分钟进行刷新，确保平滑过渡
                if token.is_expired_with_buffer(chrono::Duration::minutes(15)) {
                    tracing::info!(target: "sys", "Token expires in {:?}. Proactively refreshing...", remaining);
                    match auth.refresh_app_access_token(profile, config).await {
                        Ok(_) => {
                            tracing::info!(target: "sys", "Proactive token refresh successful");
                            let _ = vault.delete_config(profile, "last_refresh_error").await;
                            // After success, re-fetch or assume a long life. 
                            // Simplest: check again in 10 mins or calculate based on common life.
                            next_delay = Duration::from_secs(600);
                        }
                        Err(e) => {
                            tracing::error!(target: "sys", error = %e, "Proactive token refresh failed");
                            let _ = vault.set_config(profile, "last_refresh_error", &e.to_string()).await;
                            next_delay = Duration::from_secs(60); // Retry faster on error
                        }
                    }
                } else {
                    tracing::info!(target: "sys", "Token is healthy (expires at {}, remaining: {:?})", expiry, remaining);
                    next_delay = calculate_next_check_delay(expiry, now);
                }
            }
            Err(e) => {
                tracing::warn!(target: "sys", error = %e, "Token lookup failed. Check vault integrity.");
                next_delay = Duration::from_secs(300); // Check again in 5 mins
            }
        }
        
        tracing::info!(target: "sys", "Token renewer sleeping for {:?}...", next_delay);
        sleep(next_delay).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration as ChronoDuration, Utc};

    #[test]
    fn test_calculate_next_check_delay() {
        let now = Utc::now();

        // 1. Long life: 2 hours -> should be 3600s (clamped)
        let expiry = now + ChronoDuration::hours(2);
        let delay = calculate_next_check_delay(expiry, now);
        // 3600 * 0.8 = 2880? No, 2 hours = 7200s. 7200 * 0.8 = 5760. Clamped to 3600.
        // With jitter ±60, it should be between 3540 and 3660 (wait, 3600 + jitter might exceed 3600 if jitter is positive)
        // Current logic: delay = 3600, then add jitter.
        assert!(delay.as_secs() >= 3540 && delay.as_secs() <= 3660);

        // 2. Medium life: 20 mins (1200s) -> 1200 * 0.8 = 960s
        let expiry = now + ChronoDuration::minutes(20);
        let delay = calculate_next_check_delay(expiry, now);
        assert!(delay.as_secs() >= 900 && delay.as_secs() <= 1020);

        // 3. Short life: 30s -> 30 * 0.8 = 24 -> clamped to 30s
        let expiry = now + ChronoDuration::seconds(30);
        let delay = calculate_next_check_delay(expiry, now);
        assert!(delay.as_secs() >= 10 && delay.as_secs() <= 90); // 30 ± 60, min 10

        // 4. Expired -> 30s
        let expiry = now - ChronoDuration::seconds(10);
        let delay = calculate_next_check_delay(expiry, now);
        assert_eq!(delay.as_secs(), 30);
    }
}
