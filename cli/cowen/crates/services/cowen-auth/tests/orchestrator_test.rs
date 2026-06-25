use cowen_auth::lifecycle::orchestrator;
use std::fs;

#[tokio::test]
async fn test_check_failure_log_growth() {
    let temp_dir = tempfile::tempdir().unwrap();
    let log_file = temp_dir.path().join("test_auth.log");

    let mut last_size = 0;
    // File not exist yet
    let has_failed = orchestrator::check_failure_log_growth(&log_file, &mut last_size).unwrap();
    assert!(!has_failed);

    // Create file with success log
    fs::write(&log_file, "INFO: Auth started\n").unwrap();
    let has_failed = orchestrator::check_failure_log_growth(&log_file, &mut last_size).unwrap();
    assert!(!has_failed);
    assert!(last_size > 0);

    // Append ERROR log
    fs::write(&log_file, "INFO: Auth started\nERROR: Auth failed\n").unwrap();
    let has_failed = orchestrator::check_failure_log_growth(&log_file, &mut last_size).unwrap();
    assert!(has_failed);
}

#[tokio::test]
async fn test_render_last_auth_error() {
    // Just ensure it does not panic if file does not exist
    let result = orchestrator::render_last_auth_error("non_existent_profile");
    assert!(result.is_ok());
}

use cowen_common::domain::{SessionDomain, TokenDomain};
use cowen_common::models::{AuthSession, Token};
use cowen_config::ConfigManager;
use std::sync::Arc;
use tokio::time::{advance, Duration};

mod common;

#[tokio::test]
async fn test_wait_for_token_exchange_success() {
    let vault = Arc::new(common::MockVault::new());
    let cfg_mgr = ConfigManager::new().unwrap();

    // Inject session so it doesn't fail immediately
    vault
        .save_session(AuthSession {
            profile: "test_profile".to_string(),
            redirect_port: 0,
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            state: "test_session".to_string(),
            code_verifier: "test".to_string(),
            redirect_uri: "test".to_string(),
        })
        .await
        .unwrap();

    // Spawn a background task to insert token after 2 seconds
    let vault_clone = vault.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        vault_clone
            .save_access_token(
                "test_profile",
                Token {
                    value: "test_token".to_string(),
                    expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
                    created_at: chrono::Utc::now(),
                },
            )
            .await
            .unwrap();
    });

    let result = orchestrator::wait_for_token_exchange(
        "test_profile",
        vault.clone(),
        12345,
        false,
        &cfg_mgr,
        "test_session",
    )
    .await;

    assert!(result.is_ok());
}

#[tokio::test(start_paused = true)]
async fn test_wait_for_token_exchange_timeout() {
    let vault = Arc::new(common::MockVault::new());
    let cfg_mgr = ConfigManager::new().unwrap();

    // Inject session
    vault
        .save_session(AuthSession {
            profile: "test_profile_timeout".to_string(),
            redirect_port: 0,
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            state: "test_session".to_string(),
            code_verifier: "test".to_string(),
            redirect_uri: "test".to_string(),
        })
        .await
        .unwrap();

    let vault_clone = vault.clone();
    let cfg_mgr_clone = cfg_mgr.clone();

    let handle = tokio::spawn(async move {
        orchestrator::wait_for_token_exchange(
            "test_profile_timeout",
            vault_clone,
            12345,
            false,
            &cfg_mgr_clone,
            "test_session",
        )
        .await
    });

    // Let the spawned task start and capture start_time
    tokio::time::sleep(Duration::from_millis(100)).await;
    // Advance time past 5 minutes
    advance(Duration::from_secs(301)).await;
    let result = handle.await.unwrap();
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "Authentication failed: Authorization timeout"
    );
}

#[tokio::test]
async fn test_spawn_finalizer() {
    // This will just execute the current test binary which does not have "auth login --finalize"
    // So it will exit immediately, but we will get the PID.
    let pid = orchestrator::spawn_finalizer("test_profile_spawn", "session_123");
    assert!(pid.is_ok());
    let pid = pid.unwrap();
    assert!(pid > 0);

    // Test failure cleanup killing this PID
    let vault = Arc::new(common::MockVault::new());
    let cfg_mgr = ConfigManager::new().unwrap();

    orchestrator::perform_failure_cleanup("test_profile_spawn", vault, pid, true, &cfg_mgr).await;
}

#[tokio::test]
async fn test_check_session_lost() {
    let vault = Arc::new(common::MockVault::new());
    let cfg_mgr = ConfigManager::new().unwrap();

    // Session doesn't exist, Token doesn't exist => should error
    let handle = tokio::spawn(async move {
        orchestrator::wait_for_token_exchange(
            "test_profile_lost",
            vault.clone(),
            12345,
            false,
            &cfg_mgr,
            "test_session_lost",
        )
        .await
    });

    let result = handle.await.unwrap();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Session lost"));
}
