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

    async fn get_cache(&self, p: &str, k: &str) -> Result<String> {
        match self.cache.get_cache(p, k).await {
            Ok(v) => Ok(v),
            Err(_) => {
                let v = self.persistence.get_cache(p, k).await?;
                let _ = self.cache.set_cache(p, k, &v, 3600).await;
                Ok(v)
            }
        }
    }
    async fn set_cache(&self, p: &str, k: &str, v: &str, ttl: u64) -> Result<()> {
        let _ = self.cache.set_cache(p, k, v, ttl).await;
        self.persistence.set_cache(p, k, v, ttl).await
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

    async fn notify_config_changed(&self, p: &str, k: &str) -> Result<()> {
        self.persistence.notify_config_changed(p, k).await
    }
    async fn watch_config(&self, p: &str) -> Result<std::pin::Pin<Box<dyn tokio_stream::Stream<Item = String> + Send>>> {
        self.persistence.watch_config(p).await
    }
}
