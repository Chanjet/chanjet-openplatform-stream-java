use crate::file::core::FileStore;
use cowen_common::{models, CowenResult};
use std::path::Path;

pub struct MonolithicSealStore {
    inner: FileStore,
}

impl MonolithicSealStore {
    pub fn new<P: AsRef<Path>>(root_dir: P, fingerprint: &str) -> Self {
        Self {
            inner: FileStore::new(root_dir, Some(fingerprint)).unwrap(),
        }
    }

    pub fn inner(&self) -> &FileStore {
        &self.inner
    }
}

#[async_trait::async_trait]
impl cowen_common::store::Store for MonolithicSealStore {
    async fn shutdown(&self) -> CowenResult<()> {
        self.inner.shutdown().await
    }
    async fn get_config(&self, p: &str, k: &str) -> CowenResult<String> {
        self.inner.get_config(p, k).await
    }
    async fn get_config_metadata(&self, p: &str, k: &str) -> CowenResult<(u64, i64)> {
        self.inner.get_config_metadata(p, k).await
    }
    async fn get_config_full(&self, p: &str, k: &str) -> CowenResult<models::Item> {
        self.inner.get_config_full(p, k).await
    }
    async fn set_config(&self, p: &str, k: &str, v: &str) -> CowenResult<()> {
        self.inner.set_config(p, k, v).await
    }
    async fn set_config_conditional(&self, p: &str, k: &str, v: &str, ev: u64) -> CowenResult<()> {
        self.inner.set_config_conditional(p, k, v, ev).await
    }
    async fn delete_config(&self, p: &str, k: &str) -> CowenResult<()> {
        self.inner.delete_config(p, k).await
    }
    async fn list_configs(&self, p: &str) -> CowenResult<Vec<String>> {
        self.inner.list_configs(p).await
    }
    async fn get_secret(&self, p: &str, k: &str) -> CowenResult<String> {
        self.inner.get_secret(p, k).await
    }
    async fn set_secret(&self, p: &str, k: &str, v: &str) -> CowenResult<()> {
        self.inner.set_secret(p, k, v).await
    }
    async fn delete_secret(&self, p: &str, k: &str) -> CowenResult<()> {
        self.inner.delete_secret(p, k).await
    }
    async fn list_secrets(&self, p: &str) -> CowenResult<Vec<String>> {
        self.inner.list_secrets(p).await
    }
    async fn get_access_token(&self, p: &str) -> CowenResult<models::Token> {
        self.inner.get_access_token(p).await
    }
    async fn save_access_token(&self, p: &str, t: models::Token) -> CowenResult<()> {
        self.inner.save_access_token(p, t).await
    }
    async fn delete_access_token(&self, p: &str) -> CowenResult<()> {
        self.inner.delete_access_token(p).await
    }
    async fn get_refresh_token(&self, p: &str) -> CowenResult<models::Token> {
        self.inner.get_refresh_token(p).await
    }
    async fn save_refresh_token(&self, p: &str, t: models::Token) -> CowenResult<()> {
        self.inner.save_refresh_token(p, t).await
    }
    async fn delete_refresh_token(&self, p: &str) -> CowenResult<()> {
        self.inner.delete_refresh_token(p).await
    }
    async fn get_app_access_token(&self, k: &str) -> CowenResult<models::Token> {
        self.inner.get_app_access_token(k).await
    }
    async fn save_app_access_token(&self, k: &str, t: models::Token) -> CowenResult<()> {
        self.inner.save_app_access_token(k, t).await
    }
    async fn delete_app_access_token(&self, k: &str) -> CowenResult<()> {
        self.inner.delete_app_access_token(k).await
    }
    async fn get_app_ticket(&self, k: &str) -> CowenResult<models::Ticket> {
        self.inner.get_app_ticket(k).await
    }
    async fn save_app_ticket(&self, k: &str, t: models::Ticket) -> CowenResult<()> {
        self.inner.save_app_ticket(k, t).await
    }
    async fn delete_app_ticket(&self, k: &str) -> CowenResult<()> {
        self.inner.delete_app_ticket(k).await
    }
    async fn get_org_permanent_code(&self, k: &str, org: &str) -> CowenResult<String> {
        self.inner.get_org_permanent_code(k, org).await
    }
    async fn save_org_permanent_code(&self, k: &str, org: &str, c: &str) -> CowenResult<()> {
        self.inner.save_org_permanent_code(k, org, c).await
    }
    async fn get_user_permanent_code(&self, k: &str, org: &str, user: &str) -> CowenResult<String> {
        self.inner.get_user_permanent_code(k, org, user).await
    }
    async fn save_user_permanent_code(
        &self,
        k: &str,
        org: &str,
        user: &str,
        c: &str,
    ) -> CowenResult<()> {
        self.inner.save_user_permanent_code(k, org, user, c).await
    }
    async fn get_token(&self, p: &str, k: &str) -> CowenResult<String> {
        self.inner.get_token(p, k).await
    }
    async fn set_token(&self, p: &str, k: &str, v: &str, exp: u64) -> CowenResult<()> {
        self.inner.set_token(p, k, v, exp).await
    }
    async fn delete_token(&self, p: &str, k: &str) -> CowenResult<()> {
        self.inner.delete_token(p, k).await
    }
    async fn list_tokens(&self, p: &str) -> CowenResult<Vec<String>> {
        self.inner.list_tokens(p).await
    }
    async fn save_audit(&self, e: &models::AuditEntry) -> CowenResult<()> {
        self.inner.save_audit(e).await
    }
    async fn list_audit(&self, p: &str, limit: usize) -> CowenResult<Vec<models::AuditEntry>> {
        self.inner.list_audit(p, limit).await
    }
    async fn push_dlq(&self, m: &models::DlqMessage) -> CowenResult<()> {
        self.inner.push_dlq(m).await
    }
    async fn pop_dlq(&self, p: &str, t: &str) -> CowenResult<Option<models::DlqMessage>> {
        self.inner.pop_dlq(p, t).await
    }
    async fn list_dlq(&self, p: &str, limit: usize) -> CowenResult<Vec<models::DlqMessage>> {
        self.inner.list_dlq(p, limit).await
    }
    async fn list_all_dlq(&self, p: &str) -> CowenResult<Vec<models::DlqMessage>> {
        self.inner.list_all_dlq(p).await
    }
    async fn get_dlq_by_id(&self, id: i64) -> CowenResult<Option<models::DlqMessage>> {
        self.inner.get_dlq_by_id(id).await
    }
    async fn list_dlq_paged(
        &self,
        p: &str,
        offset: usize,
        limit: usize,
    ) -> CowenResult<Vec<models::DlqMessage>> {
        self.inner.list_dlq_paged(p, offset, limit).await
    }
    async fn delete_dlq_by_id(&self, id: i64) -> CowenResult<()> {
        self.inner.delete_dlq_by_id(id).await
    }
    async fn migrate(&self) -> CowenResult<()> {
        self.inner.migrate().await
    }
    async fn clear_profile(&self, p: &str) -> CowenResult<()> {
        self.inner.clear_profile(p).await
    }
    async fn rename_profile(&self, old: &str, new: &str) -> CowenResult<()> {
        self.inner.rename_profile(old, new).await
    }
    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> {
        self.inner.list_all_profiles().await
    }
    async fn raw_del(&self, key: &str) -> CowenResult<()> {
        self.inner.raw_del(key).await
    }
    fn name(&self) -> &str {
        "sealed"
    }
    fn description(&self) -> String {
        format!("Encrypted Local File Store at {:?}", self.inner.root_dir())
    }
}
