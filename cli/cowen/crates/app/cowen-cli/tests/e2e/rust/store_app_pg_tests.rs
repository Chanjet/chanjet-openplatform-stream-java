use assert_cmd::Command;
use std::fs;
use std::process::Command as StdCommand;
use tempfile::tempdir;
use tokio::time::{sleep, Duration};

use super::mock_server::spawn_mock_server;

// Function to setup a Postgres database for the test. Returns the PG_URL if successful, or None if PG is unavailable.
fn setup_postgres_db(db_name: &str) -> Option<String> {
    // Check if psql is available
    if StdCommand::new("psql").arg("--version").output().is_err() {
        return None;
    }

    // Try connecting with "postgres" user
    let has_postgres_user = StdCommand::new("psql")
        .args([
            "-h",
            "localhost",
            "-U",
            "postgres",
            "-d",
            "postgres",
            "-c",
            "select 1",
        ])
        .env("PGPASSWORD", "password")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let has_default_user = if !has_postgres_user {
        StdCommand::new("psql")
            .args(["-h", "localhost", "-d", "postgres", "-c", "select 1"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        false
    };

    if !has_postgres_user && !has_default_user {
        return None; // Cannot connect
    }

    let (user_args, pg_url_base) = if has_postgres_user {
        (
            vec!["-U", "postgres"],
            "postgres://postgres:password@localhost:5432",
        )
    } else {
        (vec![], "postgres://localhost:5432")
    };

    // Drop DB if exists
    let mut drop_cmd = StdCommand::new("psql");
    drop_cmd.args(["-h", "localhost", "-d", "postgres"]);
    for arg in &user_args {
        drop_cmd.arg(arg);
    }
    if has_postgres_user {
        drop_cmd.env("PGPASSWORD", "password");
    }
    drop_cmd
        .arg("-c")
        .arg(format!("DROP DATABASE IF EXISTS {};", db_name));
    let _ = drop_cmd.status();

    // Create DB
    let mut create_cmd = StdCommand::new("psql");
    create_cmd.args(["-h", "localhost", "-d", "postgres"]);
    for arg in &user_args {
        create_cmd.arg(arg);
    }
    if has_postgres_user {
        create_cmd.env("PGPASSWORD", "password");
    }
    create_cmd
        .arg("-c")
        .arg(format!("CREATE DATABASE {};", db_name));
    if !create_cmd.status().unwrap().success() {
        return None;
    }

    Some(format!("{}/{}?sslmode=disable", pg_url_base, db_name))
}

fn query_ticket_from_pg(db_name: &str, app_key: &str) -> Option<String> {
    let mut cmd = StdCommand::new("psql");
    cmd.args([
        "-h",
        "localhost",
        "-d",
        db_name,
        "-t",
        "-c",
        &format!(
            "SELECT ticket_value FROM cowen_ticket WHERE app_key = '{}';",
            app_key
        ),
    ]);
    let output = cmd.output().unwrap();
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let val = stdout.trim();
        if !val.is_empty() {
            return Some(val.to_string());
        }
    }
    None
}

fn setup_store_app_node(
    cowen_home: &std::path::Path,
    openapi_url: &str,
    stream_url: &str,
    pg_url: &str,
) {
    fs::create_dir_all(cowen_home).unwrap();
    let app_config_path = cowen_home.join("app.yaml");
    let app_config = serde_json::json!({
        "openapi_url": openapi_url,
        "stream_url": stream_url,
        "telemetry_enabled": false,
        "log": {
            "level": "info",
        },
        "storage": {
            "store": "postgres",
            "db_url": pg_url
        }
    });
    fs::write(app_config_path, serde_yaml::to_string(&app_config).unwrap()).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_store_app_pg_ticket_persistence() {
    let db_name = format!(
        "case_35_rust_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    let pg_url = match setup_postgres_db(&db_name) {
        Some(url) => url,
        None => {
            println!("PostgreSQL not available. Skipping test_store_app_pg_ticket_persistence.");
            return;
        }
    };

    let (port, _state) = spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", port);
    let mock_ws = format!("ws://127.0.0.1:{}", port);
    let webhook_target = format!("{}/webhook_sink", mock_url);

    let dir = tempdir().unwrap();
    let home_1 = dir.path().join("node_1");
    let home_2 = dir.path().join("node_2");

    setup_store_app_node(&home_1, &mock_url, &mock_ws, &pg_url);

    // Init Node 1
    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &home_1);
    init_cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    init_cmd
        .arg("init")
        .arg("--profile")
        .arg("main")
        .arg("--app-mode")
        .arg("store-app")
        .arg("--app-key")
        .arg("AK_PG_STORE")
        .arg("--app-secret")
        .arg("AS_PG_STORE")
        .arg("--encrypt-key")
        .arg("1234567890123456")
        .arg("--webhook-target")
        .arg(&webhook_target)
        .arg("--openapi-url")
        .arg(&mock_url)
        .arg("--stream-url")
        .arg(&mock_ws);

    init_cmd.assert().success();

    let mut daemon_cmd = tokio::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd.env("COWEN_HOME", &home_1);
    daemon_cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");
    daemon_cmd
        .arg("daemon")
        .arg("start")
        .arg("--profile")
        .arg("main")
        .arg("--foreground");
    let mut child = daemon_cmd.spawn().unwrap();

    sleep(Duration::from_secs(3)).await;

    // Trigger AppTicket Push
    let client = reqwest::Client::new();
    client
        .post(format!("{}/auth/appTicket/resend", mock_url))
        .header("appKey", "AK_PG_STORE")
        .send()
        .await
        .unwrap();

    // Verify ticket is in PG
    let mut ticket_in_db = None;
    for _ in 0..15 {
        if let Some(t) = query_ticket_from_pg(&db_name, "AK_PG_STORE") {
            ticket_in_db = Some(t);
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }
    let ticket_in_db = ticket_in_db.expect("AppTicket not found in Postgres");

    // Stop daemon
    let pid = child.id().unwrap();
    let kill_status = StdCommand::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
        .expect("Failed to send SIGTERM to daemon");
    assert!(kill_status.success(), "Failed to send SIGTERM");
    let _ = child.wait().await;
    sleep(Duration::from_secs(1)).await;

    // Verify persistence
    let ticket_after_stop =
        query_ticket_from_pg(&db_name, "AK_PG_STORE").expect("Ticket lost after stop");
    assert_eq!(
        ticket_in_db, ticket_after_stop,
        "Ticket changed after daemon stop"
    );

    // Setup Node 2
    setup_store_app_node(&home_2, &mock_url, &mock_ws, &pg_url);

    // Node 2 attempts to get token
    // Since ticket is in PG, cowen token get will succeed because it reads the shared PG storage
    let mut token_cmd = Command::cargo_bin("cowen").unwrap();
    token_cmd.env("COWEN_HOME", &home_2);
    token_cmd.env("COWEN_FS_FINGERPRINT", "test_fingerprint");

    token_cmd
        .arg("auth")
        .arg("token")
        .arg("--profile")
        .arg("main");

    let output = token_cmd.output().unwrap();
    assert!(
        output.status.success(),
        "Node 2 failed to get token: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let token2 = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(!token2.is_empty());
}
