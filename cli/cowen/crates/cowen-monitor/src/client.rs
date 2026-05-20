use cowen_common::CowenResult;
use reqwest::Client;

pub struct MonitorClient {
    base_url: String,
    http: Client,
}

impl MonitorClient {
    pub fn new(port: u16) -> Self {
        Self {
            base_url: format!("http://127.0.0.1:{}", port),
            http: Client::new(),
        }
    }

    pub async fn reload_worker(&self, profile: &str) -> CowenResult<()> {
        let url = format!("{}/daemon/reload?profile={}", self.base_url, profile);
        let resp = self.http.post(&url).send().await
            .map_err(|e| cowen_common::CowenError::api(format!("Failed to connect to monitor: {}", e)))?;
        
        if resp.status().is_success() {
            Ok(())
        } else {
            let err = resp.text().await.unwrap_or_default();
            Err(cowen_common::CowenError::api(format!("Monitor reload failed: {}", err)))
        }
    }
}
