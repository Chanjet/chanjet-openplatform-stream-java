use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use cowen_auth::pool::TokenPool;
use cowen_auth::provider::oauth2::{OAuth2Provider, Pkce};
use cowen_auth::provider::AuthProvider;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[test]
fn test_pkce_generation() {
    let pkce = Pkce::new();
    assert_eq!(pkce.verifier.len(), 64);

    // Verify challenge can be computed from verifier
    let challenge = Pkce::generate_challenge(&pkce.verifier);
    assert!(!challenge.is_empty());

    // Manual verification of challenge
    let mut hasher = Sha256::new();
    hasher.update(pkce.verifier.as_bytes());
    let expected_challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
    assert_eq!(challenge, expected_challenge);
}

mod common;

#[test]
fn test_oauth2_capabilities() {
    let vault = Arc::new(common::MockVault::new());
    let pool: Arc<dyn TokenPool> = Arc::new(cowen_auth::VaultTokenPool::new(vault));
    let sender = Arc::new(common::MockHttpSender::new());
    let provider = OAuth2Provider::new(pool, sender);
    assert!(!provider.supports_webhooks());
}

#[test]
fn test_verifier_charset() {
    let verifier = Pkce::generate_verifier(1000);
    let allowed = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    for c in verifier.chars() {
        assert!(allowed.contains(c));
    }
}

#[tokio::test]
async fn test_oauth2_get_diagnostics() {
    use cowen_common::config::Config;
    use cowen_common::status::StatusContext;
    use cowen_store::file::FileStore;
    use cowen_store::StoreVault;

    let dir = tempfile::tempdir().unwrap();
    let vault_path = dir.path().join("oauth2.vault");
    let store = Arc::new(FileStore::new(vault_path, Some("fingerprint")).unwrap());
    let vault = Arc::new(StoreVault::new(store.clone(), store.clone()));
    let pool: Arc<dyn TokenPool> = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));

    use cowen_common::domain::TokenDomain;
    let sender = Arc::new(common::MockHttpSender::new());
    let provider = OAuth2Provider::new(pool, sender);

    vault
        .save_access_token(
            "test",
            cowen_common::models::Token {
                value: "mock_at".to_string(),
                created_at: chrono::Utc::now(),
                expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            },
        )
        .await
        .unwrap();
    vault
        .save_refresh_token(
            "test",
            cowen_common::models::Token {
                value: "mock_rt".to_string(),
                created_at: chrono::Utc::now(),
                expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            },
        )
        .await
        .unwrap();

    let config = Config::default_with_profile("test");
    let app_config = cowen_common::config::AppConfig::default();

    let ctx = StatusContext {
        profile: "test".to_string(),
        config: &config,
        app_config: &app_config,
        vault: vault.clone(),
    };

    let diag = provider.get_diagnostics(&ctx).await.unwrap();
    assert!(!diag.is_empty());
}

#[tokio::test]
async fn test_oauth2_check_credentials() {
    use cowen_auth::provider::AuthProvider;
    let vault = Arc::new(common::MockVault::new());
    let pool: Arc<dyn TokenPool> = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));
    let sender = Arc::new(common::MockHttpSender::new());
    let provider = OAuth2Provider::new(pool, sender);

    let res = provider
        .check_credentials(vault.as_ref(), "test")
        .await
        .unwrap();
    assert!(matches!(res, cowen_doctor::DiagnosticStatus::Ok));
}

