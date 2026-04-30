use anyhow::Result;
use async_trait::async_trait;
use sqlx::{Mssql, Pool};
use std::sync::Arc;
use super::{SqlDriver, SqlBuilder};

pub struct MssqlDriver {
    pool: Pool<Mssql>,
}

impl MssqlDriver {
    pub fn new(pool: Pool<Mssql>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SqlDriver for MssqlDriver {
    // --- Config Domain ---
    async fn get_config(&self, profile: &str, key: &str) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_config WHERE profile = @p1 AND item_key = @p2")
            .bind(profile).bind(key).fetch_one(&self.pool).await?;
        Ok(row.0)
    }
    async fn get_config_full(&self, profile: &str, key: &str) -> Result<super::super::Item> {
        let row: (String, String, String, i32, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
            "SELECT profile, item_key, item_value, version, updated_at FROM cowen_config WHERE profile = @p1 AND item_key = @p2"
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
        sqlx::query("MERGE cowen_config AS target USING (SELECT @p1, @p2, @p3) AS source (p, k, v) ON (target.profile = source.p AND target.item_key = source.k) WHEN MATCHED THEN UPDATE SET item_value = source.v, version = version + 1 WHEN NOT MATCHED THEN INSERT (profile, item_key, item_value, version) VALUES (source.p, source.k, source.v, 0);")
            .bind(profile).bind(key).bind(value).execute(&self.pool).await?;
        Ok(())
    }
    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> Result<()> {
        let res = sqlx::query("UPDATE cowen_config SET item_value = @p1, version = version + 1 WHERE profile = @p2 AND item_key = @p3 AND version = @p4")
            .bind(value).bind(profile).bind(key).bind(expected_version as i32).execute(&self.pool).await?;
        if res.rows_affected() == 0 {
            return Err(anyhow::anyhow!("Conflict: MSSQL config has been modified by another node"));
        }
        Ok(())
    }
    async fn list_configs(&self, profile: &str) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>("SELECT item_key FROM cowen_config WHERE profile = @p1")
            .bind(profile).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }
    async fn delete_config(&self, profile: &str, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM cowen_config WHERE profile = @p1 AND item_key = @p2")
            .bind(profile).bind(key).execute(&self.pool).await?;
        Ok(())
    }

    // --- Secret Domain ---
    async fn get_secret(&self, profile: &str, key: &str) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_secret WHERE profile = @p1 AND item_key = @p2")
            .bind(profile).bind(key).fetch_one(&self.pool).await?;
        Ok(row.0)
    }
    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()> {
        sqlx::query("MERGE cowen_secret AS target USING (SELECT @p1, @p2, @p3) AS source (p, k, v) ON (target.profile = source.p AND target.item_key = source.k) WHEN MATCHED THEN UPDATE SET item_value = source.v WHEN NOT MATCHED THEN INSERT (profile, item_key, item_value) VALUES (source.p, source.k, source.v);")
            .bind(profile).bind(key).bind(value).execute(&self.pool).await?;
        Ok(())
    }

