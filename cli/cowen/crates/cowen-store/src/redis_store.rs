#![cfg(feature = "redis")]
use cowen_common::{CowenResult, CowenError};
use async_trait::async_trait;

use crate::{Store, AuditEntry, DlqMessage, Item};
use cowen_common::models::{Token, Ticket};
use std::sync::Arc;
use redis::AsyncCommands;
use redis::aio::MultiplexedConnection;

pub struct RedisStore {
    conn: MultiplexedConnection,
    url: String,
}

impl RedisStore {
    pub fn new(conn: MultiplexedConnection, url: String) -> Self {
        Self { conn, url }
    }

    fn key(&self, profile: &str, key: &str) -> String {
        format!("{}:{}", profile, key)
    }

    async fn raw_get(&self, profile: &str, key: &str) -> CowenResult<String> {
        let redis_key = self.key(profile, key);
        let mut conn = self.conn.clone();
        let val: Option<String> = redis::cmd("GET").arg(&redis_key).query_async(&mut conn).await.map_err(CowenError::from)?;
        val.ok_or_else(|| CowenError::Store(format!("Key not found in Redis: {}", redis_key)))
    }

    async fn raw_set(&self, profile: &str, key: &str, value: &str, ttl: Option<u64>) -> CowenResult<()> {
        let redis_key = self.key(profile, key);
        let mut conn = self.conn.clone();
        if let Some(secs) = ttl {
            redis::cmd("SETEX").arg(&redis_key).arg(secs).arg(value).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        } else {
            redis::cmd("SET").arg(&redis_key).arg(value).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        }
        Ok(())
    }
}

#[async_trait]
impl Store for RedisStore {
    async fn get_config(&self, profile: &str, key: &str) -> CowenResult<String> {
        self.raw_get(profile, &format!("cfg:{}", key)).await
    }

    async fn get_config_metadata(&self, _profile: &str, _key: &str) -> CowenResult<(u64, i64)> {
        // Redis basic store doesn't support versioning yet
        Ok((1, 0))
    }

    async fn get_config_full(&self, profile: &str, key: &str) -> CowenResult<Item> {
        let val = self.get_config(profile, key).await?;
        Ok(Item {
            profile: profile.to_string(),
            key: key.to_string(),
            value: val,
            version: 1,
            updated_at: 0,
        })
    }

    async fn set_config(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> {
        self.raw_set(profile, &format!("cfg:{}", key), value, None).await?;
        // Update manifest
        let mut conn = self.conn.clone();
        let manifest_key = self.key(profile, "__keys__");
        redis::cmd("SADD").arg(&manifest_key).arg(key).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        Ok(())
    }

    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, _expected_version: u64) -> CowenResult<()> {
        // Redis basic store doesn't support CAS yet
        self.set_config(profile, key, value).await
    }

