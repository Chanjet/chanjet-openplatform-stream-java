use axum::{
    extract::{Query, State},
    response::Html,
    routing::get,
    Router,
};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

pub struct CallbackResult {
    pub code: String,
    pub state: String,
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

type SharedState = Arc<Mutex<Option<oneshot::Sender<Result<CallbackResult, String>>>>>;
type ShutdownState = Arc<Mutex<Option<oneshot::Sender<()>>>>;

pub struct OAuth2CallbackListener;

// Recompile trigger to refresh embedded success.html
impl OAuth2CallbackListener {
    pub async fn start(port: u16, profile: String) -> anyhow::Result<(u16, oneshot::Receiver<Result<CallbackResult, String>>)> {
        let (result_tx, result_rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        crate::core::network::validate_loopback_addr(&addr)?;
        
        let shared_result_tx = Arc::new(Mutex::new(Some(result_tx)));
        let shared_shutdown_tx = Arc::new(Mutex::new(Some(shutdown_tx)));
        let shared_profile = Arc::new(profile); // Inject profile into state

        let app = Router::new()
            .route("/callback", get(move |query: Query<CallbackQuery>, 
                                                State((res_tx, sdn_tx, profile_name)): State<(SharedState, ShutdownState, Arc<String>)>| {
                async move {
                    // 1. Handle Platform Errors (e.g. user denied)
                    if let Some(err) = &query.error {
                        let desc = query.error_description.clone().unwrap_or_else(|| "User denied authorization or request failed.".to_string());
                        if let Some(tx) = res_tx.lock().unwrap().take() {
                            let _ = tx.send(Err(format!("{}: {}", err, desc)));
                        }
                        if let Some(tx) = sdn_tx.lock().unwrap().take() { let _ = tx.send(()); }
                        
                        let html = include_str!("error.html")
                            .replace("{{ERROR}}", err)
                            .replace("{{DESCRIPTION}}", &desc);
                        return Html(html);
                    }

                    // 2. Handle missing parameters
                    if query.code.is_none() || query.state.is_none() {
                        let desc = "Missing required parameters (code/state).".to_string();
                        if let Some(tx) = res_tx.lock().unwrap().take() {
                            let _ = tx.send(Err(desc.clone()));
                        }
                        if let Some(tx) = sdn_tx.lock().unwrap().take() { let _ = tx.send(()); }
                        
                        let html = include_str!("error.html")
                            .replace("{{ERROR}}", "BAD_REQUEST")
                            .replace("{{DESCRIPTION}}", &desc);
                        return Html(html);
                    }

                    let res = CallbackResult {
                        code: query.code.clone().unwrap(),
                        state: query.state.clone().unwrap(),
                    };
                    
                    if let Some(tx) = res_tx.lock().unwrap().take() {
                        let _ = tx.send(Ok(res));
                    }
                    
                    if let Some(tx) = sdn_tx.lock().unwrap().take() {
                        let _ = tx.send(());
                    }

                    let html = include_str!("success.html")
                        .replace("{{PROFILE}}", &profile_name);
                    Html(html)
                }
            }))
            .with_state((shared_result_tx, shared_shutdown_tx, shared_profile));

        let listener = tokio::net::TcpListener::bind(addr).await?;
        let actual_port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });

        Ok((actual_port, result_rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Client;

    #[tokio::test]
    async fn test_callback_capture() {
        let (port, rx) = OAuth2CallbackListener::start(0, "test_profile".to_string()).await.unwrap();
        assert!(port > 0);

        let url = format!("http://127.0.0.1:{}/callback?code=test_code&state=test_state", port);
        let client = Client::new();
        let resp = client.get(&url).send().await.unwrap();
        assert!(resp.status().is_success());

        let result = rx.await.unwrap().unwrap();
        assert_eq!(result.code, "test_code");
        assert_eq!(result.state, "test_state");
        
        // Ensure server is down
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let resp_retry = client.get(&url).send().await;
        assert!(resp_retry.is_err());
    }
}