    // --- Token Domain ---
    async fn get_token(&self, profile: &str, key: &str) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_token WHERE profile = @p1 AND item_key = @p2 AND (expires_at IS NULL OR expires_at > GETDATE())")
            .bind(profile).bind(key).fetch_one(&self.pool).await?;
        Ok(row.0)
    }
    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> Result<()> {
        sqlx::query("MERGE cowen_token AS target USING (SELECT @p1, @p2, @p3, @p4) AS source (p, k, v, e) ON (target.profile = source.p AND target.item_key = source.k) WHEN MATCHED THEN UPDATE SET item_value = source.v, expires_at = DATEADD(second, source.e, GETDATE()) WHEN NOT MATCHED THEN INSERT (profile, item_key, item_value, expires_at) VALUES (source.p, source.k, source.v, DATEADD(second, source.e, GETDATE()));")
            .bind(profile).bind(key).bind(value).bind(expires_in_secs as i32).execute(&self.pool).await?;
        Ok(())
    }
    async fn list_tokens(&self, profile: &str) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>("SELECT item_key FROM cowen_token WHERE profile = @p1 AND (expires_at IS NULL OR expires_at > GETDATE())")
            .bind(profile).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }
    async fn list_tokens(&self, profile: &str) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>("SELECT item_key FROM cowen_token WHERE profile = @p1 AND (expires_at IS NULL OR expires_at > GETDATE())")
            .bind(profile).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    // --- Audit Domain ---
    async fn save_audit(&self, entry: &super::super::AuditEntry) -> Result<()> {
        sqlx::query("INSERT INTO cowen_audit (id, profile, timestamp, level, target, message, fields) VALUES (@p1, @p2, @p3, @p4, @p5, @p6, @p7)")
            .bind(&entry.id).bind(&entry.profile).bind(entry.timestamp).bind(&entry.level).bind(&entry.target).bind(&entry.message).bind(serde_json::to_string(&entry.fields)?).execute(&self.pool).await?;
        Ok(())
    }
    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<super::super::AuditEntry>> {
        let rows = sqlx::query_as::<_, (String, chrono::DateTime<chrono::Utc>, String, String, String, String, String)>(
            &format!("SELECT TOP {} id, timestamp, profile, level, target, message, fields FROM cowen_audit WHERE profile = @p1 ORDER BY timestamp DESC", limit)
        ).bind(profile).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| super::super::AuditEntry {
            id: r.0, timestamp: r.1, profile: r.2, level: r.3, target: r.4, message: r.5, fields: serde_json::from_str(&r.6).unwrap_or_default()
        }).collect())
    }

    // --- DLQ Domain ---
    async fn push_dlq(&self, msg: &super::super::DlqMessage) -> Result<()> {
        sqlx::query("INSERT INTO cowen_dlq (profile, topic, payload, retry_count, error) VALUES (@p1, @p2, @p3, @p4, @p5)")
            .bind(&msg.profile).bind(&msg.topic).bind(&msg.payload).bind(msg.retry_count).bind(&msg.error).execute(&self.pool).await?;
        Ok(())
    }
    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<super::super::DlqMessage>> {
        let row = sqlx::query_as::<_, (i64, String, String, String, i32, Option<String>, chrono::DateTime<chrono::Utc>)>(
            "SELECT TOP 1 id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p1 AND topic = @p2 ORDER BY id ASC"
        ).bind(profile).bind(topic).fetch_optional(&self.pool).await?;
        if let Some(r) = row {
            sqlx::query("DELETE FROM cowen_dlq WHERE id = @p1").bind(r.0).execute(&self.pool).await?;
            Ok(Some(super::super::DlqMessage { id: Some(r.0), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6 }))
        } else { Ok(None) }
    }
    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<super::super::DlqMessage>> {
        let rows = sqlx::query_as::<_, (i64, String, String, String, i32, Option<String>, chrono::DateTime<chrono::Utc>)>(
            &format!("SELECT TOP {} id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p1 ORDER BY id DESC", limit)
        ).bind(profile).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| super::super::DlqMessage {
            id: Some(r.0), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }).collect())
    }
    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<super::super::DlqMessage>> {
        let rows = sqlx::query_as::<_, (i64, String, String, String, i32, Option<String>, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p1"
        ).bind(profile).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| super::super::DlqMessage {
            id: Some(r.0), profile: r.1, topic: r.2, payload: r.3, retry_count: r.4, error: r.5, created_at: r.6
        }).collect())
    }

    // --- Cache Domain ---
    async fn get_cache(&self, profile: &str, key: &str) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT item_value FROM cowen_cache WHERE profile = @p1 AND item_key = @p2 AND (expires_at IS NULL OR expires_at > GETDATE())")
            .bind(profile).bind(key).fetch_one(&self.pool).await?;
        Ok(row.0)
    }
    async fn set_cache(&self, profile: &str, key: &str, value: &str, ttl_secs: u64) -> Result<()> {
        sqlx::query("MERGE cowen_cache AS target USING (SELECT @p1, @p2, @p3, @p4) AS source (p, k, v, e) ON (target.profile = source.p AND target.item_key = source.k) WHEN MATCHED THEN UPDATE SET item_value = source.v, expires_at = DATEADD(second, source.e, GETDATE()) WHEN NOT MATCHED THEN INSERT (profile, item_key, item_value, expires_at) VALUES (source.p, source.k, source.v, DATEADD(second, source.e, GETDATE()));")
            .bind(profile).bind(key).bind(value).bind(ttl_secs as i32).execute(&self.pool).await?;
        Ok(())
    }

    // --- Management ---
    async fn clear_profile(&self, profile: &str) -> Result<()> {
        for table in ["cowen_config", "cowen_secret", "cowen_token", "cowen_audit", "cowen_dlq", "cowen_cache"] {
            let sql = format!("DELETE FROM {} WHERE profile = @p1", table);
            sqlx::query(&sql).bind(profile).execute(&self.pool).await?;
        }
        Ok(())
    }
    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()> {
        for table in ["cowen_config", "cowen_secret", "cowen_token", "cowen_audit", "cowen_dlq", "cowen_cache"] {
            let sql = format!("UPDATE {} SET profile = @p1 WHERE profile = @p2", table);
            sqlx::query(&sql).bind(new_name).bind(old_name).execute(&self.pool).await?;
        }
        Ok(())
    }
    async fn list_all_profiles(&self) -> Result<Vec<String>> {
        let rows = sqlx::query_as::<_, (String,)>("SELECT DISTINCT profile FROM cowen_config")
            .fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn notify_config_changed(&self, _profile: &str, _key: &str) -> Result<()> { Ok(()) }
    async fn watch_config(&self, _profile: &str) -> Result<std::pin::Pin<Box<dyn tokio_stream::Stream<Item = String> + Send>>> {
        Err(anyhow::anyhow!("Notifications not supported for MSSQL"))
    }
}

