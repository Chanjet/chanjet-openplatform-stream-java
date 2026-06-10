use crate::lifecycle::AuthSessionManager;
use cowen_common::{CowenError, CowenResult};
use std::future::Future;

/// Shared implementation for finalizer listener
pub async fn execute_finalize_login<F, Fut>(
    pool: &dyn crate::pool::TokenPool,
    profile: &str,
    session_id: &str,
    provider_name: &str,
    exchange_callback: F,
) -> CowenResult<()>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = CowenResult<()>>,
{
    tracing::info!(target: "sys", profile = %profile, session_id = %session_id, "Finalizer started for {} auth", provider_name);

    let session_manager = AuthSessionManager::new(pool);
    let session = session_manager.get_session(session_id).await?;

    // Check if code is already captured (pushed via Monitor API)
    if let Ok(captured_code) = session_manager.get_captured_code(profile).await {
        tracing::info!(target: "sys", "Using pre-captured code from IPC/Session");
        return exchange_callback(captured_code).await;
    }

    let (actual_port, rx) = crate::lifecycle::listener::OAuth2CallbackListener::start(
        session.redirect_port,
        profile.to_string(),
    )
    .await?;
    tracing::info!(target: "sys", port = %actual_port, "Finalizer listening for callback");

    let res = tokio::select! {
        result = rx => {
            match result {
                Ok(inner_res) => {
                    match inner_res {
                        Ok(res) => {
                            tracing::info!(target: "sys", "Callback received, saving code...");
                            session_manager.save_code(profile, &res.code, &res.state).await?;

                            // Trigger exchange
                            match exchange_callback(res.code).await {
                                Ok(_) => {
                                    tracing::info!(target: "sys", "Token exchange successful");
                                    Ok(())
                                }
                                Err(e) => {
                                    tracing::error!(target: "sys", error = %e, "Token exchange failed");
                                    Err(e)
                                }
                            }
                        }
                        Err(e) => Err(CowenError::Auth(format!("Authorization failed: {}", e)))
                    }
                }
                Err(e) => Err(CowenError::Auth(format!("Internal listener error: {}", e)))
            }
        },
        _ = tokio::signal::ctrl_c() => {
            Err(CowenError::Auth("Cancelled by user".to_string()))
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
            Err(CowenError::Auth("Timeout waiting for authorization (5 mins)".to_string()))
        }
    };

    if res.is_err() {
        let _ = session_manager.clear(profile).await;
    }
    res
}
