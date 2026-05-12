#[cfg(feature = "mysql")]
mod mysql;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "sqlite")]
mod sqlite;
#[cfg(feature = "mssql")]
mod mssql;

use crate::{Store, AuditEntry, DlqMessage, Item};
use cowen_common::{CowenResult, CowenError};
use async_trait::async_trait;

use std::sync::Arc;

#[async_trait]
pub trait SqlDriver: Send + Sync {
    // --- Config ---
    async fn get_config(&self, profile: &str, key: &str) -> CowenResult<String>;
    async fn get_config_metadata(&self, profile: &str, key: &str) -> CowenResult<(u64, i64)>;
    async fn get_config_full(&self, profile: &str, key: &str) -> CowenResult<Item>;
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> CowenResult<()>;
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> CowenResult<()>;
    async fn list_configs(&self, profile: &str) -> CowenResult<Vec<String>>;
    async fn delete_config(&self, profile: &str, key: &str) -> CowenResult<()>;

    // --- Secret ---
    async fn get_secret(&self, profile: &str, key: &str) -> CowenResult<String>;
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> CowenResult<()>;
    async fn delete_secret(&self, profile: &str, key: &str) -> CowenResult<()>;
    #[allow(dead_code)]
    async fn list_secrets(&self, profile: &str) -> CowenResult<Vec<String>>;

    // --- Token Domain ---
    async fn get_access_token(&self, profile: &str) -> CowenResult<cowen_common::models::Token>;
    async fn save_access_token(&self, profile: &str, token: cowen_common::models::Token) -> CowenResult<()>;
    async fn delete_access_token(&self, profile: &str) -> CowenResult<()>;
    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<cowen_common::models::Token>;
    async fn save_app_access_token(&self, app_key: &str, token: cowen_common::models::Token) -> CowenResult<()>;

    // --- Ticket Domain ---
    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<cowen_common::models::Ticket>;
    async fn save_app_ticket(&self, app_key: &str, ticket: cowen_common::models::Ticket) -> CowenResult<()>;
    async fn delete_app_ticket(&self, app_key: &str) -> CowenResult<()>;

    // --- Permanent Code Domain ---
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> CowenResult<String>;
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> CowenResult<()>;
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> CowenResult<String>;
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> CowenResult<()>;

    // --- Legacy Support (Internal/Generic KV) ---
    async fn get_token(&self, profile: &str, key: &str) -> CowenResult<String>;
    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> CowenResult<()>;
    async fn delete_token(&self, profile: &str, key: &str) -> CowenResult<()>;
    async fn list_tokens(&self, profile: &str) -> CowenResult<Vec<String>>;

    // --- Audit ---
    async fn save_audit(&self, entry: &AuditEntry) -> CowenResult<()>;
    async fn list_audit(&self, profile: &str, limit: usize) -> CowenResult<Vec<AuditEntry>>;

    // --- DLQ ---
    async fn push_dlq(&self, msg: &DlqMessage) -> CowenResult<()>;
    #[allow(dead_code)]
    async fn pop_dlq(&self, profile: &str, topic: &str) -> CowenResult<Option<DlqMessage>>;
    #[allow(dead_code)]
    async fn list_dlq(&self, profile: &str, limit: usize) -> CowenResult<Vec<DlqMessage>>;
    async fn list_all_dlq(&self, profile: &str) -> CowenResult<Vec<DlqMessage>>;

    // --- Management ---
    async fn clear_profile(&self, profile: &str) -> CowenResult<()>;
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> CowenResult<()>;
    async fn list_all_profiles(&self) -> CowenResult<Vec<String>>;
    async fn raw_del(&self, key: &str) -> CowenResult<()>;

}

#[async_trait]
pub trait SqlBuilder: Send + Sync {
    fn scheme(&self) -> &str;
    async fn build(&self, url: &str) -> CowenResult<Arc<dyn SqlDriver>>;
}

pub struct SqlBuilderRegistration {
    pub builder: &'static dyn SqlBuilder,
}

inventory::collect!(SqlBuilderRegistration);

