use async_trait::async_trait;
use cowen_auth::client::{HttpSender, SimpleResponse};
use cowen_auth::pool::TokenPool;
use cowen_auth::provider::self_built::SelfBuiltProvider;
use cowen_auth::provider::{AuthProvider, InterceptRequestContext, PlatformEvent, ProxyRequestAction};
use cowen_auth::VaultTokenPool;
use cowen_common::models::{Ticket, Token};
use cowen_common::{Config, CowenResult};
use cowen_store::file::FileStore;
use cowen_store::StoreVault;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::tempdir;
use chrono::Utc;
use cowen_common::domain::TicketDomain;

struct MockHttpSender {
    pub call_count: Arc<AtomicUsize>,
    pub generate_token_response: Option<serde_json::Value>,
}

#[async_trait]
impl HttpSender for MockHttpSender {
    async fn post(
        &self,
        url: &str,
        _headers: reqwest::header::HeaderMap,
        _body: serde_json::Value,
    ) -> CowenResult<SimpleResponse> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        
        if url.contains("generateToken") {
            if let Some(ref resp) = self.generate_token_response {
                return Ok(SimpleResponse {
                    status: 200,
                    body: resp.to_string(),
                });
            }
        }
        
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
    let sender = Arc::new(MockHttpSender {
        call_count: call_count.clone(),
        generate_token_response: None,
    });
    let provider = SelfBuiltProvider::new(pool, sender);

