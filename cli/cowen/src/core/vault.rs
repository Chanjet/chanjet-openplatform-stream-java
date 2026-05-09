pub use cowen_common::vault::*;
pub use cowen_store::vault_impl::*;

use anyhow::Result;
use std::sync::Arc;
use crate::core::config::AppConfig;
use std::path::Path;

pub async fn create_vault(app_cfg: &AppConfig, app_dir: &Path, fingerprint: &str) -> Result<Arc<dyn Vault>> {
    let primary = crate::core::store::create_store_from_url(&app_cfg.storage.store, app_dir, fingerprint).await?;
    let sensitive = if let Some(url) = &app_cfg.storage.db_url {
        crate::core::store::create_store_from_url(url, app_dir, fingerprint).await?
    } else {
        primary.clone()
    };
    Ok(Arc::new(StoreVault::new(primary, sensitive)))
}
