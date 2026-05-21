use chrono::{Duration, SubsecRound, Utc};
use cowen_auth::pool::TokenPool;
use cowen_auth::VaultTokenPool;
use cowen_common::models::{Ticket, Token};
use cowen_store::file::FileStore;
use cowen_store::StoreVault;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_vault_token_pool_lifecycle() {
    let dir = tempdir().unwrap();
    let vault_path = dir.path().join("test.vault");
    let store = Arc::new(FileStore::new(vault_path, "fingerprint").unwrap());
    let vault = Arc::new(StoreVault::new(store.clone(), store.clone()));
    let pool = VaultTokenPool::new(vault);
    let profile = "test-profile";

    // 1. Ticket
    let app_key = "test_app_key";
    let ticket = Ticket {
        value: "ticket-123".to_string(),
        created_at: Utc::now(),
    };
    pool.set_app_ticket(app_key, &ticket).await.unwrap();
    let retrieved_ticket = pool.get_app_ticket(app_key).await.unwrap();
    assert_eq!(retrieved_ticket.value, "ticket-123");

    // 2. Token
    let now = Utc::now().round_subsecs(0);
    let token = Token {
        value: "token-abc".to_string(),
        expires_at: now + Duration::hours(2),
        created_at: now,
    };
    pool.set_access_token(profile, &token).await.unwrap();

    let retrieved_token = pool.get_access_token(profile).await.unwrap();
    assert_eq!(retrieved_token.value, "token-abc");
    assert_eq!(
        retrieved_token.expires_at.to_rfc3339(),
        token.expires_at.to_rfc3339()
    );

    // 3. Delete
    pool.delete_access_token(profile).await.unwrap();
    assert!(pool.get_access_token(profile).await.is_err());
}
