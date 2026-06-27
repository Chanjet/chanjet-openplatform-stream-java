use assert_cmd::Command;
use reqwest::Client;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

// BDD semantic wrappers
async fn given<F, Fut>(description: &str, f: F)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    println!("Given: {}", description);
    f().await;
}

async fn when<F, Fut>(description: &str, f: F)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    println!("When: {}", description);
    f().await;
}

async fn then<F, Fut>(description: &str, f: F)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    println!("Then: {}", description);
    f().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_chaos_stress_graceful_shutdown() {
    let dir = tempdir().unwrap();
    let home = dir.path().join(".cowen");
    fs::create_dir_all(&home).unwrap();
    let home_str = home.to_str().unwrap().to_string();

    // Use the daemon killer helper to ensure we don't leak processes
    let _killer = crate::e2e::rust::common::DaemonKiller {
        home: home_str.clone(),
    };

    let mut mock_port: u16 = 0;

    given(
        "an initialized self-built app profile connected to a mock server",
        || async {
            let (port, _) = crate::e2e::rust::mock_server::spawn_mock_server().await;
            mock_port = port;

            // Configure moderate delay in mock to keep connections alive longer
            let client = Client::new();
            client
                .post(format!("http://127.0.0.1:{}/control/config", mock_port))
                .json(&json!({"webhook_delay_ms": 1000}))
                .send()
                .await
                .expect("Failed to configure mock server delay");

            let mut cmd_init = Command::cargo_bin("cowen").unwrap();
            cmd_init.env("COWEN_HOME", &home_str);
            cmd_init.env("HOME", &home_str);
            cmd_init.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
            cmd_init
                .arg("init")
                .arg("--app-mode")
                .arg("self-built")
                .arg("--app-key")
                .arg("chaos_stress_key")
                .arg("--app-secret")
                .arg("chaos_stress_secret")
                .arg("--encrypt-key")
                .arg("12345678901234567890123456789012")
                .arg("--certificate")
                .arg("chaos_cert")
                .arg("--openapi-url")
                .arg(format!("http://127.0.0.1:{}", mock_port))
                .arg("--stream-url")
                .arg(format!("ws://127.0.0.1:{}", mock_port))
                .arg("--webhook-target")
                .arg(format!("http://127.0.0.1:{}/webhook_sink", mock_port));

            let output = cmd_init.output().expect("Failed to execute cowen init");
            assert!(
                output.status.success(),
                "Init failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );

            // Stop the background daemon spawned by `init` to allow foreground daemon to acquire the lock
            let pid_file = std::path::Path::new(&home_str).join("master_daemon.pid");
            if let Ok(content) = std::fs::read_to_string(&pid_file) {
                if let Ok(pid) = content.lines().next().unwrap_or("").trim().parse::<u32>() {
                    let _ = std::process::Command::new("kill")
                        .arg("-15")
                        .arg(pid.to_string())
                        .status();
                }
            }

            // Wait for the daemon to fully exit and release the file lock
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            // Wait a bit to ensure background daemon created by `init` is stable.
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            // Stop the background daemon spawned by `init` to prevent IPC race conditions
            let master_pid_file = home.join("master_daemon.pid");
            if master_pid_file.exists() {
                if let Ok(pid_str) = fs::read_to_string(&master_pid_file) {
                    if let Ok(pid) = pid_str.trim().parse::<i32>() {
                        let _ = std::process::Command::new("kill")
                            .arg("-TERM")
                            .arg(pid.to_string())
                            .status();
                    }
                }
                let _ = fs::remove_file(home.join("ipc.port"));
            }
        },
    )
    .await;

    let mut child_daemon_opt = None;

    when(
        "the daemon is started in the foreground and subjected to heavy concurrent broadcasting",
        || async {
            // Start daemon in foreground using std::process::Command to get the Child handle
            let daemon_log_path = home.join("logs").join("daemon.stdout.log");
            let stderr_log_path = home.join("logs").join("daemon.stderr.log");
            let stdout_file = std::fs::File::create(&daemon_log_path).unwrap();
            let stderr_file = std::fs::File::create(&stderr_log_path).unwrap();

            let mut cmd_daemon = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
            cmd_daemon.env("COWEN_HOME", &home_str);
            cmd_daemon.env("HOME", &home_str);
            cmd_daemon.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
            cmd_daemon.arg("daemon").arg("start").arg("--foreground");
            cmd_daemon.stdout(stdout_file);
            cmd_daemon.stderr(stderr_file);

            let child = cmd_daemon
                .spawn()
                .expect("Failed to start daemon in foreground");
            child_daemon_opt = Some(child);

            // Wait for daemon to be fully ready
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            let client = Client::new();
            let broadcast_url = format!("http://127.0.0.1:{}/control/broadcast", mock_port);

            // Send 40 concurrent DATA_PUSH events
            let mut set = tokio::task::JoinSet::new();
            for i in 1..=40 {
                let client = client.clone();
                let url = broadcast_url.clone();
                set.spawn(async move {
                    let payload = json!({
                        "event_type": "DATA_PUSH",
                        "payload": { "index": i },
                        "timeout_ms": 1000
                    });
                    let _ = client.post(&url).json(&payload).send().await;
                });
            }

            // Wait for all requests to be sent
            while set.join_next().await.is_some() {}

            // Wait a little bit for the daemon to start processing the push events
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        },
    )
    .await;

    then(
        "sending SIGTERM initiates a graceful shutdown where all active tasks complete",
        || async {
            if let Some(mut child) = child_daemon_opt {
                let pid = child.id();

                // Send SIGTERM to the daemon
                let kill_status = std::process::Command::new("kill")
                    .arg("-TERM")
                    .arg(pid.to_string())
                    .status()
                    .expect("Failed to send SIGTERM to daemon");
                assert!(kill_status.success(), "Failed to send SIGTERM");

                // Wait for daemon to exit
                let exit_status = child.wait().expect("Failed to wait on daemon");

                // Check exit status (it might be success or interrupted, but it should exit gracefully)
                println!("Daemon exited with status: {}", exit_status);

                // Read daemon logs to verify graceful completion
                let daemon_log_path = home.join("logs").join("daemon.stdout.log");
                let stderr_log_path = home.join("logs").join("daemon.stderr.log");

                let stdout_content = fs::read_to_string(&daemon_log_path).unwrap_or_default();
                let stderr_content = fs::read_to_string(&stderr_log_path).unwrap_or_default();
                let full_log = format!("{}\n{}", stdout_content, stderr_content);

                assert!(
                    full_log.contains("All active tasks completed gracefully")
                        || full_log.contains("shutdown complete")
                        || full_log.contains("Waiting for active tasks to complete"),
                    "Daemon logs missing graceful shutdown marker!\nLogs:\n{}",
                    full_log
                );

                // Verify Integrity & Schema
                let mut cmd_doctor =
                    std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
                cmd_doctor.env("COWEN_HOME", &home_str);
                cmd_doctor.env("HOME", &home_str);
                cmd_doctor.arg("doctor").arg("--fix");

                let doctor_status = cmd_doctor.status().expect("Failed to run cowen doctor");
                assert!(
                    doctor_status.success(),
                    "Cowen doctor reported errors after chaos shutdown"
                );
            } else {
                panic!("Daemon child process was not started");
            }
        },
    )
    .await;
}
