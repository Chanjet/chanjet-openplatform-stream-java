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

                    Html(r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>授权成功 | Cowen CLI</title>
    <style>
        body { background: #0F172A; color: #F8FAFC; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; display: flex; justify-content: center; align-items: center; min-height: 100vh; margin: 0; overflow: hidden; }
        .container { background: rgba(30, 41, 59, 0.7); backdrop-filter: blur(12px); border: 1px solid rgba(255, 255, 255, 0.1); padding: 3rem; border-radius: 1.5rem; text-align: center; box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5); animation: slideUp 0.6s cubic-bezier(0.16, 1, 0.3, 1); max-width: 400px; width: 90%; }
        .icon-wrapper { width: 80px; height: 80px; background: rgba(16, 185, 129, 0.1); border-radius: 50%; display: flex; justify-content: center; align-items: center; margin: 0 auto 1.5rem; color: #10B981; animation: scaleIn 0.5s 0.2s cubic-bezier(0.34, 1.56, 0.64, 1) both; }
        h1 { font-size: 1.75rem; font-weight: 700; margin: 0 0 1rem; letter-spacing: -0.025em; }
        p { color: #94A3B8; line-height: 1.6; margin: 0 0 2rem; }
        .back-msg { background: rgba(255, 255, 255, 0.05); padding: 0.75rem 1rem; border-radius: 0.75rem; font-size: 0.875rem; color: #64748B; border: 1px dashed rgba(255, 255, 255, 0.1); }
        @keyframes slideUp { from { transform: translateY(20px); opacity: 0; } to { transform: translateY(0); opacity: 1; } }
        @keyframes scaleIn { from { transform: scale(0.5); opacity: 0; } to { transform: scale(1); opacity: 1; } }
    </style>
</head>
<body>
    <div class="container">
        <div class="icon-wrapper">
            <svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>
        </div>
        <h1>授权成功</h1>
        <p>您已成功完成 Cowen CLI 的授权验证。<br>现在可以关闭此窗口并返回控制台继续操作。</p>
        <div class="back-msg">正在等待终端接收令牌...</div>
    </div>
</body>
</html>"#)
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
