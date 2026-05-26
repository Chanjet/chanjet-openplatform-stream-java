use crate::e2e::rust::common::{setup_test_env, start_mock_platform};
use assert_cmd::Command;
use std::fs;

use cowen_common::config::AppConfig;


#[tokio::test(flavor = "multi_thread")]
async fn test_oauth2_full_lifecycle_and_recovery() {
    let (addr, _server_handle) = start_mock_platform().await;
    let openapi_url = format!("http://{}", addr);

    let profile = "oa2_lifecycle";
    let (dir, home) = setup_test_env(profile, "oauth2", &openapi_url);
    let bin_dir = dir.path().join("bin");
    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let cowen_bin = bin_dir.join(format!("cowen{}", exe_suffix));

    // 1. Run 'cowen init' in background
    let mut child = std::process::Command::new(&cowen_bin)
        .env("COWEN_HOME", &home)
        .env("HOME", &home)
        .env("COWEN_FS_FINGERPRINT", "sync_fingerprint")
        .arg("--profile").arg(profile)
        .arg("init")
        .arg("--app-mode").arg("oauth2")
        .arg("--openapi-url").arg(&openapi_url)
        .arg("--stream-url").arg(&openapi_url)
        .spawn().unwrap();

    // 2. Wait for session to appear in DB
    let app_cfg = AppConfig { 
        openapi_url: openapi_url.clone(), 
        stream_url: openapi_url.clone(), 
        ..Default::default() 
    };
    
    let mut session = None;
    for _ in 0..80 { // 40 seconds max
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        if let Ok(vault) = cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "sync_fingerprint").await {
            if let Ok(sessions) = vault.list_sessions().await {
                if let Some(s) = sessions.into_iter().next() {
                    session = Some(s);
                    break;
                }
            }
        }
    }
    
    let session = session.expect("Timeout waiting for OAuth2 session to be created in DB");
    
    // 3. Simulate browser callback
    let callback_url = format!("http://127.0.0.1:{}/callback?code=mock_code_123&state={}", session.redirect_port, session.state);
    let client = reqwest::Client::new();
    
    let mut res = None;
    for _ in 0..20 {
        if let Ok(r) = client.get(&callback_url).send().await {
            res = Some(r);
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }
    
    let res = res.expect("Failed to connect to local callback server");
    assert!(res.status().is_success());

    // 4. Wait for 'init' to complete
    let status = child.wait().unwrap();
    assert!(status.success());

    // 5. Verify daemon can start
    let mut cmd_start = Command::new(&cowen_bin);
    cmd_start.env("COWEN_HOME", &home);
    cmd_start.env("HOME", &home);
    cmd_start.env("COWEN_FS_FINGERPRINT", "sync_fingerprint");
    cmd_start.arg("--profile").arg(profile).arg("daemon").arg("start");
    
    let output = cmd_start.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    assert!(
        stdout.contains("Startup command sent to daemon") || 
        stderr.contains("Startup command sent to daemon") ||
        stdout.contains("started successfully") ||
        stderr.contains("started successfully"),
        "Daemon should start. Stdout: {}, Stderr: {}", stdout, stderr
    );

    // Wait for it to be actually running
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // 6. Simulate crash (kill -9)
    let pid_file = std::path::Path::new(&home).join("master_daemon.pid");
    if pid_file.exists() {
        let pid_str = fs::read_to_string(&pid_file).unwrap();
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill").arg("-9").arg(pid.to_string()).status();
            }
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill").arg("/F").arg("/PID").arg(pid.to_string()).status();
            }
        }
    }
    
    // Wait for it to be gone
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // 7. Verify recovery / restart works
    let mut cmd_recover = Command::new(&cowen_bin);
    cmd_recover.env("COWEN_HOME", &home);
    cmd_recover.env("HOME", &home);
    cmd_recover.env("COWEN_FS_FINGERPRINT", "sync_fingerprint");
    cmd_recover.arg("--profile").arg(profile).arg("daemon").arg("start");
    
    let output_recover = cmd_recover.output().unwrap();
    let stdout_recover = String::from_utf8_lossy(&output_recover.stdout);
    let stderr_recover = String::from_utf8_lossy(&output_recover.stderr);
    
    assert!(
        stdout_recover.contains("Startup command sent to daemon") || 
        stdout_recover.contains("started successfully") ||
        stderr_recover.contains("Startup command sent to daemon") ||
        stderr_recover.contains("started successfully"),
        "Daemon should recover. Stdout: {}, Stderr: {}", stdout_recover, stderr_recover
    );

    // 8. Clean up
    let mut cmd_stop = Command::new(&cowen_bin);
    cmd_stop.env("COWEN_HOME", &home);
    cmd_stop.env("HOME", &home);
    cmd_stop.env("COWEN_FS_FINGERPRINT", "sync_fingerprint");
    cmd_stop.arg("--profile").arg(profile).arg("daemon").arg("stop");
    cmd_stop.assert().success();
    
    let _ = dir;
}
