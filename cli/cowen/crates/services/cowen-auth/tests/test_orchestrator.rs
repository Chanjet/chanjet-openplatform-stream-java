use chrono::{Duration, Utc};
use cowen_auth::provider::oauth2::OAuth2Provider;
use cowen_auth::provider::AuthProvider;
use cowen_common::domain::*;
use cowen_common::models::Token;
use std::sync::Arc;

mod common;

#[tokio::test]
async fn test_auth_orchestrator_background_loop_auto_heals_token() {
    // 1. Setup initial mocked state
    let vault = Arc::new(common::MockVault::new());

    // Starting condition: Token is ALREADY expired (by 1 minute)
    let initial_now = Utc::now();
    vault
        .save_access_token(
            "p_orch",
            Token {
                value: "at_1".to_string(),
                expires_at: initial_now - Duration::minutes(1),
                created_at: initial_now,
            },
        )
        .await
        .unwrap();

    vault
        .save_refresh_token(
            "p_orch",
            Token {
                value: "rt_1".to_string(),
                expires_at: initial_now + Duration::days(7),
                created_at: initial_now,
            },
        )
        .await
        .unwrap();

    let pool = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));

    // We mock the HTTP sender to return new tokens when the refresh endpoint is hit.
    // The new token will expire in 1 hour.
    let sender = Arc::new(common::MockHttpSender::with_body(
        "{\"access_token\":\"at_2\",\"refresh_token\":\"rt_2\",\"expires_in\":3600}",
    ));

    let provider = Arc::new(OAuth2Provider::new(pool, sender));
    let config = cowen_common::Config::default_with_profile("p_orch");
    let provider_clone = provider.clone();
    let config_clone = config.clone();

    // 2. We skip tokio::time::pause() since `test-util` feature is missing
    // Instead, we just simulate the background loop checking an already expired token

    // 3. Spawn a background loop similar to what `bridge.rs` does, but with short sleep
    let loop_handle = tokio::spawn(async move {
        loop {
            // The daemon calls on_maintenance_tick to check and heal token if it's expired or nearing expiration
            let _ = provider_clone
                .on_maintenance_tick("p_orch", &config_clone)
                .await;
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    });

    // 4. Verify initial state
    let token = vault.get_access_token("p_orch").await.unwrap();
    assert_eq!(token.value, "at_1", "Token should still be at_1 initially");

    // 5. Wait for the background loop to trigger and heal the token
    // The loop sleeps for 50ms, so 200ms is plenty of time
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // 6. Verify that the loop detected the expiry and auto-healed the token
    let healed_token = vault.get_access_token("p_orch").await.unwrap();
    assert_eq!(
        healed_token.value, "at_2",
        "The background loop should have auto-healed and fetched at_2"
    );

    // Cleanup
    loop_handle.abort();
}
