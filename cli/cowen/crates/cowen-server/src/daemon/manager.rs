use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use cowen_common::{CowenResult, CowenError};
use cowen_common::config::Config;
use tracing::{info, error, warn};
use cowen_config::ConfigManager;
use crate::daemon::state::{ProfileWorker, WorkerStatus};
use tokio::time::{Duration, Instant};
use cowen_auth::client::Client;
use cowen_monitor::telemetry_db::TelemetryDb;

pub struct WorkerManager {
    cfg_mgr: ConfigManager,
    workers: Arc<Mutex<HashMap<String, ProfileWorker>>>,
    telemetry_db: Option<Arc<TelemetryDb>>,
}

impl WorkerManager {
    pub fn new(cfg_mgr: ConfigManager, telemetry_db: Option<Arc<TelemetryDb>>) -> Arc<Self> {
        let workers = Arc::new(Mutex::new(HashMap::new()));
        let manager = Arc::new(Self {
            cfg_mgr,
            workers,
            telemetry_db,
        });

        // Start Watchdog
        let manager_clone = manager.clone();
        tokio::spawn(async move {
            manager_clone.watchdog_loop().await;
        });

        manager
    }

    async fn record_event(&self, profile: &str, event: &str, old: Option<&str>, new: Option<&str>, details: Option<&str>) {
        if let Some(db) = &self.telemetry_db {
            let _ = db.insert_event(profile, event, old, new, details).await;
        }
    }

    pub fn config_manager(&self) -> &ConfigManager {
        &self.cfg_mgr
    }

    pub async fn start_worker(&self, profile: &str, config: Config) -> CowenResult<()> {
        let ready_rx = {
            let mut workers = self.workers.lock().await;
            let worker = workers.entry(profile.to_string()).or_insert_with(|| ProfileWorker::new(profile));

            if !worker.can_start() {
                info!(target: "sys", profile = %profile, status = ?worker.status, "Worker already active or draining, skipping start");
                return Ok(());
            }

            self.spawn_worker_internal_raw(worker, config)?
        };

        // Wait for readiness OUTSIDE the lock
        match tokio::time::timeout(Duration::from_secs(30), ready_rx).await {
            Ok(Ok(_)) => {
                let mut workers = self.workers.lock().await;
                if let Some(w) = workers.get_mut(profile) {
                    w.status = WorkerStatus::Running;
                }
                self.record_event(profile, "status_change", Some("Starting"), Some("Running"), None).await;
                Ok(())
            }
            _ => {
                warn!(target: "sys", profile = %profile, "Worker signaled start failure or timed out");
                Ok(())
            }
        }
    }

    fn spawn_worker_internal_raw(&self, worker: &mut ProfileWorker, config: Config) -> CowenResult<tokio::sync::oneshot::Receiver<()>> {
        worker.status = WorkerStatus::Starting;
        worker.cancel_token = tokio_util::sync::CancellationToken::new();
        
        let profile_name = worker.profile.clone();
        let cfg_mgr = self.cfg_mgr.clone();
        let cancel_token = worker.cancel_token.clone();
        let workers_clone = self.workers.clone();
        let telemetry_db = self.telemetry_db.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();

        let join_handle = tokio::spawn(async move {
            let result = run_profile_worker(&profile_name, config, cfg_mgr, ready_tx, cancel_token).await;

            let mut w_map = workers_clone.lock().await;
            if let Some(w) = w_map.get_mut(&profile_name) {
                match result {
                    Ok(_) => {
                        info!(target: "sys", profile = %profile_name, "Worker stopped gracefully");
                        w.status = WorkerStatus::Stopped;
                        if let Some(db) = &telemetry_db {
                            let _ = db.insert_event(&profile_name, "status_change", Some("Draining"), Some("Stopped"), None).await;
                        }
                    }
                    Err(e) => {
                        error!(target: "sys", profile = %profile_name, error = %e, "Worker failed");
                        let current_retry = match w.status {
                            WorkerStatus::Backoff { retry_count, .. } => retry_count,
                            _ => 0,
                        };
                        
                        let next_retry = current_retry + 1;

                        if next_retry > 5 {
                            w.status = WorkerStatus::Failed { reason: format!("Circuit breaker triggered: {}", e) };
                            if let Some(db) = &telemetry_db {
                                let _ = db.insert_event(&profile_name, "circuit_break", None, Some("Failed"), Some(&e.to_string())).await;
                            }
                        } else {
                            // Exponential backoff with 10% jitter logic
                            let base_delay = 2u64.pow(next_retry as u32).min(60) as f64;
                            use rand::Rng;
                            let jitter = rand::thread_rng().gen_range(0.0..=0.2);
                            let delay_secs = base_delay * (0.9 + jitter);
                            let delay = Duration::from_secs_f64(delay_secs);
                            
                            w.status = WorkerStatus::Backoff { 
                                retry_count: next_retry, 
                                next_retry_at: Instant::now() + delay,
                                last_error: e.to_string()
                            };
                            info!(target: "sys", profile = %profile_name, "Entering backoff state, will retry in {:?}", delay);
                            if let Some(db) = &telemetry_db {
                                let _ = db.insert_event(&profile_name, "backoff", None, Some("Backoff"), Some(&format!("Retry: {}, Error: {}", next_retry, e))).await;
                            }
                        }
                    }
                }
            }
        });

        worker.join_handle = Some(join_handle);
        Ok(ready_rx)
    }

