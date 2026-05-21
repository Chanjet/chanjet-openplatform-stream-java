pub mod telemetry;

use anyhow::Result;
use cowen_config::ConfigManager;
use cowen_common::store::Store;
use std::sync::Arc;
use cowen_common::security;

pub async fn create_store(cfg_mgr: &ConfigManager) -> Result<Arc<dyn Store>> {
    let app_cfg = cfg_mgr.load_app_config().await?;
    let app_dir = &cfg_mgr.app_dir;
    let fingerprint = security::get_machine_fingerprint()?;
    
    let url = if app_cfg.storage.store == "local" {
        "local"
    } else {
        app_cfg.storage.db_url.as_deref().unwrap_or("innerdb")
    };

    Ok(cowen_store::create_store_from_url(url, app_dir, &fingerprint).await?)
}

use cowen_monitor::status::{get_active_daemon_info, DaemonInfo};

pub struct DaemonManager {
    _cfg_mgr: ConfigManager,
}

impl DaemonManager {
    pub fn new(cfg_mgr: ConfigManager) -> Self {
        Self { _cfg_mgr: cfg_mgr }
    }

    pub async fn get_status(&self, profile: &str) -> Option<DaemonInfo> {
        get_active_daemon_info(profile)
    }
}
