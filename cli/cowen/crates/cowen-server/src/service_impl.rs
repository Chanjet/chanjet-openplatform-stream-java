use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use cowen_common::daemon::DaemonService;
use cowen_common::config::Config;
use cowen_common::vault::Vault;
use cowen_common::ConfigManager;

pub struct ServerDaemonService {
    cfg_mgr: ConfigManager,
}

impl ServerDaemonService {
    pub fn new(cfg_mgr: ConfigManager) -> Self {
        Self { cfg_mgr }
    }
}

#[async_trait]
impl DaemonService for ServerDaemonService {
    async fn start_daemon(&self, profile: &str, config: &Config, vault: Arc<dyn Vault>) -> Result<()> {
        crate::cmd::start(profile, config, config.proxy_port, config.proxy_enabled, false, false, &self.cfg_mgr, vault).await
    }
}
