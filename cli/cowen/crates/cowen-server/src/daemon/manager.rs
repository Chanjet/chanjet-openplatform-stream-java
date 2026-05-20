use std::collections::HashMap;
use cowen_auth::client::Client;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};
use cowen_common::{CowenResult, CowenError};
use cowen_common::config::Config;
use tracing::{info, error};
use cowen_config::ConfigManager;

#[derive(Debug, Clone, serde::Serialize)]
pub enum WorkerStatus {
    Starting,
    Running,
    Stopped,
    Failed(String),
}

pub struct WorkerHandle {
    pub profile: String,
    pub status: WorkerStatus,
    pub stop_tx: broadcast::Sender<()>,
}

pub struct WorkerManager {
    cfg_mgr: ConfigManager,
    workers: Arc<Mutex<HashMap<String, WorkerHandle>>>,
}

impl WorkerManager {
    pub fn new(cfg_mgr: ConfigManager) -> Self {
        Self {
            cfg_mgr,
            workers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

        pub async fn start_worker(&self, profile: &str, config: Config) -> CowenResult<()> {
        let mut workers = self.workers.lock().await;
        if workers.contains_key(profile) {
            return Err(CowenError::api(format!("Worker for profile '{}' already exists", profile)));
        }

        let (stop_tx, _) = broadcast::channel(1);
        let mut stop_rx = stop_tx.subscribe();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let profile_name = profile.to_string();

        let handle = WorkerHandle {
            profile: profile_name.clone(),
            status: WorkerStatus::Starting,
            stop_tx: stop_tx.clone(),
        };
        workers.insert(profile_name.clone(), handle);

        let workers_clone = self.workers.clone();
        let cfg_mgr = self.cfg_mgr.clone();
        tokio::spawn(async move {
            info!(target: "sys", profile = %profile_name, "Starting worker for profile");
            
            let result = tokio::select! {
                _ = stop_rx.recv() => {
                    info!(target: "sys", profile = %profile_name, "Worker received stop signal");
                    Ok(())
                }
                res = run_profile_worker(&profile_name, config, cfg_mgr, ready_tx) => res,
            };

            let mut w = workers_clone.lock().await;
            if let Some(h) = w.get_mut(&profile_name) {
                match result {
                    Ok(_) => h.status = WorkerStatus::Stopped,
                    Err(e) => {
                        error!(target: "sys", profile = %profile_name, error = %e, "Worker failed");
                        h.status = WorkerStatus::Failed(e.to_string());
                    }
                }
            }
        });

        // Wait for worker to be ready
        match tokio::time::timeout(tokio::time::Duration::from_secs(40), ready_rx).await {
            Ok(Ok(_)) => {
                let mut w = self.workers.lock().await;
                if let Some(h) = w.get_mut(profile) {
                    h.status = WorkerStatus::Running;
                }
                Ok(())
            }
            Ok(Err(_)) => Err(CowenError::Internal("Worker failed to signal readiness".to_string())),
            Err(_) => Err(CowenError::Internal("Worker startup timed out".to_string())),
        }
    }

    pub async fn stop_worker(&self, profile: &str) -> CowenResult<()> {
        let workers = self.workers.lock().await;
        if let Some(h) = workers.get(profile) {
            let _ = h.stop_tx.send(());
            Ok(())
        } else {
            Err(CowenError::api(format!("Worker for profile '{}' not found", profile)))
        }
    }

    pub async fn reload_worker(&self, profile: &str) -> CowenResult<()> {
        let config = self.cfg_mgr.load(profile).await?;
        self.stop_worker(profile).await?;
        // Brief wait for stop to propagate
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        self.start_worker(profile, config).await
    }

    pub async fn list_workers(&self) -> HashMap<String, WorkerStatus> {
        let workers = self.workers.lock().await;
        workers.iter().map(|(k, v)| (k.clone(), v.status.clone())).collect()
    }
}

async fn run_profile_worker(profile: &str, config: Config, cfg_mgr: ConfigManager, ready_tx: tokio::sync::oneshot::Sender<()>) -> CowenResult<()> {
    let app_dir = cowen_common::config::get_app_dir();
    let app_cfg = cfg_mgr.load_app_config().await?;
    let vault = cowen_store::create_vault(&app_cfg, &app_dir, &config.app_key).await?;

    let _forwarder = Arc::new(crate::daemon::forwarder::Forwarder::new(profile, config.clone(), vault.clone())?);
    
    // Signal readiness early before blocking operations like token sync
    let _ = ready_tx.send(());

    // 🚀 PROACTIVE SYNC (Non-blocking): Ensure we have the latest token from shared storage immediately upon start/reload
    let auth = cowen_auth::create_auth_client(Arc::new(cowen_auth::VaultTokenPool::new(vault.clone())));
    let sync_profile = profile.to_string();
    let sync_config = config.clone();
    tokio::spawn(async move {
        tracing::info!(target: "sys", profile = %sync_profile, "Proactively syncing token from vault in background");
        let _ = auth.get_token(&sync_profile, &sync_config, &reqwest::header::HeaderMap::new()).await;
    });

    // 1. Bridge Task (Core Engine)
    // The bridge handles streaming, proxy (if enabled), and token maintenance.
    let b_profile = profile.to_string();
    let b_config = config.clone();
    let b_vault = vault.clone();
    let b_proxy_port = config.proxy_port;
    let b_enable_proxy = config.proxy_enabled;
    let b_is_dist = cfg_mgr.is_distributed_storage(&app_cfg);
    
    if let Err(e) = crate::cmd::bridge::run(&b_profile, &b_config, b_vault, b_proxy_port, b_enable_proxy, b_is_dist).await {
         error!(target: "sys", profile = %b_profile, error = %e, "Bridge task failed");
         return Err(CowenError::Internal(format!("Bridge task failed: {}", e)));
    }

    Ok(())
}
