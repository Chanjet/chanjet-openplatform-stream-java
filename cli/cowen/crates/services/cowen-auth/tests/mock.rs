use async_trait::async_trait;
use cowen_auth::client::{HttpSender, SimpleResponse};
use cowen_common::CowenResult;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RequestLog {
    pub method: String,
    pub url: String,
    pub body: Option<serde_json::Value>,
}

pub struct InMemoryHttpSender {
    pub responses: Arc<Mutex<Vec<SimpleResponse>>>,
    pub requests: Arc<Mutex<Vec<RequestLog>>>,
}

impl InMemoryHttpSender {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(Vec::new())),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for InMemoryHttpSender {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryHttpSender {
    pub async fn push_response(&self, status: u16, body: &str) {
        let mut guard = self.responses.lock().await;
        guard.push(SimpleResponse {
            status,
            body: body.to_string(),
        });
    }

    async fn record_and_respond(
        &self,
        method: &str,
        url: &str,
        body: Option<serde_json::Value>,
    ) -> CowenResult<SimpleResponse> {
        let mut req_guard = self.requests.lock().await;
        req_guard.push(RequestLog {
            method: method.to_string(),
            url: url.to_string(),
            body,
        });

        let mut res_guard = self.responses.lock().await;
        if res_guard.is_empty() {
            Ok(SimpleResponse {
                status: 200,
                body: "{}".to_string(),
            })
        } else {
            Ok(res_guard.remove(0))
        }
    }
}

#[async_trait]
impl HttpSender for InMemoryHttpSender {
    async fn post(
        &self,
        url: &str,
        _headers: reqwest::header::HeaderMap,
        body: serde_json::Value,
    ) -> CowenResult<SimpleResponse> {
        self.record_and_respond("POST", url, Some(body)).await
    }

    async fn post_form(
        &self,
        url: &str,
        _headers: reqwest::header::HeaderMap,
        body: serde_json::Value,
    ) -> CowenResult<SimpleResponse> {
        self.record_and_respond("POST_FORM", url, Some(body)).await
    }

    async fn get(
        &self,
        url: &str,
        _headers: reqwest::header::HeaderMap,
    ) -> CowenResult<SimpleResponse> {
        self.record_and_respond("GET", url, None).await
    }
}
