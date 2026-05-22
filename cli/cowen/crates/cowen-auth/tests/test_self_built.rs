use async_trait::async_trait;
use cowen_auth::client::{HttpSender, SimpleResponse};
use cowen_auth::pool::TokenPool;
use cowen_auth::provider::self_built::SelfBuiltProvider;
use cowen_auth::provider::AuthProvider;
use cowen_auth::VaultTokenPool;
use cowen_common::{Config, CowenError, CowenResult};
use cowen_store::file::FileStore;
use cowen_store::StoreVault;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::tempdir;

struct CounterHttpSender {
    pub call_count: Arc<AtomicUsize>,
}

#[async_trait]
impl HttpSender for CounterHttpSender {
    async fn post(
        &self,
        _url: &str,
        _headers: reqwest::header::HeaderMap,
        _body: serde_json::Value,
    ) -> CowenResult<SimpleResponse> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(SimpleResponse {
            status: 200,
            body: serde_json::json!({
                "code": "200",
                "message": "success"
            })
            .to_string(),
        })
    }

    async fn post_form(
        &self,
        _url: &str,
        _headers: reqwest::header::HeaderMap,
        _body: serde_json::Value,
    ) -> CowenResult<SimpleResponse> {
        Ok(SimpleResponse {
            status: 200,
            body: "{}".to_string(),
        })
    }

    async fn get(
        &self,
        _url: &str,
        _headers: reqwest::header::HeaderMap,
    ) -> CowenResult<SimpleResponse> {
        Ok(SimpleResponse {
            status: 200,
            body: "{}".to_string(),
        })
    }
}

#[tokio::test]
async fn test_self_built_concurrent_resend_lock() {
    let dir = tempdir().unwrap();
    let vault_path = dir.path().join("test.vault");
    let store = Arc::new(FileStore::new(vault_path, Some("fingerprint")).unwrap());
    let vault = Arc::new(StoreVault::new(store.clone(), store.clone()));
    let pool: Arc<dyn TokenPool> = Arc::new(VaultTokenPool::new(vault));

    let call_count = Arc::new(AtomicUsize::new(0));
    let sender = Arc::new(CounterHttpSender {
        call_count: call_count.clone(),
    });
    let provider = SelfBuiltProvider::new(pool, sender);

    let config = Config {
        app_key: "AK_TEST".to_string(),
        app_secret: "AS_TEST".to_string(),
        ..Config::default_with_profile("test")
    };

    // Spawn 10 concurrent trigger_push requests
    let provider = Arc::new(provider);
    let config = Arc::new(config);
    let mut tasks = vec![];
    for _ in 0..10 {
        let p = provider.clone();
        let c = config.clone();
        tasks.push(tokio::spawn(async move {
            let _ = p.trigger_push("test", &c, true).await;
        }));
    }

    for task in tasks {
        let _ = task.await;
    }

    // Since it's a concurrent lock/dedup, only 1 request should succeed to send
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}
