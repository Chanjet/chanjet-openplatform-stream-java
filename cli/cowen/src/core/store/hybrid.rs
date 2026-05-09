use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use super::{Store, AuditEntry, DlqMessage, Item};

pub struct HybridStore {
    cache: Arc<dyn Store>,
    persistence: Arc<dyn Store>,
}

impl HybridStore {
    pub fn new(cache: Arc<dyn Store>, persistence: Arc<dyn Store>) -> Self {
        Self { cache, persistence }
    }
}

#[async_trait]
impl Store for HybridStore {
    async fn get_config(&self, p: &str, k: &str) -> Result<String> {
        match self.cache.get_config(p, k).await {
            Ok(v) => Ok(v),
            Err(_) => {
                let v = self.persistence.get_config(p, k).await?;
                let _ = self.cache.set_config(p, k, &v).await;
                Ok(v)
            }
        }
    }
    async fn get_config_metadata(&self, p: &str, k: &str) -> Result<(u64, i64)> {
        self.persistence.get_config_metadata(p, k).await
    }
    async fn get_config_full(&self, p: &str, k: &str) -> Result<Item> {
        self.persistence.get_config_full(p, k).await
    }
    async fn set_config(&self, p: &str, k: &str, v: &str) -> Result<()> {
        self.persistence.set_config(p, k, v).await?;
        let _ = self.cache.set_config(p, k, v).await;
        Ok(())
    }
    async fn set_config_conditional(&self, p: &str, k: &str, v: &str, ev: u64) -> Result<()> {
        self.persistence.set_config_conditional(p, k, v, ev).await?;
        let _ = self.cache.delete_config(p, k).await;
        Ok(())
    }
    async fn delete_config(&self, p: &str, k: &str) -> Result<()> {
        let _ = self.cache.delete_config(p, k).await;
        self.persistence.delete_config(p, k).await
    }
    async fn list_configs(&self, p: &str) -> Result<Vec<String>> {
        self.persistence.list_configs(p).await
    }

    async fn get_secret(&self, p: &str, k: &str) -> Result<String> {
        self.persistence.get_secret(p, k).await
    }
    async fn set_secret(&self, p: &str, k: &str, v: &str) -> Result<()> {
        self.persistence.set_secret(p, k, v).await
    }
    async fn delete_secret(&self, p: &str, k: &str) -> Result<()> {
        self.persistence.delete_secret(p, k).await
    }
    async fn list_secrets(&self, p: &str) -> Result<Vec<String>> {
        self.persistence.list_secrets(p).await
    }

    async fn get_access_token(&self, p: &str) -> Result<crate::auth::models::Token> {
        match self.cache.get_access_token(p).await {
            Ok(v) => Ok(v),
            Err(_) => {
                let v = self.persistence.get_access_token(p).await?;
                let _ = self.cache.save_access_token(p, v.clone()).await;
                Ok(v)
            }
        }
    }
    async fn save_access_token(&self, p: &str, t: crate::auth::models::Token) -> Result<()> {
        self.persistence.save_access_token(p, t.clone()).await?;
        let _ = self.cache.save_access_token(p, t).await;
        Ok(())
    }
    async fn delete_access_token(&self, p: &str) -> Result<()> {
        let _ = self.cache.delete_access_token(p).await;
        self.persistence.delete_access_token(p).await
    }
    async fn get_app_access_token(&self, app_key: &str) -> Result<crate::auth::models::Token> {
        match self.cache.get_app_access_token(app_key).await {
            Ok(v) => Ok(v),
            Err(_) => {
                let v = self.persistence.get_app_access_token(app_key).await?;
                let _ = self.cache.save_app_access_token(app_key, v.clone()).await;
                Ok(v)
            }
        }
    }
    async fn save_app_access_token(&self, app_key: &str, t: crate::auth::models::Token) -> Result<()> {
        self.persistence.save_app_access_token(app_key, t.clone()).await?;
        let _ = self.cache.save_app_access_token(app_key, t).await;
        Ok(())
    }

