use crate::Store;
use async_trait::async_trait;
use cowen_common::models::{AuditEntry, DlqMessage, Item, Ticket, Token};
use cowen_common::CowenResult;
use std::sync::Arc;

pub struct HybridStore {
    persistence: Arc<dyn Store>,
    cache: Arc<dyn Store>,
}

impl HybridStore {
    pub fn new(persistence: Arc<dyn Store>, cache: Arc<dyn Store>) -> Self {
        Self { persistence, cache }
    }
}

#[async_trait]
impl Store for HybridStore {
    async fn get_config(&self, p: &str, k: &str) -> CowenResult<String> {
        if let Ok(v) = self.cache.get_config(p, k).await {
            return Ok(v);
        }
        let v = self.persistence.get_config(p, k).await?;
        let _ = self.cache.set_config(p, k, &v).await;
        Ok(v)
    }
    async fn get_config_metadata(&self, p: &str, k: &str) -> CowenResult<(u64, i64)> {
        self.persistence.get_config_metadata(p, k).await
    }
    async fn get_config_full(&self, p: &str, k: &str) -> CowenResult<Item> {
        self.persistence.get_config_full(p, k).await
    }
    async fn set_config(&self, p: &str, k: &str, v: &str) -> CowenResult<()> {
        self.persistence.set_config(p, k, v).await?;
        let _ = self.cache.set_config(p, k, v).await;
        Ok(())
    }
    async fn set_config_conditional(&self, p: &str, k: &str, v: &str, ev: u64) -> CowenResult<()> {
        self.persistence.set_config_conditional(p, k, v, ev).await?;
        let _ = self.cache.set_config(p, k, v).await;
        Ok(())
    }
    async fn delete_config(&self, p: &str, k: &str) -> CowenResult<()> {
        self.persistence.delete_config(p, k).await?;
        let _ = self.cache.delete_config(p, k).await;
        Ok(())
    }
    async fn list_configs(&self, p: &str) -> CowenResult<Vec<String>> {
        self.persistence.list_configs(p).await
    }

    async fn get_secret(&self, p: &str, k: &str) -> CowenResult<String> {
        self.persistence.get_secret(p, k).await
    }
    async fn set_secret(&self, p: &str, k: &str, v: &str) -> CowenResult<()> {
        self.persistence.set_secret(p, k, v).await
    }
    async fn delete_secret(&self, p: &str, k: &str) -> CowenResult<()> {
        self.persistence.delete_secret(p, k).await
    }
    async fn list_secrets(&self, p: &str) -> CowenResult<Vec<String>> {
        self.persistence.list_secrets(p).await
    }

    async fn get_access_token(&self, p: &str) -> CowenResult<Token> {
        if let Ok(v) = self.cache.get_access_token(p).await {
            return Ok(v);
        }
        let v = self.persistence.get_access_token(p).await?;
        let _ = self.cache.save_access_token(p, v.clone()).await;
        Ok(v)
    }
    async fn save_access_token(&self, p: &str, t: Token) -> CowenResult<()> {
        self.persistence.save_access_token(p, t.clone()).await?;
        let _ = self.cache.save_access_token(p, t).await;
        Ok(())
    }
    async fn delete_access_token(&self, p: &str) -> CowenResult<()> {
        self.persistence.delete_access_token(p).await?;
        let _ = self.cache.delete_access_token(p).await;
        Ok(())
    }