pub struct SqlStore {
    driver: Arc<dyn SqlDriver>,
    name: String,
    url: String,
}

impl SqlStore {
    pub fn new(driver: Arc<dyn SqlDriver>, name: &str, url: &str) -> Self {
        Self { driver, name: name.to_string(), url: url.to_string() }
    }

    fn resolve_profile(&self, profile: &str) -> String {
        profile.to_string()
    }

    pub fn supported_schemes() -> Vec<String> {
        inventory::iter::<SqlBuilderRegistration>
            .into_iter()
            .map(|reg| reg.builder.scheme().to_string())
            .collect()
    }

    pub fn is_supported(scheme: &str) -> bool {
        let scheme = if scheme == "innerdb" { "sqlite" } else { scheme };
        inventory::iter::<SqlBuilderRegistration>
            .into_iter()
            .any(|reg| reg.builder.scheme() == scheme)
    }

    pub async fn from_url(url: &str) -> CowenResult<Self> {
        let mut scheme = url.split(":").next().ok_or_else(|| CowenError::api("Invalid database URL"))?;
        let mut actual_url = if scheme == "innerdb" {
            scheme = "sqlite";
            url.replace("innerdb://", "sqlite:")
        } else {
            url.to_string()
        };

        if actual_url.starts_with("sqlite://") {
             actual_url = actual_url.replace("sqlite://", "sqlite:");
        } else if (scheme == "mysql" || scheme == "postgres") && !actual_url.contains("://") {
             actual_url = actual_url.replace("mysql:", "mysql://").replace("postgres:", "postgres://");
        }
        
        for reg in inventory::iter::<SqlBuilderRegistration> {
            if reg.builder.scheme() == scheme {
                let driver = reg.builder.build(&actual_url).await?;
                return Ok(Self::new(driver, scheme, url));
            }
        }

        Err(CowenError::api(format!("Unsupported database scheme: {}. Supported: {:?}", scheme, Self::supported_schemes())))
    }
}

