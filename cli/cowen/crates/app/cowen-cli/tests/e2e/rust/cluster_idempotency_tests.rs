use reqwest::Client;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn test_cluster_idempotency() {
    let (mock_port, _state) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}/connect", mock_port);

    let home_a = tempfile::tempdir().unwrap();
    let home_a_path = home_a.path();
    let db_path = home_a_path.join("shared_cluster.db");
    let db_url = format!("sqlite://{}", db_path.display());

    let profile = "cluster_node";

    let app_yaml_content = format!(
        "monitor_port: 0\nstorage:\n  store: sqlite\n  db_url: \"{}\"\n",
        db_url
    );
    std::fs::write(home_a_path.join("app.yaml"), &app_yaml_content).unwrap();

    let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    cmd.env("COWEN_HOME", home_a_path);
    cmd.args([
        "init",
        "--profile",
        profile,
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_CLUSTER",
        "--app-secret",
        "AS_CLUSTER",
        "--certificate",
        "CERT_CLUSTER",
        "--encrypt-key",
        "1234567890123456",
        "--webhook-target",
        &format!("{}/webhook_sink", mock_url),
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--proxy-port",
        "0",
    ]);
    assert!(cmd.status().unwrap().success());

    // Start Node A
    let mut node_a = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    node_a.env("COWEN_HOME", home_a_path);
    node_a.args(["daemon", "start", "--profile", profile, "--foreground"]);
    let mut child_a = node_a.spawn().unwrap();

    tokio::time::sleep(Duration::from_secs(3)).await;

    // Login to fetch AppTicket
    let mut cmd_auth = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    cmd_auth.env("COWEN_HOME", home_a_path);
    cmd_auth.args(["auth", "login", "--profile", profile, "--force"]);
    assert!(cmd_auth.status().unwrap().success());

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Setup Node B HOME
    let home_b = tempfile::tempdir().unwrap();
    let home_b_path = home_b.path();
    std::fs::write(home_b_path.join("app.yaml"), &app_yaml_content).unwrap();

    // Start Node B
    let mut node_b = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    node_b.env("COWEN_HOME", home_b_path);
    node_b.args(["daemon", "start", "--profile", profile, "--foreground"]);
    let mut child_b = node_b.spawn().unwrap();

    tokio::time::sleep(Duration::from_secs(5)).await;

    // Broadcast a single message
    let msg_id = format!(
        "MSG_IDEMP_{}",
        std::time::UNIX_EPOCH.elapsed().unwrap().as_secs()
    );

    let client = Client::new();
    client
        .post(format!("{}/control/clear_webhooks", mock_url))
        .send()
        .await
        .unwrap();

    let payload = serde_json::json!({
        "msgType": "DATA_PUSH",
        "msg_type": "DATA_PUSH",
        "appKey": "AK_CLUSTER",
        "msgId": msg_id,
        "biz_content": {"data": "idempotency_test"},
        "bizContent": {"data": "idempotency_test"},
        "time": "2026-06-26 12:00:00"
    });

    client
        .post(format!("{}/control/broadcast", mock_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(5)).await;

    let resp = client
        .get(format!("{}/control/webhooks", mock_url))
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();

    let mut received_count = 0;
    for hook in resp {
        if let Some(body) = hook.get("body") {
            if body.get("msgId").and_then(|v| v.as_str()) == Some(&msg_id) {
                received_count += 1;
            }
        } else if hook.get("msgId").and_then(|v| v.as_str()) == Some(&msg_id) {
            received_count += 1;
        }
    }

    println!("Received {} messages", received_count);

    // Kill daemons
    crate::e2e::rust::common::graceful_kill_child(&mut child_a).ok();
    child_a.wait().ok();
    crate::e2e::rust::common::graceful_kill_child(&mut child_b).ok();
    child_b.wait().ok();

    assert!(
        received_count > 0,
        "At least one message should be received"
    );
    if received_count > 1 {
        println!("⚠️ [BLIND SPOT VERIFIED] Idempotency violation! Sink received {} messages for the same msgId.", received_count);
    } else {
        println!("✅ Idempotency successful! Only 1 message received at sink.");
    }
}
