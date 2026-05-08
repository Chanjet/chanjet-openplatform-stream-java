use anyhow::Result;
use async_trait::async_trait;
use crate::core::store::{Store, AuditEntry, DlqMessage, Item};
use crate::core::config::AppConfig;
use std::path::Path;
use std::sync::Arc;

#[async_trait]
pub trait Vault: 
    Send + Sync + 
    crate::domain::TicketDomain + 
    crate::domain::TokenDomain + 
    crate::domain::SessionDomain +
    crate::domain::SecretDomain +
    crate::domain::ConfigDomain +
    crate::domain::AuditDomain +
    crate::domain::DlqDomain +
    crate::domain::PermanentCodeDomain +
    crate::domain::ManagementDomain
{
    // --- Notification ---

    // --- Migration Support ---
    fn primary_store(&self) -> Arc<dyn Store>;
}

pub struct StoreVault {
    primary: Arc<dyn Store>,    // Config, Token, Logs, DLQ...
    sensitive: Arc<dyn Store>,  // Pinned to .seal OR Database
}

impl StoreVault {
    pub fn new(primary: Arc<dyn Store>, sensitive: Arc<dyn Store>) -> Self {
        Self { primary, sensitive }
    }
}

#[async_trait]
impl Vault for StoreVault {

    fn primary_store(&self) -> Arc<dyn Store> {
        self.primary.clone()
    }
}

#[async_trait]
impl crate::domain::ConfigDomain for StoreVault {
    async fn get_config(&self, profile: &str, key: &str) -> Result<String> { self.primary.get_config(profile, key).await }
    async fn get_config_full(&self, profile: &str, key: &str) -> Result<crate::core::store::Item> { self.primary.get_config_full(profile, key).await }
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()> { self.primary.set_config(profile, key, value).await }
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, ev: u64) -> Result<()> { self.primary.set_config_conditional(profile, key, value, ev).await }
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>> { self.primary.list_configs(profile).await }
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()> { self.primary.delete_config(profile, key).await }
}

#[async_trait]
impl crate::domain::AuditDomain for StoreVault {
    async fn save_audit(&self, entry: &crate::core::store::AuditEntry) -> Result<()> { self.primary.save_audit(entry).await }
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<crate::core::store::AuditEntry>> { self.primary.list_audit(profile, limit).await }
}

#[async_trait]
impl crate::domain::DlqDomain for StoreVault {
    async fn push_dlq(&self, msg: &crate::core::store::DlqMessage) -> Result<()> { self.primary.push_dlq(msg).await }
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<crate::core::store::DlqMessage>> { self.primary.pop_dlq(profile, topic).await }
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<crate::core::store::DlqMessage>> { self.primary.list_dlq(profile, limit).await }
    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<crate::core::store::DlqMessage>> { self.primary.list_all_dlq(profile).await }
}

#[async_trait]
impl crate::domain::ManagementDomain for StoreVault {
    async fn clear_profile(&self, profile: &str) -> Result<()> {
        let _ = self.sensitive.clear_profile(profile).await;
        if !Arc::ptr_eq(&self.sensitive, &self.primary) {
            let _ = self.primary.clear_profile(profile).await;
        }
        Ok(())
    }
    async fn rename_profile(&self, old: &str, new: &str) -> Result<()> {
        let _ = self.sensitive.rename_profile(old, new).await;
        if !Arc::ptr_eq(&self.sensitive, &self.primary) {
            let _ = self.primary.rename_profile(old, new).await;
        }
        Ok(())
    }
    async fn list_all_profiles(&self) -> Result<Vec<String>> {
        self.primary.list_all_profiles().await
    }
}

#[async_trait]
impl crate::domain::TicketDomain for StoreVault {
    async fn get_app_ticket(&self, app_key: &str) -> Result<crate::auth::models::Ticket> {
        self.sensitive.get_app_ticket(app_key).await
    }
    async fn save_app_ticket(&self, app_key: &str, ticket: crate::auth::models::Ticket) -> Result<()> {
        self.sensitive.save_app_ticket(app_key, ticket).await
    }
}

#[async_trait]
impl crate::domain::TokenDomain for StoreVault {
    async fn get_access_token(&self, profile: &str) -> Result<crate::auth::models::Token> {
        self.primary.get_access_token(profile).await
    }
    async fn save_access_token(&self, profile: &str, token: crate::auth::models::Token) -> Result<()> {
        self.primary.save_access_token(profile, token).await
    }
    async fn delete_access_token(&self, profile: &str) -> Result<()> {
        self.primary.delete_access_token(profile).await
    }
    async fn get_app_access_token(&self, app_key: &str) -> Result<crate::auth::models::Token> {
        self.primary.get_app_access_token(app_key).await
    }
    async fn save_app_access_token(&self, app_key: &str, token: crate::auth::models::Token) -> Result<()> {
        self.primary.save_app_access_token(app_key, token).await
    }
}

