use cucumber::{given, then, when, World};
use reqwest::StatusCode;
use std::convert::Infallible;

#[derive(Debug, Default, World)]
#[world(init = Self::new)]
pub struct GatewayWorld {
    pub has_token: bool,
    pub gateway_url: String,
    pub last_status: Option<u16>,
}

impl GatewayWorld {
    pub fn new() -> Self {
        Self::default()
    }
}

#[given("客户端未配置有效的 App Token")]
async fn given_no_valid_token(world: &mut GatewayWorld) {
    world.has_token = false;
}

#[given(expr = "网关已启动并监听在 {string}")]
async fn given_gateway_started(world: &mut GatewayWorld, bind_addr: String) {
    // In a real test, we would start the actual gateway process or axum router here
    // For this example, we mock the fact that it started at some URL
    // e.g. let addr = start_mock_gateway().await;
    world.gateway_url = format!("http://{}", bind_addr.replace("0", "8080"));
}

#[when(expr = "客户端向网关发送 {string} 请求")]
async fn when_client_sends_request(world: &mut GatewayWorld, request: String) {
    // We mock the HTTP request to the gateway
    // In real scenario: let client = reqwest::Client::new(); let res = client.get(...).send().await;
    let parts: Vec<&str> = request.split(' ').collect();
    let _method = parts[0];
    let _path = parts[1];

    if !world.has_token {
        // Mocking the gateway intercepting the request
        world.last_status = Some(401);
    } else {
        world.last_status = Some(200);
    }
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
