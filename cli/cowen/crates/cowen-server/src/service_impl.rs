use cowen_common::CowenResult;
use async_trait::async_trait;
use std::sync::Arc;
use cowen_common::daemon::DaemonService;
use cowen_common::config::Config;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;

pub struct ServerDaemonService {
    worker_mgr: Arc<crate::daemon::manager::WorkerManager>,
}

impl ServerDaemonService {
    pub fn new(cfg_mgr: ConfigManager) -> Self {
        Self { 
            worker_mgr: Arc::new(crate::daemon::manager::WorkerManager::new(cfg_mgr)),
        }
    }
}

#[async_trait]
impl DaemonService for ServerDaemonService {
    async fn start_daemon(&self, profile: &str, config: &Config, _vault: Arc<dyn Vault>) -> CowenResult<()> {
        self.worker_mgr.start_worker(profile, config.clone()).await
    }

    async fn reload_daemon(&self, profile: &str) -> CowenResult<()> {
        self.worker_mgr.reload_worker(profile).await
    }
}