#[async_trait]
impl crate::domain::SessionDomain for StoreVault {
    async fn get_session(&self, state: &str) -> Result<crate::auth::models::AuthSession> {
        let json = self.primary.get_token("global", &format!("session:{}", state)).await?;
        Ok(serde_json::from_str(&json)?)
    }
    async fn save_session(&self, session: crate::auth::models::AuthSession) -> Result<()> {
        let json = serde_json::to_string(&session)?;
        let ttl = (session.expires_at - chrono::Utc::now()).num_seconds().max(0) as u64;
        self.primary.set_token("global", &format!("session:{}", session.state), &json, ttl).await
    }
    async fn delete_session(&self, state: &str) -> Result<()> {
        self.primary.delete_token("global", &format!("session:{}", state)).await
    }
}

#[async_trait]
impl crate::domain::PermanentCodeDomain for StoreVault {
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> Result<String> {
        self.sensitive.get_org_permanent_code(app_key, org_id).await
    }
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> Result<()> {
        self.sensitive.save_org_permanent_code(app_key, org_id, code).await
    }
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> Result<String> {
        self.sensitive.get_user_permanent_code(app_key, org_id, user_id).await
    }
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> Result<()> {
        self.sensitive.save_user_permanent_code(app_key, org_id, user_id, code).await
    }
}

#[async_trait]
impl crate::domain::SecretDomain for StoreVault {
    async fn get_secret(&self, profile: &str, key: &str) -> Result<String> {
        self.sensitive.get_secret(profile, key).await
    }
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()> {
        self.sensitive.set_secret(profile, key, value).await
    }
    async fn delete_secret(&self, profile: &str, key: &str) -> Result<()> {
        self.sensitive.delete_secret(profile, key).await
    }
}

pub async fn create_vault(app_config: &AppConfig, app_dir: &Path, fingerprint: &str) -> Result<Arc<dyn Vault>> {
    use crate::core::store::sql::SqlStore;
    use crate::core::store::file::{FileStore, MonolithicSealStore};

    let storage_cfg = &app_config.storage;
    let store_type = storage_cfg.store.as_str();
    let seal_path = app_dir.join(".seal");

    // 1. Determine Primary and Sensitive Stores
    let (primary, sensitive): (Arc<dyn Store>, Arc<dyn Store>) = if store_type == "innerdb" || store_type == "sqlite" {
        let db_url = storage_cfg.db_url.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database URL is required for storage mode: {}", store_type))?;
        let sql_store: Arc<dyn Store> = Arc::new(SqlStore::from_url(db_url).await?);
        
        let secret_store: Arc<dyn Store> = if store_type == "sqlite" {
            sql_store.clone()
        } else if seal_path.is_file() {
            Arc::new(MonolithicSealStore::new(seal_path, fingerprint))
        } else {
            Arc::new(FileStore::new(seal_path, fingerprint)?)
        };
        (sql_store, secret_store)
    } else {
        let mut found = None;
        for reg in inventory::iter::<crate::core::store::StoreBuilderRegistration> {
            if reg.builder.scheme() == store_type {
                let store = reg.builder.build(storage_cfg.db_url.as_deref().unwrap_or(""), app_dir, fingerprint).await?;
                found = Some((store.clone(), store));
                break;
            }
        }

        if let Some(pair) = found {
            pair
        } else if SqlStore::is_supported(store_type) {
            let db_url = storage_cfg.db_url.as_ref()
                .ok_or_else(|| anyhow::anyhow!("Database URL is required for remote SQL storage"))?;
            let sql_store: Arc<dyn Store> = Arc::new(SqlStore::from_url(db_url).await?);
            (sql_store.clone(), sql_store)
        } else {
            return Err(anyhow::anyhow!("Unsupported store type: {}", store_type));
        }
    };

    let mut final_primary = primary;
    if storage_cfg.cache != "none" {
        let mut applied = false;
        for reg in inventory::iter::<crate::core::store::CacheBuilderRegistration> {
            if reg.builder.scheme() == storage_cfg.cache {
                let cache_url = storage_cfg.cache_url.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Cache URL is required for storage cache mode: {}", storage_cfg.cache))?;
                final_primary = reg.builder.build(cache_url, final_primary).await?;
                applied = true;
                break;
            }
        }
        if !applied {
            return Err(anyhow::anyhow!("Unsupported cache mode: {}", storage_cfg.cache));
        }
    }

    Ok(Arc::new(StoreVault::new(final_primary, sensitive)))
}
