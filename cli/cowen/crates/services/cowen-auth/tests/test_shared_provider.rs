use cowen_auth::lifecycle::AuthSessionManager;
use cowen_auth::pool::TokenPool;
use cowen_auth::provider::shared::execute_finalize_login;
use cowen_auth::VaultTokenPool;
use cowen_common::CowenError;
use std::sync::Arc;
use tokio::time::Duration;

mod common;

async fn setup_pool() -> Arc<dyn TokenPool> {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(cowen_store::FileStore::new(tmp.path().join("vault"), None).unwrap());
    let vault = Arc::new(common::MockVault::with_store(store));
    Arc::new(VaultTokenPool::new(vault))
}

#[tokio::test]
async fn test_execute_finalize_login_with_pre_captured_code() {
    let pool = setup_pool().await;
    let session_manager = AuthSessionManager::new(pool.as_ref());

    let profile = "test_shared_profile_1";
    let session = session_manager.create_session(profile, 0).await.unwrap();

    // Setup pre-captured code
    session_manager
        .save_code(profile, "pre_captured_code_123", &session.state)
        .await
        .unwrap();

    // Call shared provider logic
    let res = execute_finalize_login(
        pool.as_ref(),
        profile,
        &session.state,
        "test_provider",
        |code| async move {
            assert_eq!(code, "pre_captured_code_123");
            Ok(())
        },
    )
    .await;

    assert!(res.is_ok());
}

#[tokio::test]
async fn test_execute_finalize_login_with_listener_success() {
    let pool = setup_pool().await;
    let session_manager = AuthSessionManager::new(pool.as_ref());

    let profile = "test_shared_profile_2";
    let session = session_manager.create_session(profile, 0).await.unwrap();

    let pool_ref = pool.clone();
    let session_id = session.state.clone();

    // Spawn task to simulate hitting the callback
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;

        let session_manager = AuthSessionManager::new(pool_ref.as_ref());
        let session = session_manager.get_session(&session_id).await.unwrap();
        let port = session.redirect_port;

        // Ensure port is assigned
        assert!(port > 0);

        let client = reqwest::Client::new();
        let _ = client
            .get(format!(
                "http://127.0.0.1:{}/callback?code=mock_code_abc&state={}",
                port, session.state
            ))
            .send()
            .await;
    });

    let res = execute_finalize_login(
        pool.as_ref(),
        profile,
        &session.state,
        "test_provider",
        |code| async move {
            assert_eq!(code, "mock_code_abc");
            Ok(())
        },
    )
    .await;

    assert!(res.is_ok());

    // Ensure code is saved to pool
    let saved_code = session_manager.get_captured_code(profile).await.unwrap();
    assert_eq!(saved_code, "mock_code_abc");
}

#[tokio::test]
async fn test_execute_finalize_login_exchange_error() {
    let pool = setup_pool().await;
    let session_manager = AuthSessionManager::new(pool.as_ref());

    let profile = "test_shared_profile_3";
    let session = session_manager.create_session(profile, 0).await.unwrap();

    let pool_ref = pool.clone();
    let session_id = session.state.clone();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let session_manager = AuthSessionManager::new(pool_ref.as_ref());
        let session = session_manager.get_session(&session_id).await.unwrap();
        let port = session.redirect_port;

        let client = reqwest::Client::new();
        let _ = client
            .get(format!(
                "http://127.0.0.1:{}/callback?code=mock_code_xyz&state={}",
                port, session.state
            ))
            .send()
            .await;
    });

    let res = execute_finalize_login(
        pool.as_ref(),
        profile,
        &session.state,
        "test_provider",
        |_| async move { Err(CowenError::Auth("simulated exchange error".to_string())) },
    )
    .await;

    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("simulated exchange error"));

    // When error occurs, it should clear the session
    let get_session = session_manager.get_session(&session.state).await;
    assert!(get_session.is_err());
}
