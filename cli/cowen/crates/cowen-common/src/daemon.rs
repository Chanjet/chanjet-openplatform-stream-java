use crate::CowenResult;
use async_trait::async_trait;
use std::sync::Arc;
use crate::config::Config;
use crate::vault::Vault;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DaemonStatus {
    pub is_running: bool,
    pub pid: Option<u32>,
    pub monitor_port: Option<u16>,
}

#[async_trait]
pub trait DaemonService: Send + Sync {
    async fn start_daemon(&self, profile: &str, config: &Config, vault: Arc<dyn Vault>) -> CowenResult<()>;
    async fn reload_daemon(&self, profile: &str) -> CowenResult<()>;
    async fn stop_daemon(&self, _profile: &str) -> CowenResult<()> { Ok(()) }
    async fn stop_all(&self) -> CowenResult<()> { Ok(()) }

    async fn finalize_auth(&self, _profile: &str, _code: &str, _state: Option<&str>, _session_id: &str) -> CowenResult<()> {
        Err(crate::CowenError::api("Auth finalization not supported by this daemon service"))
    }
}
