pub use cowen_common::vault::*;
pub use cowen_store::vault_impl::*;

use anyhow::Result;
use std::sync::Arc;
use cowen_common::AppConfig;
use std::path::Path;

pub async fn create_vault(app_cfg: &AppConfig, app_dir: &Path, fingerprint: &str) -> Result<Arc<dyn Vault>> {
    let store_type = &app_cfg.storage.store;
    
    // 1. Resolve Primary Store (Manifest / Audit)
    let primary = if store_type == "local" {
        cowen_store::create_store_from_url(store_type, app_dir, fingerprint).await?
    } else if store_type == "innerdb" || store_type == "sqlite" {
        let url = app_cfg.storage.db_url.as_ref().cloned().unwrap_or_else(|| "innerdb".to_string());
        cowen_store::create_store_from_url(&url, app_dir, fingerprint).await?
    } else {
        let url = app_cfg.storage.db_url.as_ref().ok_or_else(|| anyhow::anyhow!("Database URL is missing for distributed store: {}", store_type))?;
        cowen_store::create_store_from_url(url, app_dir, fingerprint).await?
    };

    // 2. Resolve Sensitive Store (Secrets / Tokens)
    let sensitive = if let Some(url) = &app_cfg.storage.db_url {
        // Optimization: If db_url matches what we used for primary, reuse it
        let mut is_same_db = false;
        if store_type == "innerdb" || store_type == "sqlite" {
             is_same_db = true; // primary already used this url
        } else if let Some(p_url) = app_cfg.storage.db_url.as_ref() {
             if url == p_url { is_same_db = true; }
        }

        if is_same_db {
             primary.clone()
        } else {
             // Different DB or cache+DB hybrid
             cowen_store::create_store_from_url(url, app_dir, fingerprint).await?
        }
    } else {
        primary.clone()
    };

    Ok(Arc::new(StoreVault::new(primary, sensitive)))
}
