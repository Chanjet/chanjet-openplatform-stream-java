mod mock;

use cucumber::{given, then, when, World};
use std::sync::Arc;
use tokio::sync::oneshot;
use reqwest::Client;
use tempfile::tempdir;

use cowen_common::config::{AppConfig, Config, GatewayConfig, AuthRoutingConfig};
use cowen_store::file::FileStore;
use cowen_store::StoreVault;
use cowen_common::vault::Vault;

#[derive(Debug, Default, World)]
#[world(init = Self::new)]
pub struct GatewayWorld {
    pub has_token: bool,
    pub gateway_port: Option<u16>,
    pub last_status: Option<u16>,
    pub client: Client,
}

impl GatewayWorld {
    pub fn new() -> Self {
        Self {
            has_token: false,
            gateway_port: None,
            last_status: None,
            client: Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
        }
    }
}

#[given("客户端未配置有效的 App Token")]
async fn given_no_valid_token(world: &mut GatewayWorld) {
    world.has_token = false;
}

#[given("客户端配置了有效的 App Token")]
async fn given_valid_token(world: &mut GatewayWorld) {
    world.has_token = true;
}

#[given(expr = "网关已启动并监听在 {string}")]
async fn given_gateway_started(world: &mut GatewayWorld, _bind_addr: String) {
    let config = Config::default_with_profile("default");
    let mut gateway_config = GatewayConfig {
        bind_address: "127.0.0.1:0".to_string(),
        auth_sync_hook: None,
        auth_routing: AuthRoutingConfig::default(),
        routes: vec![],
    };
    // Configure default whitelist for /health
    let mut auth_routing = AuthRoutingConfig::default();
    auth_routing.bypass_rules = vec!["/api/v1/health".to_string(), "/health".to_string()];
    gateway_config.auth_routing = auth_routing;
    
    let app_config = AppConfig::default();

    let dir = tempdir().unwrap();
    let store = Arc::new(FileStore::new(dir.path(), None).unwrap());
    
    let temp_vault = StoreVault::new(store.clone(), store.clone());
    temp_vault.migrate().await.unwrap();

    let vault = Arc::new(StoreVault::new(store.clone(), store.clone()));

    let mut mock_client = mock::MockClient::new();
    if !world.has_token {
        mock_client.should_auth_fail = true;
    }

    let auth_client = Arc::new(mock_client);

    let (tx, rx) = oneshot::channel();

    // Spawn the gateway
    tokio::spawn(async move {
        cowen_gateway::start_gateway(
            "test_profile",
            &config,
            &gateway_config,
            &app_config,
            auth_client,
            vault,
            Some(tx),
        )
        .await
        .unwrap();
    });

    // Wait for the gateway to bind and send back its port
    let port = rx.await.expect("Gateway failed to start and send port");
    world.gateway_port = Some(port);
}

#[when(expr = "客户端向网关发送 {string} 请求")]
async fn when_client_sends_request(world: &mut GatewayWorld, request: String) {
    let parts: Vec<&str> = request.split(' ').collect();
    let method = parts[0];
    let path = parts[1];

    let port = world.gateway_port.expect("Gateway port not set");
    let url = format!("http://127.0.0.1:{}{}", port, path);

    let req = match method {
        "GET" => world.client.get(&url).header("Accept", "application/json"),
        "POST" => world.client.post(&url).header("Accept", "application/json"),
        _ => panic!("Unsupported method"),
    };

    let res = req.send().await.expect("Failed to send request");
    world.last_status = Some(res.status().as_u16());
}

#[then(expr = "网关应返回状态码 {string}")]
async fn then_gateway_returns_status(world: &mut GatewayWorld, expected_status: String) {
    let expected: u16 = expected_status.parse().unwrap();
    assert_eq!(world.last_status, Some(expected));
}

#[tokio::main]
async fn main() {
    GatewayWorld::run("tests/features").await;
}
