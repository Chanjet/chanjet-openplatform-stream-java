use async_trait::async_trait;
use cowen_common::{CowenError, CowenResult};

#[cfg(feature = "mssql")]
mod mssql;
#[cfg(feature = "mysql")]
mod mysql;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "sqlite")]
pub mod sqlite;

pub mod migration_trait;
#[macro_use]
pub mod macros;
use crate::Store;
use cowen_common::models::{AuditEntry, DlqMessage, Item, Ticket, Token};
use std::sync::Arc;

#[async_trait]
pub trait SqlDriver: Send + Sync {
    async fn shutdown(&self) -> CowenResult<()> {
        Ok(())
    }

    // --- Config ---
    async fn get_config(&self, prf: &str, k: &str) -> CowenResult<String>;
    async fn get_config_metadata(&self, prf: &str, k: &str) -> CowenResult<(u64, i64)>;
    async fn get_config_full(&self, prf: &str, k: &str) -> CowenResult<Item>;
    async fn set_config(&self, prf: &str, k: &str, val: &str) -> CowenResult<()>;
    async fn set_config_conditional(
        &self,
        prf: &str,
        k: &str,
        val: &str,
        exp_ver: u64,
    ) -> CowenResult<()>;
    async fn list_configs(&self, prf: &str) -> CowenResult<Vec<String>>;
    async fn delete_config(&self, prf: &str, k: &str) -> CowenResult<()>;

    // --- Secret ---
    async fn get_secret(&self, prf: &str, k: &str) -> CowenResult<String>;
    async fn set_secret(&self, prf: &str, k: &str, val: &str) -> CowenResult<()>;
    async fn delete_secret(&self, prf: &str, k: &str) -> CowenResult<()>;
    async fn list_secrets(&self, prf: &str) -> CowenResult<Vec<String>>;

    // --- Token Domain ---
    async fn get_access_token(&self, prf: &str) -> CowenResult<Token>;
    async fn save_access_token(&self, prf: &str, tok: Token) -> CowenResult<()>;
    async fn delete_access_token(&self, prf: &str) -> CowenResult<()>;
    async fn get_refresh_token(&self, prf: &str) -> CowenResult<Token>;
    async fn save_refresh_token(&self, prf: &str, tok: Token) -> CowenResult<()>;
    async fn delete_refresh_token(&self, prf: &str) -> CowenResult<()>;
    async fn get_app_access_token(&self, ak: &str) -> CowenResult<Token>;
    async fn save_app_access_token(&self, ak: &str, tok: Token) -> CowenResult<()>;
    async fn delete_app_access_token(&self, ak: &str) -> CowenResult<()>;

    // --- Ticket Domain ---
    async fn get_app_ticket(&self, ak: &str) -> CowenResult<Ticket>;
    async fn save_app_ticket(&self, ak: &str, tkt: Ticket) -> CowenResult<()>;
    async fn delete_app_ticket(&self, ak: &str) -> CowenResult<()>;

    // --- Permanent Code Domain ---
    async fn get_org_permanent_code(&self, ak: &str, oid: &str) -> CowenResult<String>;
    async fn save_org_permanent_code(&self, ak: &str, oid: &str, val: &str) -> CowenResult<()>;
    async fn get_user_permanent_code(&self, ak: &str, oid: &str, uid: &str) -> CowenResult<String>;
    async fn save_user_permanent_code(
        &self,
        ak: &str,
        oid: &str,
        uid: &str,
        val: &str,
    ) -> CowenResult<()>;

    // --- Legacy Support ---
    async fn get_token(&self, prf: &str, k: &str) -> CowenResult<String>;
    async fn set_token(&self, prf: &str, k: &str, val: &str, exp_secs: u64) -> CowenResult<()>;
    async fn delete_token(&self, prf: &str, k: &str) -> CowenResult<()>;
    async fn list_tokens(&self, prf: &str) -> CowenResult<Vec<String>>;

    // --- Audit ---
    async fn save_audit(&self, ent: &AuditEntry) -> CowenResult<()>;
    async fn list_audit(&self, prf: &str, lim: usize) -> CowenResult<Vec<AuditEntry>>;

    // --- DLQ ---
    async fn push_dlq(&self, m: &DlqMessage) -> CowenResult<()>;
    async fn pop_dlq(&self, prf: &str, t: &str) -> CowenResult<Option<DlqMessage>>;
    async fn list_dlq(&self, prf: &str, lim: usize) -> CowenResult<Vec<DlqMessage>>;
    async fn list_all_dlq(&self, prf: &str) -> CowenResult<Vec<DlqMessage>>;
    async fn get_dlq_by_id(&self, id: i64) -> CowenResult<Option<DlqMessage>>;
    async fn list_dlq_paged(
        &self,
        prf: &str,
        off: usize,
        lim: usize,
    ) -> CowenResult<Vec<DlqMessage>>;
    async fn delete_dlq_by_id(&self, id: i64) -> CowenResult<()>;