#[async_trait]
impl Store for SqlStore {
    // --- Config ---
    async fn get_config(&self, profile: &str, key: &str) -> CowenResult<String> { self.driver.get_config(&self.resolve_profile(profile), key).await }
    async fn get_config_metadata(&self, profile: &str, key: &str) -> CowenResult<(u64, i64)> { self.driver.get_config_metadata(&self.resolve_profile(profile), key).await }
    async fn get_config_full(&self, profile: &str, key: &str) -> CowenResult<Item> { 
        self.driver.get_config_full(&self.resolve_profile(profile), key).await
    }
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> { self.driver.set_config(&self.resolve_profile(profile), key, value).await }
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, ev: u64) -> CowenResult<()> { self.driver.set_config_conditional(&self.resolve_profile(profile), key, value, ev).await }
    async fn list_configs(&self, profile: &str) -> CowenResult<Vec<String>> { self.driver.list_configs(&self.resolve_profile(profile)).await }
    async fn delete_config(&self, profile: &str, key: &str) -> CowenResult<()> { self.driver.delete_config(&self.resolve_profile(profile), key).await }

    // --- Secret ---
    async fn get_secret(&self, profile: &str, key: &str) -> CowenResult<String> { self.driver.get_secret(&self.resolve_profile(profile), key).await }
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> { self.driver.set_secret(&self.resolve_profile(profile), key, value).await }
    async fn delete_secret(&self, profile: &str, key: &str) -> CowenResult<()> { self.driver.delete_secret(&self.resolve_profile(profile), key).await }
    async fn list_secrets(&self, profile: &str) -> CowenResult<Vec<String>> { self.driver.list_secrets(&self.resolve_profile(profile)).await }

    // --- Token ---
    async fn get_access_token(&self, profile: &str) -> CowenResult<cowen_common::models::Token> { self.driver.get_access_token(&self.resolve_profile(profile)).await }
    async fn save_access_token(&self, profile: &str, token: cowen_common::models::Token) -> CowenResult<()> { self.driver.save_access_token(&self.resolve_profile(profile), token).await }
    async fn delete_access_token(&self, profile: &str) -> CowenResult<()> { self.driver.delete_access_token(&self.resolve_profile(profile)).await }
    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<cowen_common::models::Token> { self.driver.get_app_access_token(app_key).await }
    async fn save_app_access_token(&self, app_key: &str, token: cowen_common::models::Token) -> CowenResult<()> { self.driver.save_app_access_token(app_key, token).await }

    // --- Ticket ---
    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<cowen_common::models::Ticket> { self.driver.get_app_ticket(app_key).await }
    async fn save_app_ticket(&self, app_key: &str, ticket: cowen_common::models::Ticket) -> CowenResult<()> { self.driver.save_app_ticket(app_key, ticket).await }
    async fn delete_app_ticket(&self, app_key: &str) -> CowenResult<()> { self.driver.delete_app_ticket(app_key).await }

    // --- Permanent Code Domain ---
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> CowenResult<String> { self.driver.get_org_permanent_code(app_key, org_id).await }
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> CowenResult<()> { self.driver.save_org_permanent_code(app_key, org_id, code).await }
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> CowenResult<String> { self.driver.get_user_permanent_code(app_key, org_id, user_id).await }
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> CowenResult<()> { self.driver.save_user_permanent_code(app_key, org_id, user_id, code).await }

    // --- Legacy Support ---
    async fn get_token(&self, profile: &str, key: &str) -> CowenResult<String> { self.driver.get_token(&self.resolve_profile(profile), key).await }
    async fn set_token(&self, profile: &str, key: &str, value: &str, exp: u64) -> CowenResult<()> { self.driver.set_token(&self.resolve_profile(profile), key, value, exp).await }
    async fn delete_token(&self, profile: &str, key: &str) -> CowenResult<()> { self.driver.delete_token(&self.resolve_profile(profile), key).await }
    async fn list_tokens(&self, profile: &str) -> CowenResult<Vec<String>> { self.driver.list_tokens(&self.resolve_profile(profile)).await }

    // --- Audit ---
    async fn save_audit(&self, entry: &AuditEntry) -> CowenResult<()> { 
        self.driver.save_audit(entry).await 
    }
    async fn list_audit(&self, profile: &str, limit: usize) -> CowenResult<Vec<AuditEntry>> { 
        self.driver.list_audit(&self.resolve_profile(profile), limit).await
    }

    // --- DLQ ---
    async fn push_dlq(&self, msg: &DlqMessage) -> CowenResult<()> { 
        self.driver.push_dlq(msg).await 
    }
    async fn pop_dlq(&self, profile: &str, topic: &str) -> CowenResult<Option<DlqMessage>> { 
        self.driver.pop_dlq(&self.resolve_profile(profile), topic).await
    }
    async fn list_dlq(&self, profile: &str, limit: usize) -> CowenResult<Vec<DlqMessage>> { 
        self.driver.list_dlq(&self.resolve_profile(profile), limit).await
    }
    async fn list_all_dlq(&self, profile: &str) -> CowenResult<Vec<DlqMessage>> { 
        self.driver.list_all_dlq(&self.resolve_profile(profile)).await
    }

    // --- Management ---
    async fn clear_profile(&self, profile: &str) -> CowenResult<()> { self.driver.clear_profile(&self.resolve_profile(profile)).await }
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> CowenResult<()> { self.driver.rename_profile(&self.resolve_profile(old_name), &self.resolve_profile(new_name)).await }
    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> {
        self.driver.list_all_profiles().await
    }
    async fn raw_del(&self, key: &str) -> CowenResult<()> { self.driver.raw_del(key).await }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> String {
        format!("SQL Database ({}): {}", self.name, cowen_common::utils::mask_url(&self.url))
    }
}

pub struct SqlStoreBuilder;

#[async_trait]
impl cowen_common::store::StoreBuilder for SqlStoreBuilder {
    fn scheme(&self) -> &str { "sql-dispatch" } // Not really used for direct match
    async fn build(&self, url: &str, _app_dir: &std::path::Path, _fingerprint: &str) -> CowenResult<Arc<dyn cowen_common::store::Store>> {
        Ok(Arc::new(SqlStore::from_url(url).await?))
    }
}
