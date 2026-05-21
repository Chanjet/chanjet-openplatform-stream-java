use cowen_common::CowenResult;
use reqwest::Client;
use crate::mgmt::{AuthProgressInfo, FinalizeRequest};

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

    pub async fn finalize_auth(&self, profile: &str, code: &str, state: Option<&str>, session_id: &str) -> CowenResult<()> {
        let url = format!("{}/v1/mgmt/auth/finalize", self.base_url);
        let req = FinalizeRequest {
            profile: profile.to_string(),
            code: code.to_string(),
            state: state.map(|s| s.to_string()),
            session_id: session_id.to_string(),
        };

        let resp = self.http.post(&url).json(&req).send().await
            .map_err(|e| cowen_common::CowenError::api(format!("Failed to connect to monitor for finalization: {}", e)))?;
        
        if resp.status().is_success() {
            Ok(())
        } else {
            let err = resp.text().await.unwrap_or_default();
            Err(cowen_common::CowenError::api(format!("Auth finalization failed: {}", err)))
        }
    }

    pub async fn get_auth_progress(&self, profile: &str) -> CowenResult<AuthProgressInfo> {
        let url = format!("{}/v1/mgmt/auth/progress?profile={}", self.base_url, profile);
        let resp = self.http.get(&url).send().await
            .map_err(|e| cowen_common::CowenError::api(format!("Failed to connect to monitor for progress: {}", e)))?;
        
        if resp.status().is_success() {
            Ok(resp.json().await.map_err(|e| cowen_common::CowenError::api(format!("Failed to parse progress JSON: {}", e)))?)
        } else {
            let err = resp.text().await.unwrap_or_default();
            Err(cowen_common::CowenError::api(format!("Progress query failed: {}", err)))
        }
    }
}
