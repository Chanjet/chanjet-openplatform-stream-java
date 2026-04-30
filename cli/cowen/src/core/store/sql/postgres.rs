use anyhow::Result;
use async_trait::async_trait;
use sqlx::{Postgres, Pool};
use std::sync::Arc;
use tokio_stream::StreamExt;
use sqlx::postgres::PgListener;
use super::{SqlDriver, SqlBuilder};

pub struct PostgresDriver {
    pool: Pool<Postgres>,
    url: String,
}

impl PostgresDriver {
    pub fn new(pool: Pool<Postgres>, url: &str) -> Self {
        Self { pool, url: url.to_string() }
    }
}

#[async_trait]
impl SqlDriver for PostgresDriver {
    // --- Config Domain ---
    async fn get_config(&self, profile: &str, key: &str) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_config WHERE profile = $1 AND item_key = $2")
            .bind(profile).bind(key).fetch_one(&self.pool).await?;
        Ok(row.0)
    }
    async fn get_config_full(&self, profile: &str, key: &str) -> Result<super::super::Item> {
        let row: (String, String, String, i64, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
            "SELECT profile, item_key, item_value, version, updated_at FROM cowen_config WHERE profile = $1 AND item_key = $2"
        ).bind(profile).bind(key).fetch_one(&self.pool).await?;
        Ok(super::super::Item {
            profile: row.0,
            key: row.1,
            value: row.2,
            version: row.3 as u64,
            updated_at: row.4.timestamp(),
        })
    }
    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()> {
        sqlx::query("INSERT INTO cowen_config (profile, item_key, item_value, version) VALUES ($1, $2, $3, 0) ON CONFLICT (profile, item_key) DO UPDATE SET item_value = EXCLUDED.item_value, version = cowen_config.version + 1")
            .bind(profile).bind(key).bind(value).execute(&self.pool).await?;
        Ok(())
    }
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> Result<()> {
        let res = sqlx::query("UPDATE cowen_config SET item_value = $1, version = version + 1 WHERE profile = $2 AND item_key = $3 AND version = $4")
            .bind(value).bind(profile).bind(key).bind(expected_version as i64).execute(&self.pool).await?;
        if res.rows_affected() == 0 {
            return Err(anyhow::anyhow!("Conflict: Config has been modified by another node (expected version {}, but found different)", expected_version));
        }
        Ok(())
    }
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>("SELECT item_key FROM cowen_config WHERE profile = $1")
            .bind(profile).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM cowen_config WHERE profile = $1 AND item_key = $2")
            .bind(profile).bind(key).execute(&self.pool).await?;
        Ok(())
    }

    // --- Secret Domain ---
    async fn get_secret(&self, profile: &str, key: &str) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_secret WHERE profile = $1 AND item_key = $2")
            .bind(profile).bind(key).fetch_one(&self.pool).await?;
        Ok(row.0)
    }
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()> {
        sqlx::query("INSERT INTO cowen_secret (profile, item_key, item_value) VALUES ($1, $2, $3) ON CONFLICT (profile, item_key) DO UPDATE SET item_value = EXCLUDED.item_value")
            .bind(profile).bind(key).bind(value).execute(&self.pool).await?;
        Ok(())
    }

    // --- Token Domain ---
    async fn get_token(&self, profile: &str, key: &str) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_token WHERE profile = $1 AND item_key = $2 AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)")
            .bind(profile).bind(key).fetch_one(&self.pool).await?;
        Ok(row.0)
    }
    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> Result<()> {
        sqlx::query("INSERT INTO cowen_token (profile, item_key, item_value, expires_at) VALUES ($1, $2, $3, CURRENT_TIMESTAMP + INTERVAL '1 second' * $4) ON CONFLICT (profile, item_key) DO UPDATE SET item_value = EXCLUDED.item_value, expires_at = EXCLUDED.expires_at")
            .bind(profile).bind(key).bind(value).bind(expires_in_secs as i64).execute(&self.pool).await?;
        Ok(())
    }
    async fn list_tokens(&self, profile: &str) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>("SELECT item_key FROM cowen_token WHERE profile = $1 AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)")
            .bind(profile).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    // --- Audit Domain ---
    async fn save_audit(&self, entry: &super::super::AuditEntry) -> Result<()> {
        sqlx::query("INSERT INTO cowen_audit (id, profile, timestamp, level, target, message, fields) VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(&entry.id).bind(&entry.profile).bind(entry.timestamp).bind(&entry.level).bind(&entry.target).bind(&entry.message).bind(&entry.fields).execute(&self.pool).await?;
        Ok(())
    }
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<super::super::AuditEntry>> {
        let rows = sqlx::query_as::<_, (String, chrono::DateTime<chrono::Utc>, String, String, String, String, serde_json::Value)>(
            "SELECT id, timestamp, profile, level, target, message, fields FROM cowen_audit WHERE profile = $1 ORDER BY timestamp DESC LIMIT $2"
        ).bind(profile).bind(limit as i64).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| super::super::AuditEntry {
            id: r.0, timestamp: r.1, profile: r.2, level: r.3, target: r.4, message: r.5, fields: r.6
        }).collect())
    }

    // --- DLQ Domain ---
    async fn push_dlq(&self, msg: &super::super::DlqMessage) -> Result<()> {
        sqlx::query("INSERT INTO cowen_dlq (profile, topic, payload, retry_count, error) VALUES ($1, $2, $3, $4, $5)")
            .bind(&msg.profile).bind(&msg.topic).bind(&msg.payload).bind(msg.retry_count).bind(&msg.error).execute(&self.pool).await?;
        Ok(())
    }
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<super::super::DlqMessage>> {
        let row = sqlx::query_as::<_, (i64, String, String, String, i32, Option<String>, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = $1 AND topic = $2 ORDER BY id ASC LIMIT 1"
        ).bind(profile).bind(topic).fetch_optional(&self.pool).await?;
        if let Some(r) = row {
            sqlx::query("DELETE FROM cowen_dlq WHERE id = $1").bind(r.0).execute(&self.pool).await?;
            Ok(Some(super::super::DlqMessage { id: Some(r.0), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6 }))
        } else { Ok(None) }
    }
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<super::super::DlqMessage>> {
        let rows = sqlx::query_as::<_, (i64, String, String, String, i32, Option<String>, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = $1 ORDER BY id DESC LIMIT $2"
        ).bind(profile).bind(limit as i64).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| super::super::DlqMessage {
            id: Some(r.0), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }).collect())
    }
    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<super::super::DlqMessage>> {
        let rows = sqlx::query_as::<_, (i64, String, String, String, i32, Option<String>, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = $1"
        ).bind(profile).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| super::super::DlqMessage {
            id: Some(r.0), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }).collect())
    }

    // --- Cache Domain ---
    async fn get_cache(&self, profile: &str, key: &str) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_cache WHERE profile = $1 AND item_key = $2 AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)")
            .bind(profile).bind(key).fetch_one(&self.pool).await?;
        Ok(row.0)
    }
    async fn set_cache(&self, profile: &str, key: &str, value: &str, ttl_secs: u64) -> Result<()> {
        sqlx::query("INSERT INTO cowen_cache (profile, item_key, item_value, expires_at) VALUES ($1, $2, $3, CURRENT_TIMESTAMP + INTERVAL '1 second' * $4) ON CONFLICT (profile, item_key) DO UPDATE SET item_value = EXCLUDED.item_value, expires_at = EXCLUDED.expires_at")
            .bind(profile).bind(key).bind(value).bind(ttl_secs as i64).execute(&self.pool).await?;
        Ok(())
    }

    // --- Management ---
    async fn clear_profile(&self, profile: &str) -> Result<()> {
        for table in ["cowen_config", "cowen_secret", "cowen_token", "cowen_audit", "cowen_dlq", "cowen_cache"] {
            let sql = format!("DELETE FROM {} WHERE profile = $1", table);
            sqlx::query(&sql).bind(profile).execute(&self.pool).await?;
        }
        Ok(())
    }
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()> {
        for table in ["cowen_config", "cowen_secret", "cowen_token", "cowen_audit", "cowen_dlq", "cowen_cache"] {
            let sql = format!("UPDATE {} SET profile = $1 WHERE profile = $2", table);
            sqlx::query(&sql).bind(new_name).bind(old_name).execute(&self.pool).await?;
        }
        Ok(())
    }
    async fn list_all_profiles(&self) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>("SELECT DISTINCT profile FROM cowen_config")
            .fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn notify_config_changed(&self, profile: &str, key: &str) -> Result<()> {
        let channel = "cowen_config_changed";
        let payload = format!("{}:{}", profile, key);
        sqlx::query(&format!("NOTIFY {}, '{}'", channel, payload))
            .execute(&self.pool).await?;
        Ok(())
    }

    async fn watch_config(&self, profile: &str) -> Result<std::pin::Pin<Box<dyn tokio_stream::Stream<Item = String> + Send>>> {
        let mut listener = PgListener::connect(&self.url).await?;
        listener.listen("cowen_config_changed").await?;
        
        let p = profile.to_string();
        let stream = listener.into_stream().filter_map(move |res| {
            if let Ok(notification) = res {
                let payload = notification.payload();
                if let Some(pos) = payload.find(':') {
                    if &payload[..pos] == p {
                        return Some(payload[pos+1..].to_string());
                    }
                }
            }
            None
        });
        
        Ok(Box::pin(stream))
    }
}

