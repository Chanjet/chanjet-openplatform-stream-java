use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread")]
async fn test_sealed_storage() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().join(".cowen");
    let home = dir.path().join("home");
    fs::create_dir_all(&cowen_home).unwrap();
    fs::create_dir_all(&home).unwrap();

    let cowen_home_str = cowen_home.to_str().unwrap().to_string();
    let home_str = home.to_str().unwrap().to_string();

    let app_config_path = cowen_home.join("app.yaml");
    let app_config = serde_json::json!({
        "storage": {
            "store": "local"
        },
        "log": {
            "level": "debug"
        }
    });
    fs::write(app_config_path, serde_yaml::to_string(&app_config).unwrap()).unwrap();

    // Touch .seal file to activate MonolithicSealStore
    fs::write(cowen_home.join(".seal"), "").unwrap();

    let profile = "case_85_sealed";

    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &cowen_home_str);
    init_cmd.env("HOME", &home_str);
    init_cmd
        .arg("init")
        .arg("--profile")
        .arg(profile)
        .arg("--app-key")
        .arg("dummykey")
        .arg("--app-secret")
        .arg("dummysecret")
        .arg("--app-mode")
        .arg("self-built")
        .arg("--certificate")
        .arg("dummy_cert")
        .arg("--encrypt-key")
        .arg("supersecret");
    init_cmd.assert().success();

    assert!(cowen_home.join("vault").exists());
    assert!(cowen_home.join(".seal").exists());

    let mut cfg_set = Command::cargo_bin("cowen").unwrap();
    cfg_set.env("COWEN_HOME", &cowen_home_str);
    cfg_set.env("HOME", &home_str);
    cfg_set
        .arg("--profile")
        .arg(profile)
        .arg("config")
        .arg("set")
        .arg("webhook_target")
        .arg("http://localhost:9999");
    cfg_set.assert().success();

    let mut cfg_get = Command::cargo_bin("cowen").unwrap();
    cfg_get.env("COWEN_HOME", &cowen_home_str);
    cfg_get.env("HOME", &home_str);
    cfg_get
        .arg("--profile")
        .arg(profile)
        .arg("config")
        .arg("get")
        .arg("webhook_target");

    let get_out = String::from_utf8_lossy(&cfg_get.output().unwrap().stdout).to_string();
    assert!(get_out.contains("http://localhost:9999"));

    let mut sec_get = Command::cargo_bin("cowen").unwrap();
    sec_get.env("COWEN_HOME", &cowen_home_str);
    sec_get.env("HOME", &home_str);
    sec_get
        .arg("--profile")
        .arg(profile)
        .arg("config")
        .arg("get")
        .arg("encrypt_key");

    let sec_out = String::from_utf8_lossy(&sec_get.output().unwrap().stdout).to_string();
    assert!(sec_out.contains("supersecret"));

    let mut cfg_list = Command::cargo_bin("cowen").unwrap();
    cfg_list.env("COWEN_HOME", &cowen_home_str);
    cfg_list.env("HOME", &home_str);
    cfg_list
        .arg("--profile")
        .arg(profile)
        .arg("config")
        .arg("list");

    let list_out = String::from_utf8_lossy(&cfg_list.output().unwrap().stdout).to_string();
    assert!(list_out.contains("webhook_target"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_redis_shared_storage() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let redis_port = listener.local_addr().unwrap().port();
    drop(listener);

    let dir = tempdir().unwrap();
    let cowen_home_1 = dir.path().join("home_1");
    let cowen_home_2 = dir.path().join("home_2");
    fs::create_dir_all(&cowen_home_1).unwrap();
    fs::create_dir_all(&cowen_home_2).unwrap();

    let mut redis_cmd = std::process::Command::new("redis-server");
    redis_cmd
        .arg("--port")
        .arg(redis_port.to_string())
        .arg("--dir")
        .arg(dir.path().to_str().unwrap())
        .arg("--save")
        .arg("");

    let mut redis_child = redis_cmd.spawn().expect("Failed to start redis-server");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let (mock_port, _) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}/connect", mock_port);
    let proxy_port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();

    let redis_url = format!("redis://127.0.0.1:{}/0", redis_port);

    let app_config = serde_json::json!({
        "storage": {
            "store": "redis",
            "db_url": redis_url,
        },
        "log": {
            "level": "debug"
        },
        "telemetry_enabled": false,
        "ai_enabled": false
    });

    fs::write(
        cowen_home_1.join("app.yaml"),
        serde_yaml::to_string(&app_config).unwrap(),
    )
    .unwrap();

    let app_yaml_2 = format!(
        "storage:\n  store: redis\n  db_url: \"{}\"\nlog:\n  level: debug\nopenapi_url: \"{}\"\nstream_url: \"{}\"\ntelemetry_enabled: false\nai_enabled: false\n",
        redis_url, mock_url, mock_ws
    );
    std::fs::write(cowen_home_2.join("app.yaml"), app_yaml_2).unwrap();

    let profile = "main";
    let cowen_home_1_str = cowen_home_1.to_str().unwrap().to_string();
    let cowen_home_2_str = cowen_home_2.to_str().unwrap().to_string();
    let home_str = dir.path().to_str().unwrap().to_string();

    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &cowen_home_1_str);
    init_cmd.env("HOME", &home_str);
    init_cmd
        .arg("init")
        .arg("--profile")
        .arg(profile)
        .arg("--app-mode")
        .arg("self-built")
        .arg("--app-key")
        .arg("AK_REDIS")
        .arg("--app-secret")
        .arg("AS_REDIS")
        .arg("--certificate")
        .arg("CERT_REDIS")
        .arg("--encrypt-key")
        .arg("1234567890123456")
        .arg("--openapi-url")
        .arg(&mock_url)
        .arg("--stream-url")
        .arg(&mock_ws)
        .arg("--webhook-target")
        .arg(format!("{}/webhook_sink", mock_url))
        .arg("--proxy-port")
        .arg(proxy_port.to_string());

    init_cmd.assert().success();

    let mut daemon_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd.env("COWEN_HOME", &cowen_home_1_str);
    daemon_cmd.env("HOME", &home_str);
    daemon_cmd
        .arg("daemon")
        .arg("start")
        .arg("--profile")
        .arg(profile)
        .arg("--foreground")
        .arg("--log-level=debug");
    daemon_cmd.stdout(std::process::Stdio::inherit());
    daemon_cmd.stderr(std::process::Stdio::inherit());

    let mut daemon_child = daemon_cmd.spawn().expect("Failed to start daemon");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Send APP_TICKET to trigger auth
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "msgType": "APP_TICKET",
        "msg_type": "APP_TICKET",
        "appKey": "AK_REDIS",
        "bizContent": {
            "appTicket": "ticket_redis_123"
        },
        "time": "2026-06-27 12:00:00"
    });
    let res = client
        .post(format!("{}/control/broadcast", mock_url))
        .json(&payload)
        .send()
        .await
        .expect("Failed to broadcast APP_TICKET");

    println!("Broadcast response: {:?}", res.text().await);

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Get token from Node 1
    let mut token_1 = String::new();
    for _ in 0..5 {
        let mut get_token_cmd = Command::cargo_bin("cowen").unwrap();
        get_token_cmd.env("COWEN_HOME", &cowen_home_1_str);
        get_token_cmd.env("HOME", &home_str);
        get_token_cmd
            .arg("auth")
            .arg("token")
            .arg("--profile")
            .arg(profile)
            .arg("--format")
            .arg("json");
        let output = get_token_cmd.output().unwrap();
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout).to_string();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&out_str) {
                if let Some(t) = json.get("access_token").and_then(|v| v.as_str()) {
                    token_1 = t.to_string();
                    break;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    assert!(!token_1.is_empty(), "Failed to get token on Node 1");

    // Get token from Node 2 (No init, no daemon, just CLI)
    let mut get_token_cmd_2 = Command::cargo_bin("cowen").unwrap();
    get_token_cmd_2.env("COWEN_HOME", &cowen_home_2_str);
    get_token_cmd_2.env("HOME", &home_str);
    get_token_cmd_2
        .arg("auth")
        .arg("token")
        .arg("--profile")
        .arg(profile)
        .arg("--format")
        .arg("json");
    let output_2 = get_token_cmd_2.output().unwrap();
    assert!(output_2.status.success(), "Failed to get token on Node 2");

    let out_str_2 = String::from_utf8_lossy(&output_2.stdout).to_string();
    let json_2 = serde_json::from_str::<serde_json::Value>(&out_str_2).unwrap();
    let token_2 = json_2
        .get("access_token")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    assert_eq!(
        token_1, token_2,
        "Node 2 did not retrieve the same token from Redis"
    );

    // Clear Redis key
    let mut redis_cli = std::process::Command::new("redis-cli");
    redis_cli
        .arg("-p")
        .arg(redis_port.to_string())
        .arg("DEL")
        .arg("app:AK_REDIS:tok_v2:app_access");
    redis_cli.output().expect("Failed to delete redis key");

    // Get token from Node 2 again, should renew
    let mut get_token_cmd_3 = Command::cargo_bin("cowen").unwrap();
    get_token_cmd_3.env("COWEN_HOME", &cowen_home_2_str);
    get_token_cmd_3.env("HOME", &home_str);
    get_token_cmd_3
        .arg("auth")
        .arg("token")
        .arg("--profile")
        .arg(profile)
        .arg("--format")
        .arg("json");
    let output_3 = get_token_cmd_3.output().unwrap();
    assert!(
        output_3.status.success(),
        "Failed to get token on Node 2 after delete"
    );

    let out_str_3 = String::from_utf8_lossy(&output_3.stdout).to_string();
    let err_str_3 = String::from_utf8_lossy(&output_3.stderr).to_string();
    println!("Node 2 token stdout: {}", out_str_3);
    println!("Node 2 token stderr: {}", err_str_3);
    let json_3 = serde_json::from_str::<serde_json::Value>(&out_str_3).unwrap();
    let token_3 = json_3
        .get("access_token")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    assert_ne!(token_1, token_3, "Token did not renew after redis deletion");
    assert!(!token_3.is_empty());

    {
        #[cfg(unix)]
        let _ = std::process::Command::new("kill")
            .arg("-15")
            .arg(daemon_child.id().to_string())
            .status();
        #[cfg(windows)]
        let _ = daemon_child.kill();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = daemon_child.wait();
    {
        #[cfg(unix)]
        let _ = std::process::Command::new("kill")
            .arg("-15")
            .arg(redis_child.id().to_string())
            .status();
        #[cfg(windows)]
        let _ = redis_child.kill();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = redis_child.wait();
}
#[tokio::test(flavor = "multi_thread")]
async fn test_redis_fault_tolerance() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let redis_port = listener.local_addr().unwrap().port();
    drop(listener);

    let dir = tempdir().unwrap();
    let cowen_home = dir.path().join("home_fault");
    fs::create_dir_all(&cowen_home).unwrap();
    let cowen_home_str = cowen_home.to_str().unwrap().to_string();
    let home_str = dir.path().to_str().unwrap().to_string();

    let mut redis_cmd = std::process::Command::new("redis-server");
    redis_cmd
        .arg("--port")
        .arg(redis_port.to_string())
        .arg("--dir")
        .arg(&cowen_home_str)
        .arg("--save")
        .arg("");

    let mut redis_child = redis_cmd.spawn().expect("Failed to start redis-server");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let (mock_port, _) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}/connect", mock_port);
    let proxy_port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();

    let redis_url = format!("redis://127.0.0.1:{}/0", redis_port);

    let app_config = serde_json::json!({
        "storage": {
            "store": "sqlite",
            "db_url": format!("sqlite://{}/persistence.db", cowen_home_str),
            "cache": "redis",
            "cache_url": redis_url,
        },
        "log": {
            "level": "debug"
        },
        "telemetry_enabled": false,
        "ai_enabled": false
    });

    fs::write(
        cowen_home.join("app.yaml"),
        serde_yaml::to_string(&app_config).unwrap(),
    )
    .unwrap();

    let profile = "redis_hybrid";

    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &cowen_home_str);
    init_cmd.env("HOME", &home_str);
    init_cmd
        .arg("init")
        .arg("--profile")
        .arg(profile)
        .arg("--app-mode")
        .arg("self-built")
        .arg("--app-key")
        .arg("AK_FAULT")
        .arg("--app-secret")
        .arg("AS_FAULT")
        .arg("--certificate")
        .arg("CERT_FAULT")
        .arg("--encrypt-key")
        .arg("1234567890123456")
        .arg("--openapi-url")
        .arg(&mock_url)
        .arg("--stream-url")
        .arg(&mock_ws)
        .arg("--webhook-target")
        .arg(format!("{}/webhook_sink", mock_url))
        .arg("--proxy-port")
        .arg(proxy_port.to_string());

    init_cmd.assert().success();

    let mut daemon_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd.env("COWEN_HOME", &cowen_home_str);
    daemon_cmd.env("HOME", &home_str);
    daemon_cmd
        .arg("daemon")
        .arg("start")
        .arg("--profile")
        .arg(profile)
        .arg("--foreground")
        .arg("--log-level=debug");
    daemon_cmd.stdout(std::process::Stdio::inherit());
    daemon_cmd.stderr(std::process::Stdio::inherit());

    let mut daemon_child = daemon_cmd.spawn().expect("Failed to start daemon");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Send APP_TICKET to trigger auth
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "msgType": "APP_TICKET",
        "msg_type": "APP_TICKET",
        "appKey": "AK_FAULT",
        "bizContent": {
            "appTicket": "ticket_fault_123"
        },
        "time": "2026-06-27 12:00:00"
    });
    let res = client
        .post(format!("{}/control/broadcast", mock_url))
        .json(&payload)
        .send()
        .await
        .expect("Failed to broadcast APP_TICKET");

    println!("Broadcast response: {:?}", res.text().await);
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Get token
    let mut token_1 = String::new();
    for _ in 0..5 {
        let mut get_token_cmd = Command::cargo_bin("cowen").unwrap();
        get_token_cmd.env("COWEN_HOME", &cowen_home_str);
        get_token_cmd.env("HOME", &home_str);
        get_token_cmd
            .arg("auth")
            .arg("token")
            .arg("--profile")
            .arg(profile)
            .arg("--format")
            .arg("json");
        let output = get_token_cmd.output().unwrap();
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout).to_string();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&out_str) {
                if let Some(t) = json.get("access_token").and_then(|v| v.as_str()) {
                    token_1 = t.to_string();
                    break;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    assert!(!token_1.is_empty(), "Failed to get initial token");

    // Stop Redis
    {
        #[cfg(unix)]
        let _ = std::process::Command::new("kill")
            .arg("-15")
            .arg(redis_child.id().to_string())
            .status();
        #[cfg(windows)]
        let _ = redis_child.kill();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = redis_child.wait();
    let _ = redis_child.wait();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Get Token again
    let mut get_token_cmd_2 = Command::cargo_bin("cowen").unwrap();
    get_token_cmd_2.env("COWEN_HOME", &cowen_home_str);
    get_token_cmd_2.env("HOME", &home_str);
    get_token_cmd_2
        .arg("auth")
        .arg("token")
        .arg("--profile")
        .arg(profile)
        .arg("--format")
        .arg("json");
    let output_2 = get_token_cmd_2.output().unwrap();
    assert!(
        output_2.status.success(),
        "Failed to get token with Redis down"
    );

    let out_str_2 = String::from_utf8_lossy(&output_2.stdout).to_string();
    let json_2 = serde_json::from_str::<serde_json::Value>(&out_str_2).unwrap();
    let token_2 = json_2
        .get("access_token")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    assert!(!token_2.is_empty(), "Token 2 is empty");

    // Start Redis again
    let mut redis_cmd_2 = std::process::Command::new("redis-server");
    redis_cmd_2
        .arg("--port")
        .arg(redis_port.to_string())
        .arg("--dir")
        .arg(&cowen_home_str)
        .arg("--save")
        .arg("");

    let mut redis_child_2 = redis_cmd_2.spawn().expect("Failed to restart redis-server");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Restart daemon
    {
        #[cfg(unix)]
        let _ = std::process::Command::new("kill")
            .arg("-15")
            .arg(daemon_child.id().to_string())
            .status();
        #[cfg(windows)]
        let _ = daemon_child.kill();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = daemon_child.wait();
    let _ = daemon_child.wait();

    let mut daemon_cmd_2 = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd_2.env("COWEN_HOME", &cowen_home_str);
    daemon_cmd_2.env("HOME", &home_str);
    daemon_cmd_2
        .arg("daemon")
        .arg("start")
        .arg("--profile")
        .arg(profile)
        .arg("--foreground")
        .arg("--log-level=debug");
    daemon_cmd_2.stdout(std::process::Stdio::inherit());
    daemon_cmd_2.stderr(std::process::Stdio::inherit());
    let mut daemon_child_2 = daemon_cmd_2.spawn().expect("Failed to restart daemon");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Get token again
    let mut get_token_cmd_3 = Command::cargo_bin("cowen").unwrap();
    get_token_cmd_3.env("COWEN_HOME", &cowen_home_str);
    get_token_cmd_3.env("HOME", &home_str);
    get_token_cmd_3
        .arg("auth")
        .arg("token")
        .arg("--profile")
        .arg(profile)
        .arg("--format")
        .arg("json");
    let output_3 = get_token_cmd_3.output().unwrap();
    assert!(
        output_3.status.success(),
        "Failed to get token after recovery"
    );
    let out_str_3 = String::from_utf8_lossy(&output_3.stdout).to_string();
    let json_3 = serde_json::from_str::<serde_json::Value>(&out_str_3).unwrap();
    let token_3 = json_3
        .get("access_token")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    assert!(!token_3.is_empty(), "Token 3 is empty");

    {
        #[cfg(unix)]
        let _ = std::process::Command::new("kill")
            .arg("-15")
            .arg(daemon_child_2.id().to_string())
            .status();
        #[cfg(windows)]
        let _ = daemon_child_2.kill();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = daemon_child_2.wait();
    {
        #[cfg(unix)]
        let _ = std::process::Command::new("kill")
            .arg("-15")
            .arg(redis_child_2.id().to_string())
            .status();
        #[cfg(windows)]
        let _ = redis_child_2.kill();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = redis_child_2.wait();
}
#[tokio::test(flavor = "multi_thread")]
async fn test_hybrid_data_drift() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let redis_port = listener.local_addr().unwrap().port();
    drop(listener);

    let dir = tempdir().unwrap();
    let cowen_home = dir.path().join("home_drift");
    fs::create_dir_all(&cowen_home).unwrap();
    let cowen_home_str = cowen_home.to_str().unwrap().to_string();
    let home_str = dir.path().to_str().unwrap().to_string();

    let mut redis_cmd = std::process::Command::new("redis-server");
    redis_cmd
        .arg("--port")
        .arg(redis_port.to_string())
        .arg("--dir")
        .arg(&cowen_home_str)
        .arg("--save")
        .arg("");

    let mut redis_child = redis_cmd.spawn().expect("Failed to start redis-server");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let (mock_port, _) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}/connect", mock_port);
    let proxy_port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();

    let redis_url = format!("redis://127.0.0.1:{}/0", redis_port);
    let db_path = cowen_home.join("persistence.db");

    let app_config = serde_json::json!({
        "storage": {
            "store": "sqlite",
            "db_url": format!("sqlite://{}", db_path.to_str().unwrap()),
            "cache": "redis",
            "cache_url": redis_url,
        },
        "log": {
            "level": "debug"
        },
        "telemetry_enabled": false,
        "ai_enabled": false
    });

    fs::write(
        cowen_home.join("app.yaml"),
        serde_yaml::to_string(&app_config).unwrap(),
    )
    .unwrap();

    let profile = "hybrid_drift";

    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &cowen_home_str);
    init_cmd.env("HOME", &home_str);
    init_cmd
        .arg("init")
        .arg("--profile")
        .arg(profile)
        .arg("--app-mode")
        .arg("self-built")
        .arg("--app-key")
        .arg("AK_HYBRID")
        .arg("--app-secret")
        .arg("AS_HYBRID")
        .arg("--certificate")
        .arg("CERT_HYBRID")
        .arg("--encrypt-key")
        .arg("1234567890123456")
        .arg("--openapi-url")
        .arg(&mock_url)
        .arg("--stream-url")
        .arg(&mock_ws)
        .arg("--webhook-target")
        .arg(format!("{}/webhook_sink", mock_url))
        .arg("--proxy-port")
        .arg(proxy_port.to_string());

    init_cmd.assert().success();

    let mut daemon_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd.env("COWEN_HOME", &cowen_home_str);
    daemon_cmd.env("HOME", &home_str);
    daemon_cmd
        .arg("daemon")
        .arg("start")
        .arg("--profile")
        .arg(profile)
        .arg("--foreground")
        .arg("--log-level=debug");
    daemon_cmd.stdout(std::process::Stdio::inherit());
    daemon_cmd.stderr(std::process::Stdio::inherit());

    let mut daemon_child = daemon_cmd.spawn().expect("Failed to start daemon");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Send APP_TICKET to trigger auth
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "msgType": "APP_TICKET",
        "msg_type": "APP_TICKET",
        "appKey": "AK_HYBRID",
        "bizContent": {
            "appTicket": "ticket_drift_123"
        },
        "time": "2026-06-27 12:00:00"
    });
    let _res = client
        .post(format!("{}/control/broadcast", mock_url))
        .json(&payload)
        .send()
        .await
        .expect("Failed to broadcast APP_TICKET");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Get Token 1
    let mut token_1 = String::new();
    for _ in 0..5 {
        let mut get_token_cmd = Command::cargo_bin("cowen").unwrap();
        get_token_cmd.env("COWEN_HOME", &cowen_home_str);
        get_token_cmd.env("HOME", &home_str);
        get_token_cmd
            .arg("auth")
            .arg("token")
            .arg("--profile")
            .arg(profile)
            .arg("--format")
            .arg("json");
        let output = get_token_cmd.output().unwrap();
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout).to_string();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&out_str) {
                if let Some(t) = json.get("access_token").and_then(|v| v.as_str()) {
                    token_1 = t.to_string();
                    break;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    assert!(!token_1.is_empty(), "Failed to get initial token");

    // Simulate Data Drift
    let drift_token = format!("drift_token_{}", chrono::Utc::now().timestamp());

    let mut sqlite_cmd = std::process::Command::new("sqlite3");
    sqlite_cmd.arg(db_path.to_str().unwrap());
    sqlite_cmd.arg(format!(
        "UPDATE cowen_token SET item_value='{}' WHERE profile='{}';",
        drift_token, profile
    ));

    let sqlite_output = sqlite_cmd.output().expect("Failed to run sqlite3 CLI");
    assert!(sqlite_output.status.success(), "sqlite3 update failed");

    // Get Token 2
    let mut get_token_cmd_2 = Command::cargo_bin("cowen").unwrap();
    get_token_cmd_2.env("COWEN_HOME", &cowen_home_str);
    get_token_cmd_2.env("HOME", &home_str);
    get_token_cmd_2
        .arg("auth")
        .arg("token")
        .arg("--profile")
        .arg(profile)
        .arg("--format")
        .arg("json");
    let output_2 = get_token_cmd_2.output().unwrap();
    assert!(output_2.status.success(), "Failed to get token after drift");
    let out_str_2 = String::from_utf8_lossy(&output_2.stdout).to_string();
    let json_2 = serde_json::from_str::<serde_json::Value>(&out_str_2).unwrap();
    let token_2 = json_2
        .get("access_token")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    // Verify Blind Spot
    assert_ne!(
        token_2, drift_token,
        "Data drift resolved?! That means the blind spot was fixed!"
    );
    assert_eq!(
        token_2, token_1,
        "Token should be served from stale Redis cache"
    );

    let mut list_cmd = Command::cargo_bin("cowen").unwrap();
    list_cmd.env("COWEN_HOME", &cowen_home_str);
    list_cmd.env("HOME", &home_str);
    list_cmd
        .arg("config")
        .arg("list")
        .arg("--profile")
        .arg("main");
    let list_out = String::from_utf8_lossy(&list_cmd.output().unwrap().stdout).to_string();
    assert!(!list_out.contains("AS_HYBRID")); // Sanity check

    {
        #[cfg(unix)]
        let _ = std::process::Command::new("kill")
            .arg("-15")
            .arg(daemon_child.id().to_string())
            .status();
        #[cfg(windows)]
        let _ = daemon_child.kill();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = daemon_child.wait();
    {
        #[cfg(unix)]
        let _ = std::process::Command::new("kill")
            .arg("-15")
            .arg(redis_child.id().to_string())
            .status();
        #[cfg(windows)]
        let _ = redis_child.kill();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = redis_child.wait();
}

fn setup_mysql_db(db_name: &str) -> Option<String> {
    if std::process::Command::new("mysql")
        .arg("--version")
        .output()
        .is_err()
    {
        return None;
    }

    let has_mysql_root = std::process::Command::new("mysql")
        .args(["-u", "root", "-h", "127.0.0.1", "-e", "select 1"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let has_mysql_root_password = if !has_mysql_root {
        std::process::Command::new("mysql")
            .args(["-u", "root", "-proot", "-h", "127.0.0.1", "-e", "select 1"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        false
    };

    if !has_mysql_root && !has_mysql_root_password {
        return None;
    }

    let mysql_url_base = if has_mysql_root {
        "mysql://root@127.0.0.1:3306"
    } else {
        "mysql://root:root@127.0.0.1:3306"
    };

    let mut drop_cmd = std::process::Command::new("mysql");
    drop_cmd.args(["-u", "root", "-h", "127.0.0.1"]);
    if has_mysql_root_password {
        drop_cmd.arg("-proot");
    }
    drop_cmd.arg("-e").arg(format!(
        "DROP DATABASE IF EXISTS {}; CREATE DATABASE {};",
        db_name, db_name
    ));

    let drop_res = drop_cmd.output().expect("Failed to execute mysql command");
    if !drop_res.status.success() {
        return None;
    }

    Some(format!("{}/{}", mysql_url_base, db_name))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mysql_shared_storage() {
    let mysql_url = match setup_mysql_db("case_31") {
        Some(url) => url,
        None => {
            println!("Skipping test_mysql_shared_storage: MySQL not available");
            return;
        }
    };

    let dir = tempdir().unwrap();
    let home_1 = dir.path().join("home_1");
    let home_2 = dir.path().join("home_2");
    std::fs::create_dir_all(&home_1).unwrap();
    std::fs::create_dir_all(&home_2).unwrap();

    let home_1_str = home_1.to_str().unwrap().to_string();
    let home_2_str = home_2.to_str().unwrap().to_string();

    let (mock_port, _) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}/connect", mock_port);
    let proxy_port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();

    let app_config = serde_json::json!({
        "storage": {
            "store": "mysql",
            "db_url": mysql_url
        },
        "log": {
            "level": "debug"
        },
        "telemetry_enabled": false,
        "ai_enabled": false
    });

    std::fs::write(
        home_1.join("app.yaml"),
        serde_yaml::to_string(&app_config).unwrap(),
    )
    .unwrap();
    std::fs::write(
        home_2.join("app.yaml"),
        serde_yaml::to_string(&app_config).unwrap(),
    )
    .unwrap();

    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &home_1_str);
    init_cmd.env("HOME", &home_1_str);
    init_cmd
        .arg("init")
        .arg("--profile")
        .arg("main")
        .arg("--app-mode")
        .arg("self-built")
        .arg("--app-key")
        .arg("AK_MYSQL")
        .arg("--app-secret")
        .arg("AS_MYSQL")
        .arg("--certificate")
        .arg("CERT_MYSQL")
        .arg("--encrypt-key")
        .arg("1234567890123456")
        .arg("--openapi-url")
        .arg(&mock_url)
        .arg("--stream-url")
        .arg(&mock_ws)
        .arg("--webhook-target")
        .arg(format!("{}/webhook_sink", mock_url))
        .arg("--proxy-port")
        .arg(proxy_port.to_string());

    init_cmd.assert().success();

    let mut daemon_cmd_1 = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd_1.env("COWEN_HOME", &home_1_str);
    daemon_cmd_1.env("HOME", &home_1_str);
    daemon_cmd_1
        .arg("daemon")
        .arg("start")
        .arg("--profile")
        .arg("main")
        .arg("--foreground");
    let mut daemon_child_1 = daemon_cmd_1.spawn().unwrap();

    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "msgType": "APP_TICKET",
        "msg_type": "APP_TICKET",
        "appKey": "AK_MYSQL",
        "bizContent": {
            "appTicket": "ticket_mysql_shared_123"
        },
        "time": "2026-06-27 12:00:00"
    });

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    let _ = client
        .post(format!("{}/control/broadcast", mock_url))
        .json(&payload)
        .send()
        .await;

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let mut daemon_cmd_2 = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd_2.env("COWEN_HOME", &home_2_str);
    daemon_cmd_2.env("HOME", &home_2_str);
    daemon_cmd_2
        .arg("daemon")
        .arg("start")
        .arg("--profile")
        .arg("default")
        .arg("--foreground");
    let mut daemon_child_2 = daemon_cmd_2.spawn().unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let mut status_cmd = Command::cargo_bin("cowen").unwrap();
    status_cmd.env("COWEN_HOME", &home_2_str);
    status_cmd.env("HOME", &home_2_str);
    status_cmd.arg("status").arg("--all");
    let status_out = String::from_utf8_lossy(&status_cmd.output().unwrap().stdout).to_string();
    assert!(
        status_out.contains("main"),
        "Node 2 did not discover main profile"
    );

    let mut token_cmd_1 = Command::cargo_bin("cowen").unwrap();
    token_cmd_1.env("COWEN_HOME", &home_1_str);
    token_cmd_1.env("HOME", &home_1_str);
    token_cmd_1
        .arg("auth")
        .arg("token")
        .arg("--profile")
        .arg("main")
        .arg("--format")
        .arg("json");
    let out_1 = String::from_utf8_lossy(&token_cmd_1.output().unwrap().stdout).to_string();
    let j_1 = serde_json::from_str::<serde_json::Value>(&out_1).unwrap();
    let t_1 = j_1
        .get("access_token")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    let mut token_cmd_2 = Command::cargo_bin("cowen").unwrap();
    token_cmd_2.env("COWEN_HOME", &home_2_str);
    token_cmd_2.env("HOME", &home_2_str);
    token_cmd_2
        .arg("auth")
        .arg("token")
        .arg("--profile")
        .arg("main")
        .arg("--format")
        .arg("json");
    let out_2 = String::from_utf8_lossy(&token_cmd_2.output().unwrap().stdout).to_string();
    let j_2 = serde_json::from_str::<serde_json::Value>(&out_2).unwrap();
    let t_2 = j_2
        .get("access_token")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    assert_eq!(t_1, t_2, "Tokens from both nodes should match");
    assert!(!t_1.is_empty(), "Token should not be empty");

    {
        #[cfg(unix)]
        let _ = std::process::Command::new("kill")
            .arg("-15")
            .arg(daemon_child_1.id().to_string())
            .status();
        #[cfg(windows)]
        let _ = daemon_child_1.kill();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = daemon_child_1.wait();
    {
        #[cfg(unix)]
        let _ = std::process::Command::new("kill")
            .arg("-15")
            .arg(daemon_child_2.id().to_string())
            .status();
        #[cfg(windows)]
        let _ = daemon_child_2.kill();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let _ = daemon_child_2.wait();
}