    // --- Schema Migration ---
    async fn migrate(&self) -> CowenResult<()>;

    // --- Management ---
    async fn clear_profile(&self, prf: &str) -> CowenResult<()>;
    async fn rename_profile(&self, old: &str, new: &str) -> CowenResult<()>;
    async fn list_all_profiles(&self) -> CowenResult<Vec<String>>;
    async fn raw_del(&self, k: &str) -> CowenResult<()>;
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
        Self {
            driver,
            name: name.to_string(),
            url: url.to_string(),
        }
    }

    pub fn supported_schemes() -> Vec<String> {
        inventory::iter::<SqlBuilderRegistration>
            .into_iter()
            .map(|reg| reg.builder.scheme().to_string())
            .collect()
    }

    pub fn is_supported(scheme: &str) -> bool {
        let scheme = if scheme == "innerdb" {
            "sqlite"
        } else {
            scheme
        };
        inventory::iter::<SqlBuilderRegistration>
            .into_iter()
            .any(|reg| reg.builder.scheme() == scheme)
    }

    pub async fn from_url(url: &str) -> CowenResult<Self> {
        let mut scheme = url
            .split(":")
            .next()
            .ok_or_else(|| CowenError::api("Invalid database URL"))?
            .to_string();
        let mut actual_url = if scheme == "innerdb" {
            scheme = "sqlite".to_string();
            url.replace("innerdb://", "sqlite:")
        } else {
            url.to_string()
        };

        if actual_url.starts_with("sqlite://") {
            actual_url = actual_url.replace("sqlite://", "sqlite:");
        } else if (scheme == "mysql" || scheme == "postgres") && !actual_url.contains("://") {
            actual_url = actual_url.replace(&format!("{}:", scheme), &format!("{}://", scheme));
        }

        for reg in inventory::iter::<SqlBuilderRegistration> {
            if reg.builder.scheme() == scheme {
                let driver = reg.builder.build(&actual_url).await?;
                return Ok(Self::new(driver, &scheme, url));
            }
        }

        Err(CowenError::api(format!(
            "Unsupported database scheme: {}. Supported: {:?}",
            scheme,
            Self::supported_schemes()
        )))
    }
}

#[async_trait]
impl Store for SqlStore {
    async fn shutdown(&self) -> CowenResult<()> {
        self.driver.shutdown().await
    }

    async fn get_config(&self, p: &str, k: &str) -> CowenResult<String> {
        self.driver.get_config(p, k).await
    }
    async fn get_config_metadata(&self, p: &str, k: &str) -> CowenResult<(u64, i64)> {
        self.driver.get_config_metadata(p, k).await
    }
    async fn get_config_full(&self, p: &str, k: &str) -> CowenResult<Item> {
        self.driver.get_config_full(p, k).await
    }
    async fn set_config(&self, p: &str, k: &str, v: &str) -> CowenResult<()> {
        self.driver.set_config(p, k, v).await
    }
    async fn set_config_conditional(&self, p: &str, k: &str, v: &str, ev: u64) -> CowenResult<()> {
        self.driver.set_config_conditional(p, k, v, ev).await
    }
    async fn list_configs(&self, p: &str) -> CowenResult<Vec<String>> {
        self.driver.list_configs(p).await
    }
    async fn delete_config(&self, p: &str, k: &str) -> CowenResult<()> {
        self.driver.delete_config(p, k).await
    }

    async fn get_secret(&self, p: &str, k: &str) -> CowenResult<String> {
        self.driver.get_secret(p, k).await
    }
    async fn set_secret(&self, p: &str, k: &str, v: &str) -> CowenResult<()> {
        self.driver.set_secret(p, k, v).await
    }
    async fn delete_secret(&self, p: &str, k: &str) -> CowenResult<()> {
        self.driver.delete_secret(p, k).await
    }
    async fn list_secrets(&self, p: &str) -> CowenResult<Vec<String>> {
        self.driver.list_secrets(p).await
    }

