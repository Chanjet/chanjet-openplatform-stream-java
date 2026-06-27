#[tokio::test(flavor = "multi_thread")]
async fn test_dlq_manual_retry() {
    let (mock_port, _state) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);
    let webhook_sink = format!("{}/webhook_sink", mock_url);
    let profile = "dlq_manual";
    let dir = tempfile::tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    std::fs::create_dir_all(&cowen_home).unwrap();
    let home_str = cowen_home.to_str().unwrap().to_string();

    // Set mock to return 500
    let client = reqwest::Client::new();
    client.post(format!("{}/control/config", mock_url))
        .json(&serde_json::json!({"webhook_sink_status": 500}))
        .send().await.unwrap();

    let mut cmd_init = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_init.env("COWEN_HOME", &home_str);
    cmd_init.env("HOME", &home_str);
    cmd_init.args([
        "init", "--profile", profile, "--app-mode", "self-built",
        "--app-key", "AK_DLQ", "--app-secret", "AS_DLQ",
        "--encrypt-key", "1234567890123456", "--certificate", "CERT_DLQ",
        "--webhook-target", &webhook_sink,
        "--openapi-url", &mock_url, "--stream-url", &mock_ws,
    ]);
    cmd_init.assert().success();

    let mut cmd_start = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_start.env("COWEN_HOME", &home_str);
    cmd_start.env("HOME", &home_str);
    cmd_start.args(["daemon", "start", "--profile", profile, "--all"]);
    cmd_start.assert().success();

    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    let mut cmd_auth = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_auth.env("COWEN_HOME", &home_str);
    cmd_auth.env("HOME", &home_str);
    cmd_auth.args(["auth", "login", "--profile", profile, "--force"]);
    cmd_auth.assert().success();

    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    let payload = serde_json::json!({
        "msg_type": "DLQ_MANUAL",
        "msgType": "DLQ_MANUAL",
        "msgId": "mock_dlq_manual_123",
        "appKey": "AK_DLQ",
        "biz_content": { "test": "fail_manual" },
        "bizContent": { "test": "fail_manual" },
        "time": "2026-06-26 12:00:00"
    });

    client.post(format!("{}/control/broadcast", mock_url))
        .json(&payload).send().await.unwrap();

    let mut found_id = None;
    for _ in 0..15 {
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        let mut cmd_dlq = assert_cmd::Command::cargo_bin("cowen").unwrap();
        cmd_dlq.env("COWEN_HOME", &home_str);
        cmd_dlq.env("HOME", &home_str);
        cmd_dlq.args(["dlq", "list", "--profile", profile, "--format", "json"]);

        let output = cmd_dlq.output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        if stdout.contains("mock_dlq_manual_123") {
            if let Ok(json_arr) = serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
                if let Some(item) = json_arr.into_iter().find(|i| {
                    i.get("msg_id").and_then(|v| v.as_str()) == Some("mock_dlq_manual_123")
                }) {
                    found_id = item.get("id").and_then(|v| v.as_i64());
                    break;
                }
            }
        }
    }
    
    let dlq_id = found_id.expect("Message NOT found in DLQ");
    
    let _killer = crate::e2e::rust::common::DaemonKiller {
        home: cowen_home.to_str().unwrap().to_string(),
    };

    client.post(format!("{}/control/config", mock_url))
        .json(&serde_json::json!({"webhook_sink_status": 200}))
        .send().await.unwrap();
    client.post(format!("{}/control/clear_webhooks", mock_url))
        .send().await.unwrap();

    let mut cmd_retry = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_retry.env("COWEN_HOME", &home_str);
    cmd_retry.env("HOME", &home_str);
    cmd_retry.args(["dlq", "retry", &dlq_id.to_string(), "--profile", profile]);
    cmd_retry.assert().success();

    let mut delivered = false;
    for _ in 0..10 {
        let res = client.get(format!("{}/control/webhooks", mock_url)).send().await.unwrap();
        let text = res.text().await.unwrap();
        if text.contains("mock_dlq_manual_123") {
            delivered = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    assert!(delivered, "Message not delivered to sink after manual retry");

    let mut cmd_dlq2 = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_dlq2.env("COWEN_HOME", &home_str);
    cmd_dlq2.env("HOME", &home_str);
    cmd_dlq2.args(["dlq", "list", "--profile", profile]);
    let output2 = cmd_dlq2.output().unwrap();
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(!stdout2.contains("mock_dlq_manual_123"), "Specific DLQ entry still exists after retry");
}
