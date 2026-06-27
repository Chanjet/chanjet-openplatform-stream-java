use assert_cmd::Command;
use axum::Router;
use std::fs;
use std::net::SocketAddr;
use tempfile::tempdir;

use super::mock_server::spawn_mock_server;

async fn spawn_service_b() -> u16 {
    let app = Router::new().fallback(|req: axum::extract::Request| async move {
        let org_id = req
            .headers()
            .get("x-org-id")
            .map(|v| v.to_str().unwrap())
            .unwrap_or("");
        let path = req.uri().path().to_string();
        format!("Order Service, org_id: {}, path: {}", org_id, path)
    });
    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    port
}

#[tokio::test(flavor = "multi_thread")]
async fn test_gateway_direct_openapi() {
    let (mock_port, _state) = spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);

    let b_port = spawn_service_b().await;

    // Bind gateway and monitor port
    let g_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gateway_port = g_listener.local_addr().unwrap().port();
    drop(g_listener);
    let m_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let monitor_port = m_listener.local_addr().unwrap().port();
    drop(m_listener);

    let dir = tempdir().unwrap();
    let cowen_home = dir.path().to_str().unwrap().to_string();
    fs::create_dir_all(&cowen_home).unwrap();

    let default_yaml = format!(
        r#"
app_key: "mock_app_key"
webhook_target: "{mock_url}"
app_mode: "store-app"
gateway:
  bind_address: "127.0.0.1:{gateway_port}"
  auth_routing:
    mode: "STRICT"
    bypass_rules:
      - "/v1/mock/ping"
      - "/open-api/v1/mock/ping"
    require_rules:
      - "**"
  routes:
    - path: "/open-api/**"
      upstream: "openapi"
      strip_prefix: "/open-api"
    - path: "/order/**"
      upstream: "http://127.0.0.1:{b_port}"
      strip_prefix: "/order"
    - path: "/**"
      upstream: "{mock_url}"
"#
    );
    fs::write(dir.path().join("default.yaml"), default_yaml).unwrap();

    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &cowen_home);
    init_cmd.args([
        "init",
        "--app-mode",
        "store_app",
        "--app-key",
        "mock_app_key",
        "--app-secret",
        "mock_app_secret",
        "--encrypt-key",
        "mock_encrypt_key",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
    ]);
    init_cmd.assert().success();

    let mut config_cmd = Command::cargo_bin("cowen").unwrap();
    config_cmd.env("COWEN_HOME", &cowen_home);
    config_cmd.args(["config", "set", "monitor_port", &monitor_port.to_string()]);
    config_cmd.assert().success();

    let mut start_cmd = Command::cargo_bin("cowen").unwrap();
    start_cmd.env("COWEN_HOME", &cowen_home);
    start_cmd.args(["daemon", "start"]);
    start_cmd.assert().success();

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let gateway_url = format!("http://127.0.0.1:{}", gateway_port);
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .pool_max_idle_per_host(0)
        .build()
        .unwrap();

    // 1. Default Upstream Route
    let mock_res = client
        .post(format!("{}/oauth2/token", mock_url))
        .form(&[
            ("grant_type", "code"),
            ("client_id", "mock_app_key"),
            ("code", "123"),
        ])
        .send()
        .await
        .unwrap();
    let mock_text = mock_res.text().await.unwrap();
    println!("MOCK RESPONSE: {}", mock_text);

    let res = client
        .get(format!("{}/v1/mock/ping", gateway_url))
        .send()
        .await
        .unwrap();
    let text = res.text().await.unwrap();
    assert!(text.contains("status"), "Expected status, got: {}", text);

    // 2. Establish session
    let res = client
        .get(format!("{}/home?code=code_org888", gateway_url))
        .send()
        .await
        .unwrap();
    let mut cookie_header = String::new();
    if let Some(cookie) = res.headers().get("set-cookie") {
        cookie_header = cookie.to_str().unwrap().to_string();
    }
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Direct OpenAPI Route
    let res = client
        .get(format!("{}/open-api/v1/mock/secure", gateway_url))
        .header("Cookie", &cookie_header)
        .send()
        .await
        .unwrap();
    let status = res.status();
    let text = res.text().await.unwrap();
    let logs_dir = format!("{}/logs", cowen_home);
    let out_content =
        std::fs::read_to_string(format!("{}/daemon.stdout.log", logs_dir)).unwrap_or_default();
    let err_content =
        std::fs::read_to_string(format!("{}/daemon.stderr.log", logs_dir)).unwrap_or_default();
    assert!(
        status.is_success(),
        "Expected verified, got status: {}, body: {}\nStdout:\n{}\nStderr:\n{}",
        status,
        text,
        out_content,
        err_content
    );
    assert!(
        text.contains("verified"),
        "Expected verified, got: {}",
        text
    );

    // Stop daemon
    let mut stop_cmd = Command::cargo_bin("cowen").unwrap();
    stop_cmd.env("COWEN_HOME", &cowen_home);
    stop_cmd.args(["daemon", "stop"]);
    let _ = stop_cmd.output();
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Purge DB
    let db_path = dir.path().join("cowen.db");
    let mut sqlite_cmd = std::process::Command::new("sqlite3");
    sqlite_cmd.arg(db_path.to_str().unwrap()).arg("DELETE FROM cowen_tenant_token; DELETE FROM cowen_secret WHERE item_key LIKE 'oauth2_token_pair_%';");
    let _ = sqlite_cmd.output();

    // Restart daemon
    let mut start_cmd2 = Command::cargo_bin("cowen").unwrap();
    start_cmd2.env("COWEN_HOME", &cowen_home);
    start_cmd2.args(["daemon", "start"]);
    start_cmd2.assert().success();
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let mut dump_cmd = std::process::Command::new("sqlite3");
    dump_cmd
        .arg(db_path.to_str().unwrap())
        .arg("SELECT * FROM cowen_permanent_code;");
    let dump_out = dump_cmd.output().unwrap();
    println!("DB DUMP: {}", String::from_utf8_lossy(&dump_out.stdout));

    // Direct OpenAPI Recovery
    let res = client
        .get(format!("{}/open-api/v1/mock/secure", gateway_url))
        .header("Cookie", &cookie_header)
        .send()
        .await
        .unwrap();
    let status = res.status();
    let headers = format!("{:?}", res.headers());
    let text = res.text().await.unwrap();
    let logs_dir = format!("{}/logs", cowen_home);
    let out_content =
        std::fs::read_to_string(format!("{}/daemon.stdout.log", logs_dir)).unwrap_or_default();
    let err_content =
        std::fs::read_to_string(format!("{}/daemon.stderr.log", logs_dir)).unwrap_or_default();
    assert!(status.is_success(), "Expected success in recovery, got status: {}, headers: {}, body: {}\nStdout:\n{}\nStderr:\n{}", status, headers, text, out_content, err_content);
    assert!(
        text.contains("mock_at_user_mock_upc_from_exchange"),
        "Expected new token (mock_at), got: {}",
        text
    );

    // 3. Multiple Upstream Route
    let res = client
        .get(format!("{}/order/list", gateway_url))
        .header("Cookie", &cookie_header)
        .send()
        .await
        .unwrap();
    let text = res.text().await.unwrap();
    assert!(
        text.contains("Order Service"),
        "Expected Order Service, got: {}",
        text
    );
    assert!(
        text.contains("mock_org_456"),
        "Expected mock_org_456, got: {}",
        text
    );
    assert!(text.contains("/list"), "Expected /list, got: {}", text);

    let mut stop_cmd2 = Command::cargo_bin("cowen").unwrap();
    stop_cmd2.env("COWEN_HOME", &cowen_home);
    stop_cmd2.args(["daemon", "stop"]);
    let _ = stop_cmd2.output();
}
