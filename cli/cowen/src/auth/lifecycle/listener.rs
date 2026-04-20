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
    code: String,
    state: String,
}

type SharedState = Arc<Mutex<Option<oneshot::Sender<CallbackResult>>>>;
type ShutdownState = Arc<Mutex<Option<oneshot::Sender<()>>>>;

pub struct OAuth2CallbackListener;

impl OAuth2CallbackListener {
    pub async fn start(port: u16) -> (u16, oneshot::Receiver<CallbackResult>) {
        let (result_tx, result_rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let shared_result_tx = Arc::new(Mutex::new(Some(result_tx)));
        let shared_shutdown_tx = Arc::new(Mutex::new(Some(shutdown_tx)));

        let app = Router::new()
            .route("/callback", get(move |query: Query<CallbackQuery>, 
                                               State((res_tx, sdn_tx)): State<(SharedState, ShutdownState)>| {
                async move {
                    let res = CallbackResult {
                        code: query.code.clone(),
                        state: query.state.clone(),
                    };
                    
                    if let Some(tx) = res_tx.lock().unwrap().take() {
                        let _ = tx.send(res);
                    }
                    
                    if let Some(tx) = sdn_tx.lock().unwrap().take() {
                        let _ = tx.send(());
                    }

                    Html(include_str!("success.html"))
                }
            }))
            .with_state((shared_result_tx, shared_shutdown_tx));

        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        let actual_port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });

        (actual_port, result_rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Client;

    #[tokio::test]
    async fn test_callback_capture() {
        let (port, rx) = OAuth2CallbackListener::start(0).await;
        assert!(port > 0);

        let url = format!("http://127.0.0.1:{}/callback?code=test_code&state=test_state", port);
        let client = Client::new();
        let resp = client.get(&url).send().await.unwrap();
        assert!(resp.status().is_success());

        let result = rx.await.unwrap();
        assert_eq!(result.code, "test_code");
        assert_eq!(result.state, "test_state");
        
        // Ensure server is down
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let resp_retry = client.get(&url).send().await;
        assert!(resp_retry.is_err());
    }
}
