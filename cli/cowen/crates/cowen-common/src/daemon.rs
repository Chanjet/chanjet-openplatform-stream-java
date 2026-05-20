use crate::CowenResult;
use async_trait::async_trait;
use std::sync::Arc;
use crate::config::Config;
use crate::vault::Vault;

#[async_trait]
pub trait DaemonService: Send + Sync {
    async fn start_daemon(&self, profile: &str, config: &Config, vault: Arc<dyn Vault>) -> CowenResult<()>;
    async fn reload_daemon(&self, profile: &str) -> CowenResult<()>;
}
