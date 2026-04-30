use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use redis::aio::MultiplexedConnection;
use super::{Store, AuditEntry, DlqMessage, Item};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct RedisConfigValue {
    v: u64,
    d: String,
}

pub struct RedisStore {
    conn: MultiplexedConnection,
    url: String,
}

impl RedisStore {
    pub fn new(conn: MultiplexedConnection, url: &str) -> Self {
        Self { conn, url: url.to_string() }
    }

    async fn raw_get(&self, profile: &str, key: &str) -> Result<String> {
        let mut conn = self.conn.clone();
        let redis_key = format!("{}:{}", profile, key);
        let val: Option<String> = redis::cmd("GET").arg(&redis_key).query_async(&mut conn).await?;
        val.ok_or_else(|| anyhow::anyhow!("Key not found in Redis: {}", redis_key))
    }

    async fn raw_set(&self, profile: &str, key: &str, value: &str, ttl: Option<u64>) -> Result<()> {
        let mut conn = self.conn.clone();
        let redis_key = format!("{}:{}", profile, key);
        if let Some(secs) = ttl {
            redis::cmd("SETEX").arg(&redis_key).arg(secs).arg(value).query_async::<()>(&mut conn).await?;
        } else {
            redis::cmd("SET").arg(&redis_key).arg(value).query_async::<()>(&mut conn).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl Store for RedisStore {
    async fn get_config(&self, profile: &str, key: &str) -> Result<String> {
        let val = self.raw_get(profile, &format!("cfg:{}", key)).await?;
        if let Ok(wrapper) = serde_json::from_str::<RedisConfigValue>(&val) {
            return Ok(wrapper.d);
        }
        Ok(val)
    }

    async fn get_config_full(&self, profile: &str, key: &str) -> Result<Item> {
        let val = self.raw_get(profile, &format!("cfg:{}", key)).await?;
        if let Ok(wrapper) = serde_json::from_str::<RedisConfigValue>(&val) {
            return Ok(Item {
                profile: profile.to_string(),
                key: key.to_string(),
                value: wrapper.d,
                version: wrapper.v,
                updated_at: chrono::Utc::now().timestamp(),
            });
        }
        Ok(Item {
            profile: profile.to_string(),
            key: key.to_string(),
            value: val,
            version: 0,
            updated_at: chrono::Utc::now().timestamp(),
        })
    }

    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()> {
        let mut version = 0;
        if let Ok(old) = self.raw_get(profile, &format!("cfg:{}", key)).await {
            if let Ok(wrapper) = serde_json::from_str::<RedisConfigValue>(&old) {
                version = wrapper.v + 1;
            }
        }
        let wrapper = RedisConfigValue { v: version, d: value.to_string() };
        self.raw_set(profile, &format!("cfg:{}", key), &serde_json::to_string(&wrapper)?, None).await
    }

    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> Result<()> {
        let mut conn = self.conn.clone();
        let redis_key = format!("{}:cfg:{}", profile, key);
        
        // Lua script for atomic CAS
        let script = r#"
            local old = redis.call('GET', KEYS[1])
            if not old then return 0 end
            local data = cjson.decode(old)
            if data.v == tonumber(ARGV[1]) then
                data.v = data.v + 1
                data.d = ARGV[2]
                redis.call('SET', KEYS[1], cjson.encode(data))
                return 1
            else
                return 0
            end
        "#;
        
        let result: i32 = redis::Script::new(script)
            .key(redis_key)
            .arg(expected_version)
            .arg(value)
            .invoke_async(&mut conn).await?;
            
        if result == 0 {
            return Err(anyhow::anyhow!("Conflict: Redis config has been modified by another node"));
        }
        Ok(())
    }
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()> {
        let mut conn = self.conn.clone();
        let redis_key = format!("{}:cfg:{}", profile, key);
        redis::cmd("DEL").arg(&redis_key).query_async::<()>(&mut conn).await?;
        Ok(())
    }
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>> {
        let mut conn = self.conn.clone();
        let pattern = format!("{}:cfg:*", profile);
        let keys: Vec<String> = redis::cmd("KEYS").arg(&pattern).query_async(&mut conn).await?;
        let prefix_len = profile.len() + 5; // "{profile}:cfg:"
        Ok(keys.into_iter().map(|k| k[prefix_len..].to_string()).collect())
    }

    async fn get_secret(&self, profile: &str, key: &str) -> Result<String> { self.raw_get(profile, &format!("sec:{}", key)).await }
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()> { self.raw_set(profile, &format!("sec:{}", key), value, None).await }

    async fn get_token(&self, profile: &str, key: &str) -> Result<String> { self.raw_get(profile, &format!("tok:{}", key)).await }
    async fn set_token(&self, profile: &str, key: &str, value: &str, expires: u64) -> Result<()> { self.raw_set(profile, &format!("tok:{}", key), value, Some(expires)).await }
    async fn list_tokens(&self, profile: &str) -> Result<Vec<String>> {
        let mut conn = self.conn.clone();
        let pattern = format!("{}:tok:*", profile);
        let keys: Vec<String> = redis::cmd("KEYS").arg(&pattern).query_async(&mut conn).await?;
        let prefix_len = profile.len() + 5; // "{profile}:tok:"
        Ok(keys.into_iter().map(|k| k[prefix_len..].to_string()).collect())
    }

    async fn save_audit(&self, entry: &AuditEntry) -> Result<()> {
        let key = format!("aud:{}:{}", entry.timestamp.timestamp_millis(), entry.id);
        let json = serde_json::to_string(entry)?;
        self.raw_set(&entry.profile, &key, &json, None).await
    }
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<AuditEntry>> {
        let mut conn = self.conn.clone();
        let pattern = format!("{}:aud:*", profile);
        let keys: Vec<String> = redis::cmd("KEYS").arg(&pattern).query_async(&mut conn).await?;
        let mut sorted_keys = keys;
        sorted_keys.sort_by(|a, b| b.cmp(a));
        let mut entries = Vec::new();
        let prefix_len = profile.len() + 1;
        for k in sorted_keys.into_iter().take(limit) {
            if let Ok(json) = self.raw_get(profile, &k[prefix_len..]).await {
                if let Ok(e) = serde_json::from_str(&json) { entries.push(e); }
            }
        }
        Ok(entries)
    }

    async fn push_dlq(&self, msg: &DlqMessage) -> Result<()> {
        let mut conn = self.conn.clone();
        let key = format!("{}:dlq:{}", msg.profile, msg.topic);
        let json = serde_json::to_string(msg)?;
        redis::cmd("RPUSH").arg(&key).arg(json).query_async::<()>(&mut conn).await?;
        Ok(())
    }
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<DlqMessage>> {
        let mut conn = self.conn.clone();
        let key = format!("{}:dlq:{}", profile, topic);
        let val: Option<String> = redis::cmd("LPOP").arg(&key).query_async(&mut conn).await?;
        if let Some(json) = val { Ok(Some(serde_json::from_str(&json)?)) } else { Ok(None) }
    }
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<DlqMessage>> {
        let mut conn = self.conn.clone();
        let pattern = format!("{}:dlq:*", profile);
        let keys: Vec<String> = redis::cmd("KEYS").arg(&pattern).query_async(&mut conn).await?;
        let mut msgs = Vec::new();
        for k in keys {
            let list: Vec<String> = redis::cmd("LRANGE").arg(&k).arg(0).arg(limit as isize - 1).query_async(&mut conn).await?;
            for json in list {
                if let Ok(m) = serde_json::from_str(&json) { msgs.push(m); }
                if msgs.len() >= limit { break; }
            }
            if msgs.len() >= limit { break; }
        }
        Ok(msgs)
    }

    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<DlqMessage>> {
        let mut conn = self.conn.clone();
        let pattern = format!("{}:dlq:*", profile);
        let keys: Vec<String> = redis::cmd("KEYS").arg(&pattern).query_async(&mut conn).await?;
        let mut msgs = Vec::new();
        for k in keys {
            let list: Vec<String> = redis::cmd("LRANGE").arg(&k).arg(0).arg(-1).query_async(&mut conn).await?;
            for json in list {
                if let Ok(m) = serde_json::from_str(&json) { msgs.push(m); }
            }
        }
        Ok(msgs)
    }

    async fn get_cache(&self, profile: &str, key: &str) -> Result<String> { self.raw_get(profile, &format!("cch:{}", key)).await }
    async fn set_cache(&self, profile: &str, key: &str, value: &str, ttl: u64) -> Result<()> { self.raw_set(profile, &format!("cch:{}", key), value, Some(ttl)).await }

    async fn clear_profile(&self, profile: &str) -> Result<()> {
        let mut conn = self.conn.clone();
        let pattern = format!("{}:*", profile);
        let keys: Vec<String> = redis::cmd("KEYS").arg(&pattern).query_async(&mut conn).await?;
        for k in keys {
            redis::cmd("DEL").arg(&k).query_async::<()>(&mut conn).await?;
        }
        Ok(())
    }
    async fn rename_profile(&self, old: &str, new: &str) -> Result<()> {
        let mut conn = self.conn.clone();
        let keys: Vec<String> = redis::cmd("KEYS").arg(&format!("{}:*", old)).query_async(&mut conn).await?;
        for ok in keys {
            let nk = ok.replace(old, new);
            redis::cmd("RENAME").arg(&ok).arg(&nk).query_async::<()>(&mut conn).await?;
        }
        Ok(())
    }
    async fn list_all_profiles(&self) -> Result<Vec<String>> {
        let mut conn = self.conn.clone();
        let keys: Vec<String> = redis::cmd("KEYS").arg("*:cfg:system:manifest").query_async(&mut conn).await?;
        Ok(keys.into_iter().map(|k| k.split(':').next().unwrap().to_string()).collect())
    }

    async fn notify_config_changed(&self, profile: &str, key: &str) -> Result<()> {
        let mut conn = self.conn.clone();
        let channel = format!("cowen:config:changed:{}", profile);
        redis::cmd("PUBLISH").arg(&channel).arg(key).query_async::<()>(&mut conn).await?;
        Ok(())
    }

    async fn watch_config(&self, profile: &str) -> Result<std::pin::Pin<Box<dyn tokio_stream::Stream<Item = String> + Send>>> {
        let client = redis::Client::open(self.url.as_str())?;
        let mut pubsub = client.get_async_pubsub().await?;
        let channel = format!("cowen:config:changed:{}", profile);
        pubsub.subscribe(channel).await?;
        
        use tokio_stream::StreamExt;
        let stream = pubsub.into_on_message().map(|msg| {
            msg.get_payload::<String>().unwrap_or_default()
        });
        
        Ok(Box::pin(stream))
    }
}

pub struct RedisCacheBuilder;

#[async_trait]
impl super::CacheBuilder for RedisCacheBuilder {
    fn scheme(&self) -> &str {
        "redis"
    }

    async fn build(&self, url: &str, primary: Arc<dyn Store>) -> Result<Arc<dyn Store>> {
        let client = redis::Client::open(url)?;
        let conn = client.get_multiplexed_tokio_connection().await?;
        let redis_store: Arc<dyn Store> = Arc::new(RedisStore::new(conn, url));
        Ok(Arc::new(super::hybrid::HybridStore::new(redis_store, primary)))
    }
}

inventory::submit! { super::CacheBuilderRegistration { builder: &RedisCacheBuilder } }

pub struct RedisStoreBuilder;

#[async_trait]
impl super::StoreBuilder for RedisStoreBuilder {
    fn scheme(&self) -> &str {
        "redis"
    }

    async fn build(&self, url: &str, _app_dir: &std::path::Path, _fingerprint: &str) -> Result<Arc<dyn Store>> {
        let client = redis::Client::open(url)?;
        let conn = client.get_multiplexed_tokio_connection().await?;
        Ok(Arc::new(RedisStore::new(conn, url)))
    }
}

inventory::submit! { super::StoreBuilderRegistration { builder: &RedisStoreBuilder } }
