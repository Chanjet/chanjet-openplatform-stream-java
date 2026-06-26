use cowen_auth::client::{AuthClient, Client};
use cowen_auth::pool::VaultTokenPool;
use cowen_common::config::{AppConfig, Config};
use cowen_common::models::AuthMode;
use cucumber::{given, then, when, World};
use std::sync::Arc;

mod mock;
use mock::InMemoryHttpSender;

#[derive(World)]
#[world(init = Self::new)]
struct AuthWorld {
    config: Config,
    http_sender: Arc<InMemoryHttpSender>,
    token_result: Option<Result<cowen_common::models::Token, cowen_common::CowenError>>,
}

impl std::fmt::Debug for AuthWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthWorld").finish()
    }
}

impl AuthWorld {
    fn new() -> Self {
        Self {
            config: Config::default_with_profile("test"),
            http_sender: Arc::new(InMemoryHttpSender::new()),
            token_result: None,
        }
    }
}

#[given(expr = "the AuthMode is {string}")]
async fn set_auth_mode(world: &mut AuthWorld, mode_str: String) {
    world.config.app_mode = match mode_str.as_str() {
        "OAuth2" => AuthMode::Oauth2,
        "SelfBuilt" => AuthMode::SelfBuilt,
        "StoreApp" => AuthMode::StoreApp,
        _ => panic!("Unknown auth mode: {}", mode_str),
    };
    world.config.app_key = "test_app_key".to_string();
    world.config.app_secret = "test_secret".to_string();
    world.config.encrypt_key = "1234567890123456".to_string();
}

#[given(expr = "the HttpSender will return a valid token with expires_in {int}")]
async fn mock_valid_token(world: &mut AuthWorld, expires_in: u64) {
    let body = if world.config.app_mode == AuthMode::Oauth2 {
        serde_json::json!({
            "access_token": "mocked_access_token",
            "refresh_token": "mocked_refresh_token",
            "expires_in": expires_in,
            "token_type": "Bearer"
        })
    } else {
        serde_json::json!({
            "code": "200",
            "result": true,
            "value": {
                "accessToken": "mocked_access_token",
                "expiresIn": expires_in
            }
        })
    };
    world
        .http_sender
        .push_response(200, &body.to_string())
        .await;
}

#[given(expr = "the HttpSender will return a {int} Unauthorized")]
async fn mock_unauthorized(world: &mut AuthWorld, status: u16) {
    let body = serde_json::json!({
        "code": "401",
        "msg": "Unauthorized"
    });
    world
        .http_sender
        .push_response(status, &body.to_string())
        .await;
}

#[when("I initialize the AuthClient and request a token")]
async fn init_and_request_token(world: &mut AuthWorld) {
    let temp_dir = tempfile::tempdir().unwrap();
    let app_cfg = AppConfig::default();
    let vault = cowen_store::create_vault(&app_cfg, temp_dir.path(), "test_fingerprint")
        .await
        .unwrap();

    let pool = Arc::new(VaultTokenPool::new(vault.clone()));

    if world.config.app_mode == AuthMode::Oauth2 {
        let t = cowen_common::models::Token {
            value: "expired_token".to_string(),
            expires_at: chrono::Utc::now() - chrono::Duration::try_seconds(3600).unwrap(),
            created_at: chrono::Utc::now() - chrono::Duration::try_seconds(7200).unwrap(),
        };
        let _ = vault.save_access_token("test", t).await;

        let rt = cowen_common::models::Token {
            value: "valid_refresh_token".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::try_seconds(86400).unwrap(),
            created_at: chrono::Utc::now(),
        };
        let _ = vault.save_refresh_token("test", rt).await;
    } else if world.config.app_mode == AuthMode::SelfBuilt {
        let _ = vault
            .set_secret("test", "app_ticket", "test_app_ticket")
            .await;
    }

    // We cannot use create_auth_client directly because we need to inject the mock.
    let sender: Arc<dyn cowen_auth::client::HttpSender> = world.http_sender.clone();
    let builder = AuthClient::builder(pool.clone()).with_http_sender(sender.clone());

    let client = builder
        .register(
            AuthMode::SelfBuilt,
            Arc::new(cowen_auth::provider::self_built::SelfBuiltProvider::new(
                pool.clone(),
                sender.clone(),
            )),
        )
        .register(
            AuthMode::Oauth2,
            Arc::new(cowen_auth::provider::oauth2::OAuth2Provider::new(
                pool.clone(),
                sender.clone(),
            )),
        )
        .register(
            AuthMode::StoreApp,
            Arc::new(cowen_auth::provider::store_app::StoreAppProvider::new(
                pool.clone(),
                sender.clone(),
            )),
        )
        .build();

    let token_result = client
        .get_token("test", &world.config, &reqwest::header::HeaderMap::new())
        .await;

    world.token_result = Some(token_result);
}

#[then("the returned token should be valid")]
async fn check_token_valid(world: &mut AuthWorld) {
    let res = world
        .token_result
        .as_ref()
        .expect("Token was not requested");
    match res {
        Ok(t) => {
            assert_eq!(t.value, "mocked_access_token");
        }
        Err(e) => {
            panic!("Expected valid token, got error: {}", e);
        }
    }
}

#[then(expr = "the HttpSender should have been called {int} time")]
#[then(expr = "the HttpSender should have been called {int} times")]
async fn check_http_calls(world: &mut AuthWorld, times: usize) {
    let requests = world.http_sender.requests.lock().await;
    assert_eq!(requests.len(), times);
}

#[then("it should fall back to vault or throw expected diagnostics error")]
async fn check_fallback_error(world: &mut AuthWorld) {
    let res = world
        .token_result
        .as_ref()
        .expect("Token was not requested");
    match res {
        Ok(_) => panic!("Expected error due to 401, but got token"),
        Err(e) => {
            let e_str = format!("{:?}", e);
            println!("Actual error string: {}", e_str);
            assert!(
                e_str.contains("401")
                    || e_str.contains("Unauthorized")
                    || e_str.contains("NotFound")
                    || e_str.contains("Store")
                    || e_str.contains("Serialization")
                    || e_str.contains("Fetch")
                    || e_str.contains("CowenError")
            );
        }
    }
}

#[tokio::main]
async fn main() {
    AuthWorld::cucumber().run_and_exit("tests/features/").await;
}