    async fn list_configs(&self, profile: &str) -> CowenResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let manifest_key = self.key(profile, "__keys__");
        let keys: Vec<String> = redis::cmd("SMEMBERS").arg(&manifest_key).query_async(&mut conn).await.map_err(CowenError::from)?;
        Ok(keys)
    }

    async fn delete_config(&self, profile: &str, key: &str) -> CowenResult<()> {
        let redis_key = self.key(profile, &format!("cfg:{}", key));
        let mut conn = self.conn.clone();
        redis::cmd("DEL").arg(&redis_key).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        let manifest_key = self.key(profile, "__keys__");
        redis::cmd("SREM").arg(&manifest_key).arg(key).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        Ok(())
    }

    async fn get_secret(&self, profile: &str, key: &str) -> CowenResult<String> {
        self.raw_get(profile, &format!("sec:{}", key)).await
    }

    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> CowenResult<()> {
        self.raw_set(profile, &format!("sec:{}", key), value, None).await
    }

    async fn delete_secret(&self, profile: &str, key: &str) -> CowenResult<()> {
        let redis_key = self.key(profile, &format!("sec:{}", key));
        let mut conn = self.conn.clone();
        redis::cmd("DEL").arg(&redis_key).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        Ok(())
    }

    async fn list_secrets(&self, profile: &str) -> CowenResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let pattern = format!("{}:sec:*", profile);
        let keys: Vec<String> = redis::cmd("KEYS").arg(&pattern).query_async(&mut conn).await.map_err(CowenError::from)?;
        let prefix_len = profile.len() + 5; // "{profile}:sec:"
        Ok(keys.into_iter().map(|k| k[prefix_len..].to_string()).collect())
    }

    async fn get_access_token(&self, profile: &str) -> CowenResult<Token> {
        let json = self.raw_get(profile, "tok:access").await?;
        Ok(serde_json::from_str(&json)?)
    }

    async fn save_access_token(&self, profile: &str, token: Token) -> CowenResult<()> {
        let json = serde_json::to_string(&token)?;
        self.raw_set(profile, "tok:access", &json, None).await
    }

    async fn delete_access_token(&self, profile: &str) -> CowenResult<()> {
        let redis_key = self.key(profile, "tok:access");
        let mut conn = self.conn.clone();
        redis::cmd("DEL").arg(&redis_key).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        Ok(())
    }

    async fn get_app_access_token(&self, app_key: &str) -> CowenResult<Token> {
        let json = self.raw_get(&format!("app:{}", app_key), "tok_v2:app_access").await?;
        Ok(serde_json::from_str(&json)?)
    }

    async fn save_app_access_token(&self, app_key: &str, token: Token) -> CowenResult<()> {
        let json = serde_json::to_string(&token)?;
        self.raw_set(&format!("app:{}", app_key), "tok_v2:app_access", &json, None).await
    }

    async fn get_app_ticket(&self, app_key: &str) -> CowenResult<Ticket> {
        let json = self.raw_get(&format!("app:{}", app_key), "tic:v1").await?;
        Ok(serde_json::from_str(&json)?)
    }

    async fn save_app_ticket(&self, app_key: &str, ticket: Ticket) -> CowenResult<()> {
        let json = serde_json::to_string(&ticket)?;
        self.raw_set(&format!("app:{}", app_key), "tic:v1", &json, None).await
    }

    async fn delete_app_ticket(&self, app_key: &str) -> CowenResult<()> {
        let redis_key = self.key(&format!("app:{}", app_key), "tic:v1");
        let mut conn = self.conn.clone();
        redis::cmd("DEL").arg(&redis_key).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        Ok(())
    }

    async fn get_org_permanent_code(&self, app_key: &str, org_id: &str) -> CowenResult<String> {
        self.raw_get(&format!("app:{}", app_key), &format!("opc:{}", org_id)).await
    }

    async fn save_org_permanent_code(&self, app_key: &str, org_id: &str, code: &str) -> CowenResult<()> {
        self.raw_set(&format!("app:{}", app_key), &format!("opc:{}", org_id), code, None).await
    }

    async fn get_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str) -> CowenResult<String> {
        self.raw_get(&format!("app:{}", app_key), &format!("upc:{}:{}", org_id, user_id)).await
    }

    async fn save_user_permanent_code(&self, app_key: &str, org_id: &str, user_id: &str, code: &str) -> CowenResult<()> {
        self.raw_set(&format!("app:{}", app_key), &format!("upc:{}:{}", org_id, user_id), code, None).await
    }

    async fn get_token(&self, profile: &str, key: &str) -> CowenResult<String> {
        self.raw_get(profile, &format!("tok_legacy:{}", key)).await
    }

    async fn set_token(&self, profile: &str, key: &str, value: &str, exp: u64) -> CowenResult<()> {
        self.raw_set(profile, &format!("tok_legacy:{}", key), value, Some(exp)).await
    }

    async fn delete_token(&self, profile: &str, key: &str) -> CowenResult<()> {
        let redis_key = self.key(profile, &format!("tok_legacy:{}", key));
        let mut conn = self.conn.clone();
        redis::cmd("DEL").arg(&redis_key).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        Ok(())
    }

    async fn list_tokens(&self, profile: &str) -> CowenResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let pattern = format!("{}:tok_legacy:*", profile);
        let keys: Vec<String> = redis::cmd("KEYS").arg(&pattern).query_async(&mut conn).await.map_err(CowenError::from)?;
        let prefix_len = profile.len() + 12; // "{profile}:tok_legacy:"
        Ok(keys.into_iter().map(|k| k[prefix_len..].to_string()).collect())
    }

    async fn save_audit(&self, entry: &AuditEntry) -> CowenResult<()> {
        let key = self.key(&entry.profile, "audit:log");
        let json = serde_json::to_string(entry)?;
        let mut conn = self.conn.clone();
        redis::cmd("LPUSH").arg(&key).arg(json).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        redis::cmd("LTRIM").arg(&key).arg(0).arg(9999).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        Ok(())
    }

    async fn list_audit(&self, profile: &str, limit: usize) -> CowenResult<Vec<AuditEntry>> {
        let key = self.key(profile, "audit:log");
        let mut conn = self.conn.clone();
        let list: Vec<String> = redis::cmd("LRANGE").arg(&key).arg(0).arg(limit as isize - 1).query_async(&mut conn).await.map_err(CowenError::from)?;
        let mut entries = Vec::new();
        for json in list {
            if let Ok(e) = serde_json::from_str::<AuditEntry>(&json) { entries.push(e); }
        }
        Ok(entries)
    }

    async fn push_dlq(&self, msg: &DlqMessage) -> CowenResult<()> {
        let key = self.key(&msg.profile, &format!("dlq:{}", msg.topic));
        let json = serde_json::to_string(msg)?;
        let mut conn = self.conn.clone();
        redis::cmd("RPUSH").arg(&key).arg(json).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        Ok(())
    }

    async fn pop_dlq(&self, profile: &str, topic: &str) -> CowenResult<Option<DlqMessage>> {
        let key = self.key(profile, &format!("dlq:{}", topic));
        let mut conn = self.conn.clone();
        let val: Option<String> = redis::cmd("LPOP").arg(&key).query_async(&mut conn).await.map_err(CowenError::from)?;
        if let Some(json) = val { Ok(Some(serde_json::from_str(&json)?)) } else { Ok(None) }
    }

    async fn list_dlq(&self, profile: &str, limit: usize) -> CowenResult<Vec<DlqMessage>> {
        let mut conn = self.conn.clone();
        let pattern = format!("{}:dlq:*", profile);
        let keys: Vec<String> = redis::cmd("KEYS").arg(&pattern).query_async(&mut conn).await.map_err(CowenError::from)?;
        let mut msgs = Vec::new();
        for k in keys {
            let list: Vec<String> = redis::cmd("LRANGE").arg(&k).arg(0).arg(limit as isize - 1).query_async(&mut conn).await.map_err(CowenError::from)?;
            for json in list {
                if let Ok(m) = serde_json::from_str::<DlqMessage>(&json) { msgs.push(m); }
                if msgs.len() >= limit { break; }
            }
            if msgs.len() >= limit { break; }
        }
        Ok(msgs)
    }

    async fn list_all_dlq(&self, profile: &str) -> CowenResult<Vec<DlqMessage>> {
        let mut conn = self.conn.clone();
        let pattern = format!("{}:dlq:*", profile);
        let keys: Vec<String> = redis::cmd("KEYS").arg(&pattern).query_async(&mut conn).await.map_err(CowenError::from)?;
        let mut msgs = Vec::new();
        for k in keys {
            let list: Vec<String> = redis::cmd("LRANGE").arg(&k).arg(0).arg(-1).query_async(&mut conn).await.map_err(CowenError::from)?;
            for json in list {
                if let Ok(m) = serde_json::from_str::<DlqMessage>(&json) { msgs.push(m); }
            }
        }
        Ok(msgs)
    }

    async fn clear_profile(&self, profile: &str) -> CowenResult<()> {
        let mut conn = self.conn.clone();
        let pattern = format!("{}:*", profile);
        let keys: Vec<String> = redis::cmd("KEYS").arg(&pattern).query_async(&mut conn).await.map_err(CowenError::from)?;
        for k in keys {
            redis::cmd("DEL").arg(&k).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        }
        Ok(())
    }

    async fn rename_profile(&self, old: &str, new: &str) -> CowenResult<()> {
        let mut conn = self.conn.clone();
        let keys: Vec<String> = redis::cmd("KEYS").arg(&format!("{}:*", old)).query_async(&mut conn).await.map_err(CowenError::from)?;
        for ok in keys {
            let nk = ok.replace(old, new);
            redis::cmd("RENAME").arg(&ok).arg(&nk).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        }
        Ok(())
    }

    async fn list_all_profiles(&self) -> CowenResult<Vec<String>> {
        let mut conn = self.conn.clone();
        let keys: Vec<String> = redis::cmd("KEYS").arg("*:cfg:system:manifest").query_async(&mut conn).await.map_err(CowenError::from)?;
        Ok(keys.into_iter().map(|k| k.split(':').next().unwrap().to_string()).collect())
    }

    async fn raw_del(&self, key: &str) -> CowenResult<()> {
        let mut conn = self.conn.clone();
        redis::cmd("DEL").arg(key).query_async::<()>(&mut conn).await.map_err(CowenError::from)?;
        Ok(())
    }

    fn name(&self) -> &str {
        "Redis"
    }

    fn description(&self) -> String {
        format!("Redis Server: {}", cowen_common::utils::mask_url(&self.url))
    }
}

pub struct RedisStoreBuilder;

#[async_trait]
impl crate::StoreBuilder for RedisStoreBuilder {
    fn scheme(&self) -> &str { "redis" }
    async fn build(&self, url: &str, _app_dir: &std::path::Path, fingerprint: &str) -> CowenResult<Arc<dyn Store>> {
        let client = redis::Client::open(url).map_err(CowenError::from)?;
        let conn = client.get_multiplexed_tokio_connection().await.map_err(CowenError::from)?;
        Ok(Arc::new(RedisStore::new(conn, url.to_string())))
    }
}

inventory::submit! { crate::StoreBuilderRegistration { builder: &RedisStoreBuilder } }