    async fn get_app_ticket(&self, app_key: &str) -> Result<crate::auth::models::Ticket> {
        match self.cache.get_app_ticket(app_key).await {
            Ok(v) => Ok(v),
            Err(_) => {
                let v = self.persistence.get_app_ticket(app_key).await?;
                let _ = self.cache.save_app_ticket(app_key, v.clone()).await;
                Ok(v)
            }
        }
    }
    async fn save_app_ticket(&self, app_key: &str, t: crate::auth::models::Ticket) -> Result<()> {
        self.persistence.save_app_ticket(app_key, t.clone()).await?;
        let _ = self.cache.save_app_ticket(app_key, t).await;
        Ok(())
    }
    async fn delete_app_ticket(&self, app_key: &str) -> Result<()> {
        let _ = self.cache.delete_app_ticket(app_key).await;
        self.persistence.delete_app_ticket(app_key).await
    }

    // --- Permanent Code ---
    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> Result<String> {
        match self.cache.get_org_permanent_code(app_key, org_id).await {
            Ok(v) => Ok(v),
            Err(_) => {
                let v = self.persistence.get_org_permanent_code(app_key, org_id).await?;
                let _ = self.cache.save_org_permanent_code(app_key, org_id, &v).await;
                Ok(v)
            }
        }
    }
    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> Result<()> {
        self.persistence.save_org_permanent_code(app_key, org_id, code).await?;
        let _ = self.cache.save_org_permanent_code(app_key, org_id, code).await;
        Ok(())
    }
    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> Result<String> {
        match self.cache.get_user_permanent_code(app_key, org_id, user_id).await {
            Ok(v) => Ok(v),
            Err(_) => {
                let v = self.persistence.get_user_permanent_code(app_key, org_id, user_id).await?;
                let _ = self.cache.save_user_permanent_code(app_key, org_id, user_id, &v).await;
                Ok(v)
            }
        }
    }
    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> Result<()> {
        self.persistence.save_user_permanent_code(app_key, org_id, user_id, code).await?;
        let _ = self.cache.save_user_permanent_code(app_key, org_id, user_id, code).await;
        Ok(())
    }

    async fn get_token(&self, p: &str, k: &str) -> Result<String> {
        match self.cache.get_token(p, k).await {
            Ok(v) => Ok(v),
            Err(_) => {
                let v = self.persistence.get_token(p, k).await?;
                let _ = self.cache.set_token(p, k, &v, 3600).await;
                Ok(v)
            }
        }
    }
    async fn set_token(&self, p: &str, k: &str, v: &str, exp: u64) -> Result<()> {
        self.persistence.set_token(p, k, v, exp).await?;
        let _ = self.cache.set_token(p, k, v, exp).await;
        Ok(())
    }
    async fn delete_token(&self, p: &str, k: &str) -> Result<()> {
        let _ = self.cache.delete_token(p, k).await;
        self.persistence.delete_token(p, k).await
    }
    async fn list_tokens(&self, p: &str) -> Result<Vec<String>> {
        self.persistence.list_tokens(p).await
    }

    async fn save_audit(&self, e: &AuditEntry) -> Result<()> {
        let _ = self.cache.save_audit(e).await;
        self.persistence.save_audit(e).await
    }
    async fn list_audit(&self, p: &str, l: usize) -> Result<Vec<AuditEntry>> {
        self.persistence.list_audit(p, l).await
    }

    async fn push_dlq(&self, m: &DlqMessage) -> Result<()> {
        self.persistence.push_dlq(m).await
    }
    async fn pop_dlq(&self, p: &str, t: &str) -> Result<Option<DlqMessage>> {
        self.persistence.pop_dlq(p, t).await
    }
    async fn list_dlq(&self, p: &str, l: usize) -> Result<Vec<DlqMessage>> {
        self.persistence.list_dlq(p, l).await
    }
    async fn list_all_dlq(&self, p: &str) -> Result<Vec<DlqMessage>> {
        self.persistence.list_all_dlq(p).await
    }


    async fn clear_profile(&self, p: &str) -> Result<()> {
        let _ = self.cache.clear_profile(p).await;
        self.persistence.clear_profile(p).await
    }
    async fn rename_profile(&self, o: &str, n: &str) -> Result<()> {
        let _ = self.cache.rename_profile(o, n).await;
        self.persistence.rename_profile(o, n).await
    }
    async fn list_all_profiles(&self) -> Result<Vec<String>> {
        self.persistence.list_all_profiles().await
    }

}
