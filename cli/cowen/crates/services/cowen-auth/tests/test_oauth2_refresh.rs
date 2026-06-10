use chrono::{Duration, Utc};
use cowen_auth::provider::oauth2::OAuth2Provider;
use cowen_auth::provider::AuthProvider;
use cowen_common::domain::*;
use cowen_common::models::Token;
use std::sync::Arc;

mod common;

#[tokio::test]
async fn test_oauth2_refresh_works_with_structured_rt() {
    let vault = Arc::new(common::MockVault::new());

    // 1. Setup structured refresh token
    vault
        .save_refresh_token(
            "p1",
            Token {
                value: "old_rt".to_string(),
                expires_at: Utc::now() + Duration::days(7),
                created_at: Utc::now(),
            },
        )
        .await
        .unwrap();

    let pool = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));
    let sender = Arc::new(common::MockHttpSender::with_body(
        "{\"access_token\":\"new_at\",\"refresh_token\":\"new_rt\",\"expires_in\":3600}",
    ));
    let provider = OAuth2Provider::new(pool, sender);

    let config = cowen_common::Config::default_with_profile("p1");

    // 2. Try to refresh. THIS SHOULD NOW SUCCEED.
    let res = provider.refresh("p1", &config, &Default::default()).await;

    assert!(
        res.is_ok(),
        "Refresh should succeed now with structured RT support: {:?}",
        res.err()
    );
    let token = res.unwrap();
    assert_eq!(token.value, "new_at");
}

#[tokio::test]
async fn test_oauth2_on_maintenance_tick_refreshes_expired_token() {
    let vault = Arc::new(common::MockVault::new());

    // 1. Setup expired access token and valid refresh token (structured)
    vault
        .save_access_token(
            "p1",
            Token {
                value: "expired_at".to_string(),
                expires_at: Utc::now() - Duration::minutes(10), // Expired
                created_at: Utc::now() - Duration::hours(1),
            },
        )
        .await
        .unwrap();

    vault
        .save_refresh_token(
            "p1",
            Token {
                value: "valid_rt".to_string(),
                expires_at: Utc::now() + Duration::days(7),
                created_at: Utc::now() - Duration::hours(1),
            },
        )
        .await
        .unwrap();

    let pool = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));
    let sender = Arc::new(common::MockHttpSender::with_body(
        "{\"access_token\":\"new_at\",\"refresh_token\":\"new_rt\",\"expires_in\":3600}",
    ));
    let provider = OAuth2Provider::new(pool, sender);

    let config = cowen_common::Config::default_with_profile("p1");

    // 2. Trigger maintenance tick
    provider
        .on_maintenance_tick("p1", &config)
        .await
        .expect("Maintenance tick failed");

    // 3. Check if access token was updated.
    // EXPECTATION: It SHOULD be updated now.
    let token = vault.get_access_token("p1").await.unwrap();
    assert_eq!(
        token.value, "new_at",
        "Token should have been updated to new_at"
    );
}
