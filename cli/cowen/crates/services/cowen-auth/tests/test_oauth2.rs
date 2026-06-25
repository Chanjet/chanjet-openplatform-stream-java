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
    use cowen_store::StoreVault;
    use cowen_store::file::FileStore;

    let dir = tempfile::tempdir().unwrap();
    let vault_path = dir.path().join("oauth2.vault");
    let store = Arc::new(FileStore::new(vault_path, Some("fingerprint")).unwrap());
    let vault = Arc::new(StoreVault::new(store.clone(), store.clone()));
    let pool: Arc<dyn TokenPool> = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));

    use cowen_common::domain::TokenDomain;
    let sender = Arc::new(common::MockHttpSender::new());
    let provider = OAuth2Provider::new(pool, sender);

    vault.save_access_token("test", cowen_common::models::Token {
        value: "mock_at".to_string(),
        created_at: chrono::Utc::now(),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
    }).await.unwrap();
    vault.save_refresh_token("test", cowen_common::models::Token {
        value: "mock_rt".to_string(),
        created_at: chrono::Utc::now(),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
    }).await.unwrap();

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

    let res = provider.check_credentials(vault.as_ref(), "test").await.unwrap();
    assert!(matches!(res, cowen_doctor::DiagnosticStatus::Ok));
}

#[tokio::test]
async fn test_oauth2_intercept_request() {
    use cowen_common::config::Config;
    use cowen_store::StoreVault;
    use cowen_store::file::FileStore;

    let vault = Arc::new(common::MockVault::new());
    let pool: Arc<dyn TokenPool> = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));
    let sender = Arc::new(common::MockHttpSender::new());
    let provider = OAuth2Provider::new(pool.clone(), sender);

    pool.set_access_token("test", &cowen_common::models::Token {
        value: "mock_oauth_token".to_string(),
        created_at: chrono::Utc::now(),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
    }).await.unwrap();

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

    let action = provider.intercept_request("test", &config, req_ctx).await.unwrap();
    if let cowen_auth::provider::ProxyRequestAction::Forward { headers } = action {
        assert_eq!(headers.get("openToken").unwrap().to_str().unwrap(), "mock_oauth_token");
        assert_eq!(headers.get("appKey").unwrap().to_str().unwrap(), "AK_TEST_INJECT");
    } else {
        panic!("Expected Forward action");
    }
}

