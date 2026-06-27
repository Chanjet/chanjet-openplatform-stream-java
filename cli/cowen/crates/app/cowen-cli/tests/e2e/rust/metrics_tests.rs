use reqwest::Client;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::zombie_processes)]
async fn test_metrics_health() {
    let (mock_port, _) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}/connect", mock_port);

    let home = tempfile::tempdir().unwrap();
    let home_path = home.path().to_str().unwrap();

    let mut init_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd.env("COWEN_HOME", home_path);
    init_cmd.args([
        "init",
        "--profile",
        "main",
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_SB",
        "--app-secret",
        "AS_SB",
        "--certificate",
        "CERT_SB",
        "--encrypt-key",
        "1234567890123456",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--webhook-target",
        "http://127.0.0.1:8080",
        "--no-telemetry",
        "--proxy-port",
        "0",
    ]);
    assert!(init_cmd.status().unwrap().success(), "Init failed");

    // Stop the background daemon spawned by `init` to allow foreground daemon to acquire the lock
    let pid_file = home.path().join("master_daemon.pid");
    if let Ok(content) = std::fs::read_to_string(&pid_file) {
        if let Ok(pid) = content.lines().next().unwrap_or("").trim().parse::<u32>() {
            let _ = std::process::Command::new("kill")
                .arg("-15")
                .arg(pid.to_string())
                .status();
        }
    }
    // Wait for the daemon to fully exit and release the file lock
    std::thread::sleep(std::time::Duration::from_millis(500));

    let mut daemon_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd.env("COWEN_HOME", home_path);
    daemon_cmd.args(["daemon", "start", "--profile", "main", "--foreground"]);
    let mut child = daemon_cmd.spawn().unwrap();

    // Wait for the master_daemon.pid to be written with the monitor port
    tokio::time::sleep(Duration::from_secs(3)).await;

    let pid_file = home.path().join("master_daemon.pid");
    let content = std::fs::read_to_string(&pid_file).expect("Failed to read pid file");

    let mut monitor_port = 0;
    for line in content.lines() {
        if line.starts_with("MONITOR_PORT=") {
            monitor_port = line.trim_start_matches("MONITOR_PORT=").parse().unwrap();
            break;
        }
    }
    assert!(
        monitor_port > 0,
        "Could not extract MONITOR_PORT from pid file"
    );

    let client = Client::new();

    // Check /health
    let health_resp = client
        .get(format!("http://127.0.0.1:{}/health", monitor_port))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(health_resp, "UP", "Health endpoint did not return UP");

    // Check /metrics
    let metrics_resp = client
        .get(format!("http://127.0.0.1:{}/metrics", monitor_port))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        metrics_resp.contains("cowen_active_connections"),
        "Metrics missing active_connections"
    );

    child.kill().ok();
    child.wait().ok();
}