pub struct MssqlBuilder;
#[async_trait]
impl SqlBuilder for MssqlBuilder {
    fn scheme(&self) -> &str { "mssql" }
    async fn build(&self, url: &str) -> Result<Arc<dyn SqlDriver>> {
        let pool = sqlx::MssqlPool::connect(url).await?;
        
        let ddl = [
            "IF OBJECT_ID('cowen_config', 'U') IS NULL CREATE TABLE cowen_config (profile NVARCHAR(255) NOT NULL, item_key NVARCHAR(255) NOT NULL, item_value NVARCHAR(MAX) NOT NULL, version INT DEFAULT 0, updated_at DATETIMEOFFSET DEFAULT SYSDATETIMEOFFSET(), PRIMARY KEY (profile, item_key))",
            "IF OBJECT_ID('cowen_secret', 'U') IS NULL CREATE TABLE cowen_secret (profile NVARCHAR(255) NOT NULL, item_key NVARCHAR(255) NOT NULL, item_value NVARCHAR(MAX) NOT NULL, updated_at DATETIMEOFFSET DEFAULT SYSDATETIMEOFFSET(), PRIMARY KEY (profile, item_key))",
            "IF OBJECT_ID('cowen_token', 'U') IS NULL CREATE TABLE cowen_token (profile NVARCHAR(255) NOT NULL, item_key NVARCHAR(255) NOT NULL, item_value NVARCHAR(MAX) NOT NULL, expires_at DATETIMEOFFSET NULL, PRIMARY KEY (profile, item_key))",
            "IF OBJECT_ID('cowen_audit', 'U') IS NULL CREATE TABLE cowen_audit (id NVARCHAR(36) PRIMARY KEY, profile NVARCHAR(255) NOT NULL, timestamp DATETIMEOFFSET NOT NULL, level NVARCHAR(20) NOT NULL, target NVARCHAR(255) NOT NULL, message NVARCHAR(MAX) NOT NULL, fields NVARCHAR(MAX))",
            "IF OBJECT_ID('cowen_dlq', 'U') IS NULL CREATE TABLE cowen_dlq (id BIGINT IDENTITY(1,1) PRIMARY KEY, profile NVARCHAR(255) NOT NULL, topic NVARCHAR(255) NOT NULL, payload NVARCHAR(MAX) NOT NULL, retry_count INT DEFAULT 0, error NVARCHAR(MAX), created_at DATETIMEOFFSET DEFAULT SYSDATETIMEOFFSET())",
            "IF OBJECT_ID('cowen_cache', 'U') IS NULL CREATE TABLE cowen_cache (profile NVARCHAR(255) NOT NULL, item_key NVARCHAR(255) NOT NULL, item_value NVARCHAR(MAX) NOT NULL, expires_at DATETIMEOFFSET NULL, PRIMARY KEY (profile, item_key))",
        ];

        for sql in ddl { sqlx::query(sql).execute(&pool).await?; }
        
        // Indices
        let _ = sqlx::query("IF NOT EXISTS (SELECT * FROM sys.indexes WHERE name = 'idx_audit_profile_ts' AND object_id = OBJECT_ID('cowen_audit')) CREATE INDEX idx_audit_profile_ts ON cowen_audit (profile, timestamp)").execute(&pool).await;
        
        Ok(Arc::new(MssqlDriver::new(pool)))
    }
}