    pub async fn stop_worker(&self, profile: &str) -> CowenResult<()> {
        let mut workers = self.workers.lock().await;
        if let Some(worker) = workers.get_mut(profile) {
            if worker.can_stop() {
                info!(target: "sys", profile = %profile, "Stopping worker (Draining)");
                worker.status = WorkerStatus::Draining;
                worker.cancel_token.cancel();
            }
            Ok(())
        } else {
            Err(CowenError::api(format!("Worker for profile '{}' not found", profile)))
        }
    }

    pub async fn reload_worker(&self, profile: &str) -> CowenResult<()> {
        let config = self.cfg_mgr.load(profile).await?;
        self.stop_worker(profile).await?;
        
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let workers = self.workers.lock().await;
            if let Some(w) = workers.get(profile) {
                if !w.is_active() { break; }
            } else {
                break;
            }
        }
        
        self.start_worker(profile, config).await
    }

    pub async fn list_workers(&self) -> HashMap<String, WorkerStatus> {
        let workers = self.workers.lock().await;
        workers.iter().map(|(k, v)| (k.clone(), v.status.clone())).collect()
    }

    /// Wait for all workers to reach a non-active state (Stopped/Failed/Created),
    /// with a maximum timeout. Used during graceful shutdown to ensure drain completes.
    pub async fn wait_all_stopped(&self, timeout: std::time::Duration) {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let has_active = {
                let workers = self.workers.lock().await;
                workers.values().any(|w| w.is_active())
            };
            if !has_active {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                tracing::warn!(target: "sys", "Timeout waiting for all workers to stop. Forcing exit.");
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    async fn watchdog_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            let mut to_restart = Vec::new();

            {
                let w_map = self.workers.lock().await;
                let now = Instant::now();
                for (name, worker) in w_map.iter() {
                    if let WorkerStatus::Backoff { next_retry_at, .. } = &worker.status {
                        if now >= *next_retry_at {
                            to_restart.push(name.clone());
                        }
                    }
                }
            }

            for name in to_restart {
                if let Ok(config) = self.cfg_mgr.load(&name).await {
                    let ready_rx = {
                        let mut w_map = self.workers.lock().await;
                        if let Some(worker) = w_map.get_mut(&name) {
                            if let WorkerStatus::Backoff { .. } = &worker.status {
                                info!(target: "sys", profile = %name, "Watchdog triggering restart from backoff");
                                self.spawn_worker_internal_raw(worker, config).ok()
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    };

                    if let Some(rx) = ready_rx {
                        // Wait for readiness OUTSIDE the lock
                        if let Ok(Ok(_)) = tokio::time::timeout(Duration::from_secs(30), rx).await {
                            let mut workers = self.workers.lock().await;
                            if let Some(w) = workers.get_mut(&name) {
                                w.status = WorkerStatus::Running;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;

async fn run_profile_worker(
    profile: &str, 
    mut config: Config, 
    cfg_mgr: ConfigManager, 
    ready_tx: tokio::sync::oneshot::Sender<()>,
    cancel_token: tokio_util::sync::CancellationToken,
) -> CowenResult<()> {
    let app_dir = cowen_common::config::get_app_dir();
    let app_cfg = cfg_mgr.load_app_config().await?;
    let vault = cowen_store::create_vault(&app_cfg, &app_dir, &config.app_key).await?;

    let auth = cowen_auth::create_auth_client(Arc::new(cowen_auth::VaultTokenPool::new(vault.clone())));
    let _ = auth.provider(&config.app_mode).hydrate_config(profile, &mut config, vault.clone()).await;

    let _forwarder = Arc::new(crate::daemon::forwarder::Forwarder::new(profile, config.clone(), vault.clone())?);
    
    let shutdown_gate = crate::utils::shutdown::ShutdownGate::new();

    // Signal readiness
    let _ = ready_tx.send(());

    // Proactive Sync
    let sync_profile = profile.to_string();
    let sync_config = config.clone();
    let sync_gate = shutdown_gate.clone();
    tokio::spawn(async move {
        let _guard = sync_gate.enter();
        let _ = auth.get_token(&sync_profile, &sync_config, &reqwest::header::HeaderMap::new()).await;
    });

    if let Err(e) = crate::cmd::bridge::run(
        profile, &config, vault, config.proxy_port, config.proxy_enabled, cfg_mgr.is_distributed_storage(&app_cfg), cancel_token, shutdown_gate
    ).await {
         return Err(CowenError::Internal(format!("Bridge task failed: {}", e)));
    }

    Ok(())
}
