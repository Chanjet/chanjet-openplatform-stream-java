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