    let config = Config {
        app_key: "AK_TEST".to_string(),
        app_secret: "AS_TEST".to_string(),
        ..Config::default_with_profile("test")
    };

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

    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_do_network_request_for_token_success() {
    let dir = tempdir().unwrap();
    let vault_path = dir.path().join("test2.vault");
    let store = Arc::new(FileStore::new(vault_path, Some("fingerprint")).unwrap());
    let vault = Arc::new(StoreVault::new(store.clone(), store.clone()));
    let pool: Arc<dyn TokenPool> = Arc::new(VaultTokenPool::new(vault.clone()));

    // Insert fake appTicket
    vault.save_app_ticket("AK_TEST", Ticket { value: "mock_ticket_123".to_string(), created_at: Utc::now() }).await.unwrap();

    let call_count = Arc::new(AtomicUsize::new(0));
    let sender = Arc::new(MockHttpSender {
        call_count: call_count.clone(),
        generate_token_response: Some(serde_json::json!({
            "result": true,
            "value": {
                "accessToken": "mock_access_token_abc",
                "expiresIn": 7200
            },
            "code": "200"
        })),
    });
    let provider = SelfBuiltProvider::new(pool.clone(), sender);

    let config = Config {
        app_key: "AK_TEST".to_string(),
        app_secret: "AS_TEST".to_string(),
        ..Config::default_with_profile("test")
    };

    let token = provider.get_token("test", &config, &reqwest::header::HeaderMap::new()).await;
    assert!(token.is_ok());
    let token = token.unwrap();
    assert_eq!(token.value, "mock_access_token_abc");
    
    // Ensure token is persisted in vault
    let cached = pool.get_app_access_token("AK_TEST").await.unwrap();
    assert_eq!(cached.value, "mock_access_token_abc");
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_obtain_app_ticket_missing() {
    let dir = tempdir().unwrap();
    let vault_path = dir.path().join("test3.vault");
    let store = Arc::new(FileStore::new(vault_path, Some("fingerprint")).unwrap());
    let vault = Arc::new(StoreVault::new(store.clone(), store.clone()));
    let pool: Arc<dyn TokenPool> = Arc::new(VaultTokenPool::new(vault.clone()));

    let call_count = Arc::new(AtomicUsize::new(0));
    let sender = Arc::new(MockHttpSender {
        call_count: call_count.clone(),
        generate_token_response: None,
    });
    let provider = SelfBuiltProvider::new(pool.clone(), sender);

    let config = Config {
        app_key: "AK_TEST_MISSING".to_string(),
        app_secret: "AS_TEST".to_string(),
        ..Config::default_with_profile("test")
    };

    let res = provider.get_token("test", &config, &reqwest::header::HeaderMap::new()).await;
    assert!(res.is_err());
    
    // It should have triggered a push. Wait up to 3 seconds for 3 attempts.
    // Call count should be >= 1 for the push request.
    assert!(call_count.load(Ordering::SeqCst) >= 1);
}

#[tokio::test]
async fn test_handle_platform_event_app_ticket() {
    let dir = tempdir().unwrap();
    let vault_path = dir.path().join("test4.vault");
    let store = Arc::new(FileStore::new(vault_path, Some("fingerprint")).unwrap());
    let vault = Arc::new(StoreVault::new(store.clone(), store.clone()));
    let pool: Arc<dyn TokenPool> = Arc::new(VaultTokenPool::new(vault.clone()));

    let call_count = Arc::new(AtomicUsize::new(0));
    let sender = Arc::new(MockHttpSender {
        call_count: call_count.clone(),
        generate_token_response: Some(serde_json::json!({
            "result": true,
            "value": {
                "accessToken": "mock_access_token_from_event",
                "expiresIn": 7200
            }
        })),
    });
    
    let provider = SelfBuiltProvider::new(pool.clone(), sender);

    let config = Config {
        app_key: "AK_TEST_EVENT".to_string(),
        app_secret: "AS_TEST".to_string(),
        ..Config::default_with_profile("test")
    };

    let res = provider.handle_platform_event("test", &config, PlatformEvent::AppTicket("new_ticket_event_123".to_string())).await;
    assert!(res.is_ok());

    // Verify ticket was saved
    let ticket = vault.get_app_ticket("AK_TEST_EVENT").await.unwrap();
    assert_eq!(ticket.value, "new_ticket_event_123");

    // Proactive refresh happens in a spawned task after 1.5 seconds.
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    // Verify token was refreshed proactively
    let cached = pool.get_app_access_token("AK_TEST_EVENT").await.unwrap();
    assert_eq!(cached.value, "mock_access_token_from_event");
}

#[tokio::test]
async fn test_intercept_request_injects_headers() {
    let dir = tempdir().unwrap();
    let vault_path = dir.path().join("test5.vault");
    let store = Arc::new(FileStore::new(vault_path, Some("fingerprint")).unwrap());
    let vault = Arc::new(StoreVault::new(store.clone(), store.clone()));
    let pool: Arc<dyn TokenPool> = Arc::new(VaultTokenPool::new(vault.clone()));

    pool.set_app_access_token("AK_TEST_INJECT", &Token {
        value: "mock_active_token".to_string(),
        created_at: Utc::now(),
        expires_at: Utc::now() + chrono::Duration::hours(2),
    }).await.unwrap();

    let sender = Arc::new(MockHttpSender {
        call_count: Arc::new(AtomicUsize::new(0)),
        generate_token_response: None,
    });
    let provider = SelfBuiltProvider::new(pool.clone(), sender);

    let config = Config {
        app_key: "AK_TEST_INJECT".to_string(),
        app_secret: "AS_TEST".to_string(),
        ..Config::default_with_profile("test")
    };

    let spec = serde_json::Value::Null;
    let ctx = InterceptRequestContext {
        method: "GET",
        path: "/api",
        headers: reqwest::header::HeaderMap::new(),
        body: &[],
        spec: &spec,
    };

    let action = provider.intercept_request("test", &config, ctx).await.unwrap();
    if let ProxyRequestAction::Forward { headers } = action {
        assert_eq!(headers.get("openToken").unwrap().to_str().unwrap(), "mock_active_token");
        assert_eq!(headers.get("appKey").unwrap().to_str().unwrap(), "AK_TEST_INJECT");
    } else {
        panic!("Expected Forward action");
    }
}

#[tokio::test]
async fn test_initialize_saves_secrets() {
    use cowen_auth::provider::InitParams;
    let dir = tempdir().unwrap();
    let vault_path = dir.path().join("test6.vault");
    let store = Arc::new(FileStore::new(vault_path, Some("fingerprint")).unwrap());
    let vault = Arc::new(StoreVault::new(store.clone(), store.clone()));
    let pool: Arc<dyn TokenPool> = Arc::new(VaultTokenPool::new(vault.clone()));

    let sender = Arc::new(MockHttpSender {
        call_count: Arc::new(AtomicUsize::new(0)),
        generate_token_response: None,
    });
    let provider = SelfBuiltProvider::new(pool.clone(), sender);

    let mut config = Config::default_with_profile("test");
    let params = InitParams {
        app_key: Some("AK_INIT".to_string()),
        app_secret: Some("AS_INIT".to_string()),
        certificate: Some("CERT_INIT".to_string()),
        encrypt_key: Some("EK_INIT".to_string()),
        webhook_target: None,
        proxy_port: None,
        auto_start: false,
        ..Default::default()
    };
    
    let mut cfg_mgr = cowen_config::ConfigManager::new().unwrap();
    cfg_mgr.app_dir = dir.path().to_path_buf();

    let res = provider.initialize("test", &mut config, vault.clone(), &cfg_mgr, params, None).await;
    assert!(res.is_ok());

    // check vault
    use cowen_common::domain::SecretDomain;
    let saved_secret = vault.get_secret("test", "app_secret").await.unwrap();
    assert_eq!(saved_secret, "AS_INIT");
    let saved_cert = vault.get_secret("test", "certificate").await.unwrap();
    assert_eq!(saved_cert, "CERT_INIT");
}

#[tokio::test]
async fn test_get_diagnostics() {
    use cowen_common::status::StatusContext;
    let dir = tempdir().unwrap();
    let vault_path = dir.path().join("test7.vault");
    let store = Arc::new(FileStore::new(vault_path, Some("fingerprint")).unwrap());
    let vault = Arc::new(StoreVault::new(store.clone(), store.clone()));
    let pool: Arc<dyn TokenPool> = Arc::new(VaultTokenPool::new(vault.clone()));

    let sender = Arc::new(MockHttpSender {
        call_count: Arc::new(AtomicUsize::new(0)),
        generate_token_response: None,
    });
    let provider = SelfBuiltProvider::new(pool.clone(), sender);

    let config = Config {
        app_key: "AK_DIAG".to_string(),
        app_secret: "AS_DIAG".to_string(),
        ..Config::default_with_profile("test")
    };
    
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