#[tokio::test]
async fn test_oauth2_intercept_request() {
    use cowen_common::config::Config;

    let vault = Arc::new(common::MockVault::new());
    let pool: Arc<dyn TokenPool> = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));
    let sender = Arc::new(common::MockHttpSender::new());
    let provider = OAuth2Provider::new(pool.clone(), sender);

    pool.set_access_token(
        "test",
        &cowen_common::models::Token {
            value: "mock_oauth_token".to_string(),
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        },
    )
    .await
    .unwrap();

    let config = Config {
        app_key: "AK_TEST_INJECT".to_string(),
        ..Config::default_with_profile("test")
    };
    let spec = serde_json::Value::Null;
    let req_ctx = cowen_auth::provider::InterceptRequestContext {
        method: "GET",
        path: "/api",
        headers: reqwest::header::HeaderMap::new(),
        body: &[],
        spec: &spec,
    };

    let action = provider
        .intercept_request("test", &config, req_ctx)
        .await
        .unwrap();
    if let cowen_auth::provider::ProxyRequestAction::Forward { headers } = action {
        assert_eq!(
            headers.get("openToken").unwrap().to_str().unwrap(),
            "mock_oauth_token"
        );
        assert_eq!(
            headers.get("appKey").unwrap().to_str().unwrap(),
            "AK_TEST_INJECT"
        );
    } else {
        panic!("Expected Forward action");
    }
}

#[tokio::test]
async fn test_oauth2_get_token_fast_path() {
    use cowen_common::config::Config;

    let vault = Arc::new(common::MockVault::new());
    let pool: Arc<dyn TokenPool> = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));
    let sender = Arc::new(common::MockHttpSender::new());
    let provider = OAuth2Provider::new(pool.clone(), sender);

    pool.set_access_token(
        "test_fast_path",
        &cowen_common::models::Token {
            value: "mock_oauth_token_fast".to_string(),
            created_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        },
    )
    .await
    .unwrap();

    let config = Config::default_with_profile("test_fast_path");
    let headers = reqwest::header::HeaderMap::new();
    let token = provider
        .get_token("test_fast_path", &config, &headers)
        .await
        .unwrap();
    assert_eq!(token.value, "mock_oauth_token_fast");
}

#[tokio::test]
async fn test_oauth2_get_token_vault_fallback() {
    use cowen_common::config::Config;
    use cowen_common::domain::TokenDomain;

    let vault = Arc::new(common::MockVault::new());
    let pool: Arc<dyn TokenPool> = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));
    let sender = Arc::new(common::MockHttpSender::new());
    let provider = OAuth2Provider::new(pool.clone(), sender);

    // Set in vault but NOT in pool
    vault
        .save_access_token(
            "test_vault_fallback",
            cowen_common::models::Token {
                value: "mock_oauth_token_vault".to_string(),
                created_at: chrono::Utc::now(),
                expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            },
        )
        .await
        .unwrap();

    let config = Config::default_with_profile("test_vault_fallback");
    let headers = reqwest::header::HeaderMap::new();
    let token = provider
        .get_token("test_vault_fallback", &config, &headers)
        .await
        .unwrap();
    assert_eq!(token.value, "mock_oauth_token_vault");
}

#[tokio::test]
async fn test_oauth2_get_token_refresh_expired() {
    use cowen_common::config::Config;
    use cowen_common::domain::TokenDomain;

    let vault = Arc::new(common::MockVault::new());
    let pool: Arc<dyn TokenPool> = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));
    let sender = Arc::new(common::MockHttpSender::new());
    let provider = OAuth2Provider::new(pool.clone(), sender);

    // Set expired access token and expired refresh token
    vault
        .save_access_token(
            "test_refresh_exp",
            cowen_common::models::Token {
                value: "mock_oauth_token_exp".to_string(),
                created_at: chrono::Utc::now() - chrono::Duration::hours(2),
                expires_at: chrono::Utc::now() - chrono::Duration::hours(1),
            },
        )
        .await
        .unwrap();
    vault
        .save_refresh_token(
            "test_refresh_exp",
            cowen_common::models::Token {
                value: "mock_refresh_token_exp".to_string(),
                created_at: chrono::Utc::now() - chrono::Duration::days(2),
                expires_at: chrono::Utc::now() - chrono::Duration::days(1),
            },
        )
        .await
        .unwrap();

    let config = Config::default_with_profile("test_refresh_exp");
    let headers = reqwest::header::HeaderMap::new();
    let res = provider
        .get_token("test_refresh_exp", &config, &headers)
        .await;
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("OAuth2 session expired") || err_msg.contains("expired"));
}