    async fn get_refresh_token(&self, p: &str) -> CowenResult<Token> {
        if let Ok(v) = self.cache.get_refresh_token(p).await {
            return Ok(v);
        }
        let v = self.persistence.get_refresh_token(p).await?;
        let _ = self.cache.save_refresh_token(p, v.clone()).await;
        Ok(v)
    }
    async fn save_refresh_token(&self, p: &str, t: Token) -> CowenResult<()> {
        self.persistence.save_refresh_token(p, t.clone()).await?;
        let _ = self.cache.save_refresh_token(p, t).await;
        Ok(())
    }
    async fn delete_refresh_token(&self, p: &str) -> CowenResult<()> {
        self.persistence.delete_refresh_token(p).await?;
        let _ = self.cache.delete_refresh_token(p).await;
        Ok(())
    }

    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<Token> {
        if let Ok(v) = self.cache.get_app_access_token(app_key).await {
            return Ok(v);
        }
        let v = self.persistence.get_app_access_token(app_key).await?;
        let _ = self.cache.save_app_access_token(app_key, v.clone()).await;
        Ok(v)
    }
    async fn save_app_access_token(&self, app_key: &str, t: Token) -> CowenResult<()> {
        self.persistence
            .save_app_access_token(app_key, t.clone())
            .await?;
        let _ = self.cache.save_app_access_token(app_key, t).await;
        Ok(())
    }
    async fn delete_app_access_token(&self, app_key: &str) -> CowenResult<()> {
        self.persistence.delete_app_access_token(app_key).await?;
        let _ = self.cache.delete_app_access_token(app_key).await;
        Ok(())
    }

    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<Ticket> {
        if let Ok(v) = self.cache.get_app_ticket(app_key).await {
            return Ok(v);
        }
        let v = self.persistence.get_app_ticket(app_key).await?;
        let _ = self.cache.save_app_ticket(app_key, v.clone()).await;
        Ok(v)
    }
    async fn save_app_ticket(&self, app_key: &str, t: Ticket) -> CowenResult<()> {
        self.persistence.save_app_ticket(app_key, t.clone()).await?;
        let _ = self.cache.save_app_ticket(app_key, t).await;
        Ok(())
    }
    async fn delete_app_ticket(&self, app_key: &str) -> CowenResult<()> {
        self.persistence.delete_app_ticket(app_key).await?;
        let _ = self.cache.delete_app_ticket(app_key).await;
        Ok(())
    }

    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> CowenResult<String> {
        if let Ok(v) = self.cache.get_org_permanent_code(app_key, org_id).await {
            return Ok(v);
        }
        let v = self
            .persistence
            .get_org_permanent_code(app_key, org_id)
            .await?;
        let _ = self
            .cache
            .save_org_permanent_code(app_key, org_id, &v)
            .await;
        Ok(v)
    }
    async fn save_org_permanent_code(
        &self,
        app_key: &str,
        org_id: &str,
        code: &str,
    ) -> CowenResult<()> {
        self.persistence
            .save_org_permanent_code(app_key, org_id, code)
            .await?;
        let _ = self
            .cache
            .save_org_permanent_code(app_key, org_id, code)
            .await;
        Ok(())
    }
    async fn get_user_permanent_code(
        &self,
        app_key: &str,
        org_id: &str,
        user_id: &str,
    ) -> CowenResult<String> {
        if let Ok(v) = self
            .cache
            .get_user_permanent_code(app_key, org_id, user_id)
            .await
        {
            return Ok(v);
        }
        let v = self
            .persistence
            .get_user_permanent_code(app_key, org_id, user_id)
            .await?;
        let _ = self
            .cache
            .save_user_permanent_code(app_key, org_id, user_id, &v)
            .await;
        Ok(v)
    }
    async fn save_user_permanent_code(
        &self,
        app_key: &str,
        org_id: &str,
        user_id: &str,
        code: &str,
    ) -> CowenResult<()> {
        self.persistence
            .save_user_permanent_code(app_key, org_id, user_id, code)
            .await?;
        let _ = self
            .cache
            .save_user_permanent_code(app_key, org_id, user_id, code)
            .await;
        Ok(())
    }

    async fn get_token(&self, p: &str, k: &str) -> CowenResult<String> {
        if let Ok(v) = self.cache.get_token(p, k).await {
            return Ok(v);
        }
        let v = self.persistence.get_token(p, k).await?;
        let _ = self.cache.set_token(p, k, &v, 3600).await;
        Ok(v)
    }
    async fn set_token(&self, p: &str, k: &str, v: &str, exp: u64) -> CowenResult<()> {
        self.persistence.set_token(p, k, v, exp).await?;
        let _ = self.cache.set_token(p, k, v, exp).await;
        Ok(())
    }
    async fn delete_token(&self, p: &str, k: &str) -> CowenResult<()> {
        self.persistence.delete_token(p, k).await?;
        let _ = self.cache.delete_token(p, k).await;
        Ok(())
    }
    async fn list_tokens(&self, p: &str) -> CowenResult<Vec<String>> {
        self.persistence.list_tokens(p).await
    }

    async fn save_audit(&self, e: &AuditEntry) -> CowenResult<()> {
        self.persistence.save_audit(e).await
    }
    async fn list_audit(&self, p: &str, l: usize) -> CowenResult<Vec<AuditEntry>> {
        self.persistence.list_audit(p, l).await
    }
    async fn push_dlq(&self, m: &DlqMessage) -> CowenResult<()> {
        self.persistence.push_dlq(m).await
    }
    async fn pop_dlq(&self, p: &str, t: &str) -> CowenResult<Option<DlqMessage>> {
        self.persistence.pop_dlq(p, t).await
    }
    async fn list_dlq(&self, p: &str, l: usize) -> CowenResult<Vec<DlqMessage>> {
        self.persistence.list_dlq(p, l).await
    }
    async fn list_all_dlq(&self, p: &str) -> CowenResult<Vec<DlqMessage>> {
        self.persistence.list_all_dlq(p).await
    }
    async fn get_dlq_by_id(&self, id: i64) -> CowenResult<Option<DlqMessage>> {
        self.persistence.get_dlq_by_id(id).await
    }
    async fn list_dlq_paged(&self, p: &str, o: usize, l: usize) -> CowenResult<Vec<DlqMessage>> {
        self.persistence.list_dlq_paged(p, o, l).await
    }
    async fn delete_dlq_by_id(&self, id: i64) -> CowenResult<()> {
        self.persistence.delete_dlq_by_id(id).await
    }

    async fn migrate(&self) -> CowenResult<()> {
        self.persistence.migrate().await
    }

    async fn clear_profile(&self, profile: &str) -> CowenResult<()> {
        let _ = self.cache.clear_profile(profile).await;
        self.persistence.clear_profile(profile).await
    }
    async fn raw_del(&self, key: &str) -> CowenResult<()> {
        let _ = self.cache.raw_del(key).await;
        self.persistence.raw_del(key).await
    }
    async fn rename_profile(&self, old: &str, new: &str) -> CowenResult<()> {
        let _ = self.cache.rename_profile(old, new).await;
        self.persistence.rename_profile(old, new).await
    }
    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> {
        self.persistence.list_all_profiles().await
    }

    fn name(&self) -> &str {
        "Hybrid"
    }

    fn description(&self) -> String {
        format!(
            "Hybrid (Cache: {}, Persistence: {})",
            self.cache.name(),
            self.persistence.name()
        )
    }
}