pub struct PostgresBuilder;
#[async_trait]
impl SqlBuilder for PostgresBuilder {
    fn scheme(&self) -> &str { "postgres" }
    async fn build(&self, url: &str) -> Result<Arc<dyn SqlDriver>> {
        let pool = sqlx::PgPool::connect(url).await?;
        
        let ddl = [
            "CREATE TABLE IF NOT EXISTS cowen_config (profile VARCHAR(255) NOT NULL, item_key VARCHAR(255) NOT NULL, item_value TEXT NOT NULL, version INTEGER DEFAULT 0, updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (profile, item_key))",
            "CREATE TABLE IF NOT EXISTS cowen_secret (profile VARCHAR(255) NOT NULL, item_key VARCHAR(255) NOT NULL, item_value TEXT NOT NULL, updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (profile, item_key))",
            "CREATE TABLE IF NOT EXISTS cowen_token (profile VARCHAR(255) NOT NULL, item_key VARCHAR(255) NOT NULL, item_value TEXT NOT NULL, expires_at TIMESTAMP WITH TIME ZONE NULL, PRIMARY KEY (profile, item_key))",
            "CREATE TABLE IF NOT EXISTS cowen_audit (id VARCHAR(36) PRIMARY KEY, profile VARCHAR(255) NOT NULL, timestamp TIMESTAMP WITH TIME ZONE NOT NULL, level VARCHAR(20) NOT NULL, target VARCHAR(255) NOT NULL, message TEXT NOT NULL, fields JSONB)",
            "CREATE TABLE IF NOT EXISTS cowen_dlq (id BIGSERIAL PRIMARY KEY, profile VARCHAR(255) NOT NULL, topic VARCHAR(255) NOT NULL, payload TEXT NOT NULL, retry_count INT DEFAULT 0, error TEXT, created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE IF NOT EXISTS cowen_cache (profile VARCHAR(255) NOT NULL, item_key VARCHAR(255) NOT NULL, item_value TEXT NOT NULL, expires_at TIMESTAMP WITH TIME ZONE NULL, PRIMARY KEY (profile, item_key))",
        ];

        for sql in ddl { sqlx::query(sql).execute(&pool).await?; }
        
        // Indices
        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_profile_ts ON cowen_audit (profile, timestamp)").execute(&pool).await;
        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_dlq_profile_topic ON cowen_dlq (profile, topic)").execute(&pool).await;

        Ok(Arc::new(PostgresDriver::new(pool, url)))
    }
}

inventory::submit! { super::SqlBuilderRegistration { builder: &PostgresBuilder } }
