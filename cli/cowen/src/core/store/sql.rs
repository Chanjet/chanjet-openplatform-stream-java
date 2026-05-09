mod mysql;
mod postgres;
mod sqlite;
mod mssql;

use super::{Store, AuditEntry, DlqMessage, Item};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait SqlDriver: Send + Sync {
    // --- Config ---
    async fn get_config(&self, profile: &str, key: &str) -> Result<String>;
    async fn get_config_metadata(&self, profile: &str, key: &str) -> Result<(u64, i64)>;
    async fn get_config_full(&self, profile: &str, key: &str) -> Result<Item>;
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> Result<()>;
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>>;
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()>;

    // --- Secret ---
    async fn get_secret(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    async fn delete_secret(&self, profile: &str, key: &str) -> Result<()>;
    #[allow(dead_code)]
    async fn list_secrets(&self, profile: &str) -> Result<Vec<String>>;

    // --- Token Domain ---
    async fn get_access_token(&self, profile: &str) -> Result<crate::auth::models::Token>;
    async fn save_access_token(&self, profile: &str, token: crate::auth::models::Token) -> Result<()>;
    async fn delete_access_token(&self, profile: &str) -> Result<()>;
    async fn get_app_access_token(&self, app_key: &str) -> Result<crate::auth::models::Token>;
    async fn save_app_access_token(&self, app_key: &str, token: crate::auth::models::Token) -> Result<()>;

    // --- Ticket Domain ---
    async fn get_app_ticket(&self, app_key: &str) -> Result<crate::auth::models::Ticket>;
    async fn save_app_ticket(&self, app_key: &str, ticket: crate::auth::models::Ticket) -> Result<()>;
    async fn delete_app_ticket(&self, app_key: &str) -> Result<()>;

    // --- Permanent Code Domain ---
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> Result<String>;
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> Result<()>;
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> Result<String>;
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> Result<()>;

    // --- Legacy Support (Internal/Generic KV) ---
    async fn get_token(&self, profile: &str, key: &str) -> Result<String>;
    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> Result<()>;
    async fn delete_token(&self, profile: &str, key: &str) -> Result<()>;
    async fn list_tokens(&self, profile: &str) -> Result<Vec<String>>;

    // --- Audit ---
    async fn save_audit(&self, entry: &AuditEntry) -> Result<()>;
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<AuditEntry>>;

    // --- DLQ ---
    async fn push_dlq(&self, msg: &DlqMessage) -> Result<()>;
    #[allow(dead_code)]
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<DlqMessage>>;
    #[allow(dead_code)]
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<DlqMessage>>;
    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<DlqMessage>>;

    // --- Management ---
    async fn clear_profile(&self, profile: &str) -> Result<()>;
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()>;
    async fn list_all_profiles(&self) -> Result<Vec<String>>;

}

#[async_trait]
pub trait SqlBuilder: Send + Sync {
    fn scheme(&self) -> &str;
    async fn build(&self, url: &str) -> Result<Arc<dyn SqlDriver>>;
}

pub struct SqlBuilderRegistration {
    pub builder: &'static dyn SqlBuilder,
}

inventory::collect!(SqlBuilderRegistration);

pub struct SqlStore {
    driver: Arc<dyn SqlDriver>,
}

impl SqlStore {
    pub fn new(driver: Arc<dyn SqlDriver>) -> Self {
        Self { driver }
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

    pub async fn from_url(url: &str) -> Result<Self> {
        let mut scheme = url.split(':').next().ok_or_else(|| anyhow::anyhow!("Invalid database URL"))?;
        
        let actual_url = if scheme == "innerdb" {
            scheme = "sqlite";
            url.replace("innerdb://", "sqlite://")
        } else {
            url.to_string()
        };
        
        for reg in inventory::iter::<SqlBuilderRegistration> {
            if reg.builder.scheme() == scheme {
                let driver = reg.builder.build(&actual_url).await?;
                return Ok(Self::new(driver));
            }
        }

        Err(anyhow::anyhow!("Unsupported database scheme: {}. Supported: {:?}", scheme, Self::supported_schemes()))
    }
}

#[async_trait]
impl Store for SqlStore {
    // --- Config ---
    async fn get_config(&self, profile: &str, key: &str) -> Result<String> { self.driver.get_config(profile, key).await }
    async fn get_config_metadata(&self, profile: &str, key: &str) -> Result<(u64, i64)> { self.driver.get_config_metadata(profile, key).await }
    async fn get_config_full(&self, profile: &str, key: &str) -> Result<Item> { self.driver.get_config_full(profile, key).await }
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()> { self.driver.set_config(profile, key, value).await }
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, ev: u64) -> Result<()> { self.driver.set_config_conditional(profile, key, value, ev).await }
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>> { self.driver.list_configs(profile).await }
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()> { self.driver.delete_config(profile, key).await }

    // --- Secret ---
    async fn get_secret(&self, profile: &str, key: &str) -> Result<String> { self.driver.get_secret(profile, key).await }
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()> { self.driver.set_secret(profile, key, value).await }
    async fn delete_secret(&self, profile: &str, key: &str) -> Result<()> { self.driver.delete_secret(profile, key).await }
    async fn list_secrets(&self, profile: &str) -> Result<Vec<String>> { self.driver.list_secrets(profile).await }

    // --- Token ---
    async fn get_access_token(&self, profile: &str) -> Result<crate::auth::models::Token> { self.driver.get_access_token(profile).await }
    async fn save_access_token(&self, profile: &str, token: crate::auth::models::Token) -> Result<()> { self.driver.save_access_token(profile, token).await }
    async fn delete_access_token(&self, profile: &str) -> Result<()> { self.driver.delete_access_token(profile).await }
    async fn get_app_access_token(&self, app_key: &str) -> Result<crate::auth::models::Token> { self.driver.get_app_access_token(app_key).await }
    async fn save_app_access_token(&self, app_key: &str, token: crate::auth::models::Token) -> Result<()> { self.driver.save_app_access_token(app_key, token).await }

    // --- Ticket ---
    async fn get_app_ticket(&self, app_key: &str) -> Result<crate::auth::models::Ticket> { self.driver.get_app_ticket(app_key).await }
    async fn save_app_ticket(&self, app_key: &str, ticket: crate::auth::models::Ticket) -> Result<()> { self.driver.save_app_ticket(app_key, ticket).await }
    async fn delete_app_ticket(&self, app_key: &str) -> Result<()> { self.driver.delete_app_ticket(app_key).await }

    // --- Permanent Code Domain ---
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> Result<String> { self.driver.get_org_permanent_code(app_key, org_id).await }
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> Result<()> { self.driver.save_org_permanent_code(app_key, org_id, code).await }
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> Result<String> { self.driver.get_user_permanent_code(app_key, org_id, user_id).await }
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> Result<()> { self.driver.save_user_permanent_code(app_key, org_id, user_id, code).await }

    // --- Legacy Support ---
    async fn get_token(&self, profile: &str, key: &str) -> Result<String> { self.driver.get_token(profile, key).await }
    async fn set_token(&self, profile: &str, key: &str, value: &str, exp: u64) -> Result<()> { self.driver.set_token(profile, key, value, exp).await }
    async fn delete_token(&self, profile: &str, key: &str) -> Result<()> { self.driver.delete_token(profile, key).await }
    async fn list_tokens(&self, profile: &str) -> Result<Vec<String>> { self.driver.list_tokens(profile).await }

    // --- Audit ---
    async fn save_audit(&self, entry: &AuditEntry) -> Result<()> { self.driver.save_audit(entry).await }
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<AuditEntry>> { self.driver.list_audit(profile, limit).await }

    // --- DLQ ---
    async fn push_dlq(&self, msg: &DlqMessage) -> Result<()> { self.driver.push_dlq(msg).await }
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<DlqMessage>> { self.driver.pop_dlq(profile, topic).await }
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<DlqMessage>> { self.driver.list_dlq(profile, limit).await }
    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<DlqMessage>> { self.driver.list_all_dlq(profile).await }

    // --- Management ---
    async fn clear_profile(&self, profile: &str) -> Result<()> { self.driver.clear_profile(profile).await }
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()> { self.driver.rename_profile(old_name, new_name).await }
    async fn list_all_profiles(&self) -> Result<Vec<String>> { self.driver.list_all_profiles().await }

}
