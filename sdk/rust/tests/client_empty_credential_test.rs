use connector_sdk::{ClientOptions, GatewayClient};

#[tokio::test]
async fn test_empty_app_key_fails_fast() {
    let options = ClientOptions {
        app_key: "".to_string(),
        app_secret: "secret".to_string(),
        encrypt_key: None,
        gateway_url: "ws://localhost:8080".to_string(),
        ..Default::default()
    };

    let client = GatewayClient::new(options);
    let result = client.start().await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("AppKey cannot be empty"));
}

#[tokio::test]
async fn test_empty_app_secret_fails_fast() {
    let options = ClientOptions {
        app_key: "app_key".to_string(),
        app_secret: "".to_string(),
        encrypt_key: None,
        gateway_url: "ws://localhost:8080".to_string(),
        ..Default::default()
    };

    let client = GatewayClient::new(options);
    let result = client.start().await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("AppSecret cannot be empty"));
}
