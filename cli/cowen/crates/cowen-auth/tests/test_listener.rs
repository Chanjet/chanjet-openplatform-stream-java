use cowen_auth::lifecycle::listener::OAuth2CallbackListener;
use reqwest::Client;

#[tokio::test]
async fn test_callback_capture() {
    let (port, rx) = OAuth2CallbackListener::start(0, "test_profile".to_string()).await.unwrap();
    assert!(port > 0);

    let url = format!("http://127.0.0.1:{}/callback?code=test_code&state=test_state", port);
    let client = Client::new();
    let resp = client.get(&url).send().await.unwrap();
    assert!(resp.status().is_success());

    let result = rx.await.unwrap().unwrap();
    assert_eq!(result.code, "test_code");
    assert_eq!(result.state, "test_state");
    
    // Ensure server is down
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let resp_retry = client.get(&url).send().await;
    assert!(resp_retry.is_err());
}
