use cowen_auth::models::Token;
use cowen_auth::pool::TokenPool;
use cowen_auth::provider::store_app::StoreAppProvider;
use cowen_auth::provider::AuthProvider;
use cowen_auth::VaultTokenPool;
use cowen_common::{Config, CowenResult};
use std::sync::Arc;

mod common;
#[tokio::test]
async fn test_get_token_missing_org_id_rejection() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(cowen_store::FileStore::new(tmp.path().join("vault"), None).unwrap());
    let vault = Arc::new(common::MockVault::with_store(store));
    let pool: Arc<dyn TokenPool> = Arc::new(VaultTokenPool::new(vault));
    let sender = Arc::new(common::MockHttpSender::new());
    let provider = StoreAppProvider::new(pool, sender);

    let config = Config::default_with_profile("test");
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-user-id", "U123".parse().unwrap());

    let result: CowenResult<Token> = provider.get_token("default", &config, &headers).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("401 Unauthorized"));
    assert!(err.to_string().contains("x-org-id"));
}
