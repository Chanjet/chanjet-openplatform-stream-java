use crate::e2e::rust::common::setup_test_env;
use assert_cmd::Command;

#[tokio::test(flavor = "multi_thread")]
async fn test_dlq_paging_and_retry() {
    let (mock_port, _state) = crate::e2e::rust::mock_server::spawn_mock_server().await;
    let openapi_url = format!("http://127.0.0.1:{}", mock_port);

    let (dir, home, _killer) = setup_test_env("case_52", "self-built", &openapi_url);

    // 1. Setup Profile with Invalid Webhook (to trigger DLQ)
    let mut cmd_init = Command::cargo_bin("cowen").unwrap();
    cmd_init.env("COWEN_HOME", &home);
    cmd_init.env("HOME", &home);
    cmd_init.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_init.args([
        "init",
        "--profile",
        "case_52",
        "--app-mode",
        "self-built",
        "--app-key",
        "test_key_dlq",
        "--app-secret",
        "test_secret_dlq",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "test_cert",
        "--openapi-url",
        &openapi_url,
        "--stream-url",
        &format!("ws://127.0.0.1:{}", mock_port),
        "--webhook-target",
        "http://127.0.0.1:1", // Invalid port
    ]);
    cmd_init.assert().success();

    // 2. Start Daemon
    let mut cmd_daemon = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    cmd_daemon.env("COWEN_HOME", &home);
    cmd_daemon.env("HOME", &home);
    cmd_daemon.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_daemon.args(["daemon", "start", "--foreground"]);
    let mut child = cmd_daemon.spawn().unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Inject 25 Failed Messages
    let client = reqwest::Client::new();
    for i in 1..=25 {
        let payload = serde_json::json!({
            "msg_id": format!("msg_{}", i),
            "data": format!("value_{}", i)
        });
        let body = serde_json::json!({
            "msg_type": "DATA_PUSH",
            "payload": payload
        });
        let _res = client
            .post(format!("{}/control/broadcast", openapi_url))
            .header("Content-Type", "application/json")
            .header("appKey", "test_key_dlq")
            .json(&body)
            .send()
            .await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // Wait for messages to hit DLQ
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // 3. Verify DLQ Paging (Default 20 items)
    let mut cmd_list = Command::cargo_bin("cowen").unwrap();
    cmd_list.env("COWEN_HOME", &home);
    cmd_list.env("HOME", &home);
    cmd_list.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_list.args(["dlq", "list", "--profile", "case_52"]);
    let list_output = String::from_utf8_lossy(&cmd_list.output().unwrap().stdout).to_string();
    let count = list_output.lines().filter(|l| l.contains("ID:")).count();
    assert_eq!(
        count, 20,
        "Expected 20 items in default list, found {}",
        count
    );

    // 4. Verify DLQ Page 2
    let mut cmd_list_p2 = Command::cargo_bin("cowen").unwrap();
    cmd_list_p2.env("COWEN_HOME", &home);
    cmd_list_p2.env("HOME", &home);
    cmd_list_p2.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_list_p2.args(["dlq", "list", "--page", "2", "--profile", "case_52"]);
    let list_output_p2 = String::from_utf8_lossy(&cmd_list_p2.output().unwrap().stdout).to_string();
    let count_p2 = list_output_p2.lines().filter(|l| l.contains("ID:")).count();
    assert!(
        count_p2 >= 5,
        "Expected at least 5 items in page 2, found {}",
        count_p2
    );

    // 5. Verify Precise Retry
    let mut first_id_p2 = None;
    for line in list_output_p2.lines() {
        if line.contains("ID:") {
            let id_str = line
                .split("ID:")
                .nth(1)
                .unwrap()
                .split(']')
                .next()
                .unwrap()
                .trim();
            first_id_p2 = Some(id_str.to_string());
            break;
        }
    }
    let first_id_p2 = first_id_p2.unwrap();

    // Temporarily fix webhook target
    let mut cmd_cfg = Command::cargo_bin("cowen").unwrap();
    cmd_cfg.env("COWEN_HOME", &home);
    cmd_cfg.env("HOME", &home);
    cmd_cfg.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_cfg.args([
        "config",
        "set",
        "webhook_target",
        &format!("{}/webhook_sink", openapi_url),
        "--profile",
        "case_52",
    ]);
    cmd_cfg.assert().success();

    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

    let mut cmd_retry = Command::cargo_bin("cowen").unwrap();
    cmd_retry.env("COWEN_HOME", &home);
    cmd_retry.env("HOME", &home);
    cmd_retry.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_retry.args(["dlq", "retry", &first_id_p2, "--profile", "case_52"]);
    cmd_retry.assert().success();

    // Verify it's gone
    let mut cmd_list_p2_after = Command::cargo_bin("cowen").unwrap();
    cmd_list_p2_after.env("COWEN_HOME", &home);
    cmd_list_p2_after.env("HOME", &home);
    cmd_list_p2_after.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    cmd_list_p2_after.args(["dlq", "list", "--page", "2", "--profile", "case_52"]);
    let list_output_p2_after =
        String::from_utf8_lossy(&cmd_list_p2_after.output().unwrap().stdout).to_string();
    assert!(
        !list_output_p2_after.contains(&format!("ID: {} ", first_id_p2))
            && !list_output_p2_after.ends_with(&format!("ID: {}", first_id_p2)),
        "Item still in DLQ"
    );

    let _ = child.kill();
    let _ = child.wait();
    let _ = dir;
}