    async fn get_access_token(&self, p: &str) -> CowenResult<Token> {
        self.driver.get_access_token(p).await
    }
    async fn save_access_token(&self, p: &str, t: Token) -> CowenResult<()> {
        self.driver.save_access_token(p, t).await
    }
    async fn delete_access_token(&self, p: &str) -> CowenResult<()> {
        self.driver.delete_access_token(p).await
    }
    async fn get_refresh_token(&self, p: &str) -> CowenResult<Token> {
        self.driver.get_refresh_token(p).await
    }
    async fn save_refresh_token(&self, p: &str, t: Token) -> CowenResult<()> {
        self.driver.save_refresh_token(p, t).await
    }
    async fn delete_refresh_token(&self, p: &str) -> CowenResult<()> {
        self.driver.delete_refresh_token(p).await
    }
    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<Token> {
        self.driver.get_app_access_token(app_key).await
    }
    async fn save_app_access_token(&self, app_key: &str, t: Token) -> CowenResult<()> {
        self.driver.save_app_access_token(app_key, t).await
    }
    async fn delete_app_access_token(&self, app_key: &str) -> CowenResult<()> {
        self.driver.delete_app_access_token(app_key).await
    }

    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<Ticket> {
        self.driver.get_app_ticket(app_key).await
    }
    async fn save_app_ticket(&self, app_key: &str, t: Ticket) -> CowenResult<()> {
        self.driver.save_app_ticket(app_key, t).await
    }
    async fn delete_app_ticket(&self, app_key: &str) -> CowenResult<()> {
        self.driver.delete_app_ticket(app_key).await
    }

    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> CowenResult<String> {
        self.driver.get_org_permanent_code(app_key, org_id).await
    }
    async fn save_org_permanent_code(
        &self,
        app_key: &str,
        org_id: &str,
        c: &str,
    ) -> CowenResult<()> {
        self.driver
            .save_org_permanent_code(app_key, org_id, c)
            .await
    }
    async fn get_user_permanent_code(
        &self,
        app_key: &str,
        org_id: &str,
        user_id: &str,
    ) -> CowenResult<String> {
        self.driver
            .get_user_permanent_code(app_key, org_id, user_id)
            .await
    }
    async fn save_user_permanent_code(
        &self,
        app_key: &str,
        org_id: &str,
        user_id: &str,
        c: &str,
    ) -> CowenResult<()> {
        self.driver
            .save_user_permanent_code(app_key, org_id, user_id, c)
            .await
    }

    async fn get_token(&self, p: &str, k: &str) -> CowenResult<String> {
        self.driver.get_token(p, k).await
    }
    async fn set_token(&self, p: &str, k: &str, v: &str, e: u64) -> CowenResult<()> {
        self.driver.set_token(p, k, v, e).await
    }
    async fn delete_token(&self, p: &str, k: &str) -> CowenResult<()> {
        self.driver.delete_token(p, k).await
    }
    async fn list_tokens(&self, p: &str) -> CowenResult<Vec<String>> {
        self.driver.list_tokens(p).await
    }

    async fn save_audit(&self, e: &AuditEntry) -> CowenResult<()> {
        self.driver.save_audit(e).await
    }
    async fn list_audit(&self, p: &str, l: usize) -> CowenResult<Vec<AuditEntry>> {
        self.driver.list_audit(p, l).await
    }
    async fn push_dlq(&self, m: &DlqMessage) -> CowenResult<()> {
        self.driver.push_dlq(m).await
    }
    async fn pop_dlq(&self, p: &str, t: &str) -> CowenResult<Option<DlqMessage>> {
        self.driver.pop_dlq(p, t).await
    }
    async fn list_dlq(&self, p: &str, l: usize) -> CowenResult<Vec<DlqMessage>> {
        self.driver.list_dlq(p, l).await
    }
    async fn list_all_dlq(&self, p: &str) -> CowenResult<Vec<DlqMessage>> {
        self.driver.list_all_dlq(p).await
    }
    async fn get_dlq_by_id(&self, id: i64) -> CowenResult<Option<DlqMessage>> {
        self.driver.get_dlq_by_id(id).await
    }
    async fn list_dlq_paged(&self, p: &str, o: usize, l: usize) -> CowenResult<Vec<DlqMessage>> {
        self.driver.list_dlq_paged(p, o, l).await
    }
    async fn delete_dlq_by_id(&self, id: i64) -> CowenResult<()> {
        self.driver.delete_dlq_by_id(id).await
    }

    async fn migrate(&self) -> CowenResult<()> {
        self.driver.migrate().await
    }

    async fn clear_profile(&self, p: &str) -> CowenResult<()> {
        self.driver.clear_profile(p).await
    }
    async fn rename_profile(&self, o: &str, n: &str) -> CowenResult<()> {
        self.driver.rename_profile(o, n).await
    }
    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> {
        self.driver.list_all_profiles().await
    }
    async fn raw_del(&self, k: &str) -> CowenResult<()> {
        self.driver.raw_del(k).await
    }

    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> String {
        format!("SQL Database ({}): {}", self.name, self.url)
    }
}
