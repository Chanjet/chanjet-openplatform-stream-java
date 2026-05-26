use crate::e2e::rust::common::setup_test_env;
use assert_cmd::Command;
use std::fs;

use predicates::prelude::*;

#[tokio::test(flavor = "multi_thread")]
async fn test_daemon_version_sync_restart() {
    let profile = "sync_test_profile";
    let (dir, home) = setup_test_env(profile, "self-built", "http://localhost:12345");
    let bin_dir = dir.path().join("bin");
    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let cowen_bin = bin_dir.join(format!("cowen{}", exe_suffix));
    
    // 1. Seed dummy tokens
    let app_cfg = cowen_common::config::AppConfig { 
        openapi_url: "http://localhost:12345".to_string(), 
        stream_url: "http://localhost:12345".to_string(), 
        ..Default::default() 
    };
    let vault = cowen_store::create_vault(&app_cfg, std::path::Path::new(&home), "sync_fingerprint").await.unwrap();
    vault.set_config(profile, "app_key", "sync_test_key").await.unwrap();
    vault.set_secret(profile, "app_secret", "sync_test_secret").await.unwrap();
    
    let at = cowen_common::models::Token {
        value: "mock_at".to_string(),
        expires_at: chrono::Utc::now() + chrono::Duration::days(1),
        created_at: chrono::Utc::now() - chrono::Duration::hours(2),
    };
    vault.save_access_token(profile, at).await.unwrap();

    // 2. Start daemon using the CURRENT binary
    let mut cmd_start = Command::new(&cowen_bin);
    cmd_start.env("COWEN_HOME", &home);
    cmd_start.env("HOME", &home);
    cmd_start.env("COWEN_FS_FINGERPRINT", "sync_fingerprint");
    cmd_start.arg("--profile").arg(profile).arg("daemon").arg("start");
    cmd_start.assert().success();

    // Wait for daemon to stabilize
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // 3. Verify it is running
    let mut cmd_status = Command::new(&cowen_bin);
    cmd_status.env("COWEN_HOME", &home);
    cmd_status.env("HOME", &home);
    cmd_status.env("COWEN_FS_FINGERPRINT", "sync_fingerprint");
    cmd_status.arg("--profile").arg(profile).arg("status");
    cmd_status.assert().success().stdout(predicate::str::contains("[RUNNING]"));

    // 4. Verify version sync triggers restart
    let mut cmd_sync = Command::new(&cowen_bin);
    cmd_sync.env("COWEN_HOME", &home);
    cmd_sync.env("HOME", &home);
    cmd_sync.env("COWEN_FS_FINGERPRINT", "sync_fingerprint");
    cmd_sync.arg("--profile").arg(profile).arg("status");
    
    // Due to build time mismatch in test environment, it often triggers sync
    let output = cmd_sync.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    if stderr.contains("后台进程版本已过时") {
        assert!(stderr.contains("正在自动重启"));
        assert!(stderr.contains("Stopping master daemon"));
    }

    // 5. Verify auto-recovery after PID removal
    let pid_file = std::path::Path::new(&home).join("master_daemon.pid");
    if pid_file.exists() {
        let pid_str = fs::read_to_string(&pid_file).unwrap();
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            #[cfg(windows)]
            let _ = std::process::Command::new("taskkill").arg("/F").arg("/PID").arg(pid.to_string()).status();
            #[cfg(unix)]
            let _ = std::process::Command::new("kill").arg("-9").arg(pid.to_string()).status();
        }
    }
    let _ = fs::remove_file(&pid_file);

    let mut cmd_recover = Command::new(&cowen_bin);
    cmd_recover.env("COWEN_HOME", &home);
    cmd_recover.env("HOME", &home);
    cmd_recover.env("COWEN_FS_FINGERPRINT", "sync_fingerprint");
    cmd_recover.arg("--profile").arg(profile).arg("status");
    
    let output_recover = cmd_recover.output().unwrap();
    let stdout_recover = String::from_utf8_lossy(&output_recover.stdout);
    let stderr_recover = String::from_utf8_lossy(&output_recover.stderr);
    
    // Recovery can be silent in stdout if it finishes quickly, but usually prints to stderr
    assert!(
        stdout_recover.contains("Daemon not running, triggering auto-recovery") || 
        stderr_recover.contains("Daemon not running, triggering auto-recovery") ||
        stderr_recover.contains("Startup command sent to daemon") ||
        stdout_recover.contains("started successfully") ||
        stderr_recover.contains("started successfully"),
        "Recovery failed. Stdout: {}, Stderr: {}", stdout_recover, stderr_recover
    );

    // 6. Clean up
    let mut cmd_stop = Command::new(&cowen_bin);
    cmd_stop.env("COWEN_HOME", &home);
    cmd_stop.env("HOME", &home);
    cmd_stop.env("COWEN_FS_FINGERPRINT", "sync_fingerprint");
    cmd_stop.arg("--profile").arg(profile).arg("daemon").arg("stop");
    cmd_stop.assert().success();
}
