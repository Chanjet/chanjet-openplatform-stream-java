use cowen_common::CowenResult;
use async_trait::async_trait;
use std::sync::Arc;
use cowen_common::daemon::DaemonService;
use cowen_common::config::Config;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;
use cowen_auth::client::Client;

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

    async fn stop_daemon(&self, profile: &str) -> CowenResult<()> {
        self.worker_mgr.stop_worker(profile).await
    }

    async fn stop_all(&self) -> CowenResult<()> {
        let workers = self.worker_mgr.list_workers().await;
        for (profile, _) in workers {
            let _ = self.worker_mgr.stop_worker(&profile).await;
        }
        // Give workers a moment to finish shutting down before exiting
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        Ok(())
    }

    async fn finalize_auth(&self, profile: &str, code: &str, state: Option<&str>, session_id: &str) -> CowenResult<()> {
        let cfg_mgr = self.worker_mgr.config_manager();
        let config = cfg_mgr.load(profile).await?;
        let app_cfg = cfg_mgr.load_app_config().await?;
        let app_dir = cowen_common::config::get_app_dir();
        
        let vault = cowen_store::create_vault(&app_cfg, &app_dir, &config.app_key).await?;
        let pool = Arc::new(cowen_auth::VaultTokenPool::new(vault.clone()));
        
        // 1. Save the code to session manager (OAuth2 provider expects this)
        let auth_cli = cowen_auth::create_auth_client(pool.clone());
        let session_manager = cowen_auth::lifecycle::AuthSessionManager::new(pool.as_ref());
        session_manager.save_code(profile, code, state.unwrap_or("")).await?;

        // 2. Perform the actual exchange
        // Passing session_id to perform_login triggers the finalization flow
        auth_cli.perform_login(profile, &config, false, Some(session_id), Some(Arc::new(self.clone_service()))).await?;

        // 3. Start the worker
        self.worker_mgr.start_worker(profile, config).await?;

        Ok(())
    }
}

impl ServerDaemonService {
    fn clone_service(&self) -> ServerDaemonService {
        Self {
            worker_mgr: self.worker_mgr.clone(),
        }
    }
}
