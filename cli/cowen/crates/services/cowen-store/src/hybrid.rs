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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::FileStore;

    #[tokio::test]
    async fn test_hybrid_store_comprehensive() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        
        let p_store = Arc::new(FileStore::new(dir1.path().to_path_buf(), None).unwrap());
        let c_store = Arc::new(FileStore::new(dir2.path().to_path_buf(), None).unwrap());
        
        let hybrid = HybridStore::new(p_store.clone(), c_store.clone());
        
        // 1. Config tests
        hybrid.set_config("p", "k", "v").await.unwrap();
        assert_eq!(hybrid.get_config("p", "k").await.unwrap(), "v");
        let v_meta = hybrid.get_config_metadata("p", "k").await.unwrap().0;
        assert_eq!(hybrid.get_config_full("p", "k").await.unwrap().value, "v");
        
        hybrid.set_config_conditional("p", "k", "v2", v_meta).await.unwrap();
        assert_eq!(hybrid.get_config("p", "k").await.unwrap(), "v2");
        
        assert_eq!(hybrid.list_configs("p").await.unwrap(), vec!["k".to_string()]);
        
        hybrid.delete_config("p", "k").await.unwrap();
        assert!(hybrid.get_config("p", "k").await.is_err());

        // 2. Secret tests
        hybrid.set_secret("p", "sk", "sv").await.unwrap();
        assert_eq!(hybrid.get_secret("p", "sk").await.unwrap(), "sv");
        assert_eq!(hybrid.list_secrets("p").await.unwrap(), vec!["sk".to_string()]);
        hybrid.delete_secret("p", "sk").await.unwrap();
        assert!(hybrid.get_secret("p", "sk").await.is_err());

        // 3. Tokens & Tickets & Codes
        let tok = Token {
            value: "t1".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            created_at: chrono::Utc::now(),
        };
        hybrid.save_access_token("p", tok.clone()).await.unwrap();
        assert_eq!(hybrid.get_access_token("p").await.unwrap().value, "t1");
        hybrid.delete_access_token("p").await.unwrap();

        hybrid.save_refresh_token("p", tok.clone()).await.unwrap();
        assert_eq!(hybrid.get_refresh_token("p").await.unwrap().value, "t1");
        hybrid.delete_refresh_token("p").await.unwrap();

        hybrid.save_app_access_token("ak", tok.clone()).await.unwrap();
        assert_eq!(hybrid.get_app_access_token("ak").await.unwrap().value, "t1");
        hybrid.delete_app_access_token("ak").await.unwrap();

        let ticket = Ticket {
            value: "tick1".to_string(),
            created_at: chrono::Utc::now(),
        };
        hybrid.save_app_ticket("ak", ticket.clone()).await.unwrap();
        assert_eq!(hybrid.get_app_ticket("ak").await.unwrap().value, "tick1");
        hybrid.delete_app_ticket("ak").await.unwrap();

        hybrid.save_org_permanent_code("ak", "org", "c1").await.unwrap();
        assert_eq!(hybrid.get_org_permanent_code("ak", "org").await.unwrap(), "c1");

        hybrid.save_user_permanent_code("ak", "org", "usr", "c2").await.unwrap();
        assert_eq!(hybrid.get_user_permanent_code("ak", "org", "usr").await.unwrap(), "c2");

        hybrid.set_token("p", "tk", "tv", 3600).await.unwrap();
        assert_eq!(hybrid.get_token("p", "tk").await.unwrap(), "tv");
        assert_eq!(hybrid.list_tokens("p").await.unwrap(), vec!["tk".to_string()]);
        hybrid.delete_token("p", "tk").await.unwrap();

        // 4. Audit & DLQ
        let audit = AuditEntry {
            id: "1".to_string(),
            timestamp: chrono::Utc::now(),
            profile: "p".to_string(),
            level: "info".to_string(),
            target: "t".to_string(),
            message: "msg".to_string(),
            fields: serde_json::Value::Null,
        };
        hybrid.save_audit(&audit).await.unwrap();
        assert_eq!(hybrid.list_audit("p", 10).await.unwrap().len(), 1);

        let dlq = DlqMessage {
            id: Some(1),
            profile: "p".to_string(),
            topic: "t".to_string(),
            payload: "pay".to_string(),
            retry_count: 0,
            error: Some("err".to_string()),
            created_at: chrono::Utc::now(),
        };
        hybrid.push_dlq(&dlq).await.unwrap();
        assert_eq!(hybrid.list_all_dlq("p").await.unwrap().len(), 1);
        assert_eq!(hybrid.list_dlq("p", 10).await.unwrap().len(), 1);
        assert_eq!(hybrid.list_dlq_paged("p", 0, 10).await.unwrap().len(), 1);
        assert!(hybrid.get_dlq_by_id(1).await.unwrap().is_some());
        assert!(hybrid.pop_dlq("p", "t").await.unwrap().is_some());
        hybrid.delete_dlq_by_id(1).await.unwrap();

        // 5. Profile & Management
        hybrid.set_config("p", "k", "v").await.unwrap();
        hybrid.rename_profile("p", "new_p").await.unwrap();
        let profiles = hybrid.list_all_profiles().await.unwrap();
        assert!(profiles.contains(&"new_p".to_string()));
        assert!(profiles.contains(&"ak".to_string()));
        hybrid.clear_profile("new_p").await.unwrap();
        hybrid.raw_del("new_p/config/k").await.unwrap();
        hybrid.migrate().await.unwrap();

        assert_eq!(hybrid.name(), "Hybrid");
        assert!(hybrid.description().contains("Hybrid"));
    }
}
