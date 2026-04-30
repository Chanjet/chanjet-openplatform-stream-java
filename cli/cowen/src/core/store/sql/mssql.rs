use anyhow::{Result, anyhow};
use async_trait::async_trait;
use std::sync::Arc;
use deadpool_tiberius::{Pool, Manager};
use super::{SqlDriver, SqlBuilder};
use crate::core::store::{AuditEntry, DlqMessage, Item};

pub struct MssqlDriver {
    pool: Pool,
}

impl MssqlDriver {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    async fn execute(&self, sql: &str, params: &[&dyn tiberius::ToSql]) -> Result<()> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        conn.execute(sql, params).await?;
        Ok(())
    }
}

#[async_trait]
impl SqlDriver for MssqlDriver {
    async fn get_config(&self, profile: &str, key: &str) -> Result<String> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query("SELECT item_value FROM cowen_config WHERE profile = @p1 AND item_key = @p2", &[&profile, &key])
            .await?.into_row().await?
            .ok_or_else(|| anyhow!("Config not found: {}/{}", profile, key))?;
        
        let val: &str = row.get(0).ok_or_else(|| anyhow!("Null value in config"))?;
        Ok(val.to_string())
    }

    async fn get_config_full(&self, profile: &str, key: &str) -> Result<Item> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query(
            "SELECT profile, item_key, item_value, version, updated_at FROM cowen_config WHERE profile = @p1 AND item_key = @p2",
            &[&profile, &key]
        ).await?.into_row().await?
        .ok_or_else(|| anyhow!("Config not found: {}/{}", profile, key))?;

        let p: &str = row.get(0).unwrap();
        let k: &str = row.get(1).unwrap();
        let v: &str = row.get(2).unwrap();
        let ver: i64 = row.get(3).unwrap();
        let ts: chrono::DateTime<chrono::Utc> = row.get(4).unwrap();

        Ok(Item {
            profile: p.to_string(),
            key: k.to_string(),
            value: v.to_string(),
            version: ver as u64,
            updated_at: ts.timestamp(),
        })
    }

    async fn set_config(&self, profile: &str, key: &str, value: &str) -> Result<()> {
        let sql = "
            MERGE cowen_config WITH (HOLDLOCK) AS target
            USING (SELECT @p1 AS p, @p2 AS k) AS source
            ON (target.profile = source.p AND target.item_key = source.k)
            WHEN MATCHED THEN
                UPDATE SET item_value = @p3, version = version + 1, updated_at = GETUTCDATE()
            WHEN NOT MATCHED THEN
                INSERT (profile, item_key, item_value, version)
                VALUES (@p1, @p2, @p3, 0);";
        self.execute(sql, &[&profile, &key, &value]).await
    }

    async fn set_config_conditional(&self, profile: &str, key: &str, value: &str, expected_version: u64) -> Result<()> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let res = conn.execute(
            "UPDATE cowen_config SET item_value = @p1, version = version + 1, updated_at = GETUTCDATE() WHERE profile = @p2 AND item_key = @p3 AND version = @p4",
            &[&value, &profile, &key, &(expected_version as i64)]
        ).await?;
        
        if res.total() == 0 {
            return Err(anyhow!("Conflict or missing record for conditional update"));
        }
        Ok(())
    }

    async fn list_configs(&self, profile: &str) -> Result<Vec<String>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let rows = conn.query("SELECT item_key FROM cowen_config WHERE profile = @p1", &[&profile]).await?.into_first_result().await?;
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap().to_string()).collect())
    }

    async fn delete_config(&self, profile: &str, key: &str) -> Result<()> {
        self.execute("DELETE FROM cowen_config WHERE profile = @p1 AND item_key = @p2", &[&profile, &key]).await
    }

    async fn get_secret(&self, profile: &str, key: &str) -> Result<String> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query("SELECT item_value FROM cowen_secret WHERE profile = @p1 AND item_key = @p2", &[&profile, &key])
            .await?.into_row().await?
            .ok_or_else(|| anyhow!("Secret not found"))?;
        let val: &str = row.get(0).unwrap();
        Ok(val.to_string())
    }

    async fn set_secret(&self, profile: &str, key: &str, value: &str) -> Result<()> {
        let sql = "
            MERGE cowen_secret WITH (HOLDLOCK) AS target
            USING (SELECT @p1 AS p, @p2 AS k) AS source
            ON (target.profile = source.p AND target.item_key = source.k)
            WHEN MATCHED THEN
                UPDATE SET item_value = @p3, updated_at = GETUTCDATE()
            WHEN NOT MATCHED THEN
                INSERT (profile, item_key, item_value)
                VALUES (@p1, @p2, @p3);";
        self.execute(sql, &[&profile, &key, &value]).await
    }

    async fn get_token(&self, profile: &str, key: &str) -> Result<String> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query(
            "SELECT item_value FROM cowen_token WHERE profile = @p1 AND item_key = @p2 AND (expires_at IS NULL OR expires_at > GETUTCDATE())",
            &[&profile, &key]
        ).await?.into_row().await?
        .ok_or_else(|| anyhow!("Token not found or expired"))?;
        let val: &str = row.get(0).unwrap();
        Ok(val.to_string())
    }

    async fn set_token(&self, profile: &str, key: &str, value: &str, expires_in_secs: u64) -> Result<()> {
        let sql = "
            MERGE cowen_token WITH (HOLDLOCK) AS target
            USING (SELECT @p1 AS p, @p2 AS k) AS source
            ON (target.profile = source.p AND target.item_key = source.k)
            WHEN MATCHED THEN
                UPDATE SET item_value = @p3, expires_at = DATEADD(second, @p4, GETUTCDATE())
            WHEN NOT MATCHED THEN
                INSERT (profile, item_key, item_value, expires_at)
                VALUES (@p1, @p2, @p3, DATEADD(second, @p4, GETUTCDATE()));";
        self.execute(sql, &[&profile, &key, &value, &(expires_in_secs as i64)]).await
    }

    async fn list_tokens(&self, profile: &str) -> Result<Vec<String>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let rows = conn.query("SELECT item_key FROM cowen_token WHERE profile = @p1 AND (expires_at IS NULL OR expires_at > GETUTCDATE())", &[&profile]).await?.into_first_result().await?;
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap().to_string()).collect())
    }

    async fn save_audit(&self, entry: &AuditEntry) -> Result<()> {
        let fields_json = serde_json::to_string(&entry.fields)?;
        self.execute(
            "INSERT INTO cowen_audit (id, profile, timestamp, level, target, message, fields) VALUES (@p1, @p2, @p3, @p4, @p5, @p6, @p7)",
            &[&entry.id, &entry.profile, &entry.timestamp, &entry.level, &entry.target, &entry.message, &fields_json]
        ).await
    }

    async fn list_audit(&self, profile: &str, limit: usize) -> Result<Vec<AuditEntry>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let rows = conn.query(
            "SELECT TOP (@p1) id, timestamp, profile, level, target, message, fields FROM cowen_audit WHERE profile = @p2 ORDER BY timestamp DESC",
            &[&(limit as i64), &profile]
        ).await?.into_first_result().await?;
        
        Ok(rows.into_iter().map(|row| AuditEntry {
            id: row.get::<&str, _>(0).unwrap().to_string(),
            timestamp: row.get(1).unwrap(),
            profile: row.get::<&str, _>(2).unwrap().to_string(),
            level: row.get::<&str, _>(3).unwrap().to_string(),
            target: row.get::<&str, _>(4).unwrap().to_string(),
            message: row.get::<&str, _>(5).unwrap().to_string(),
            fields: serde_json::from_str(row.get::<&str, _>(6).unwrap()).unwrap_or_default(),
        }).collect())
    }

    async fn push_dlq(&self, msg: &DlqMessage) -> Result<()> {
        self.execute(
            "INSERT INTO cowen_dlq (profile, topic, payload, retry_count, error) VALUES (@p1, @p2, @p3, @p4, @p5)",
            &[&msg.profile, &msg.topic, &msg.payload, &msg.retry_count, &msg.error]
        ).await
    }

    async fn pop_dlq(&self, profile: &str, topic: &str) -> Result<Option<DlqMessage>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query(
            "SELECT TOP 1 id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p1 AND topic = @p2 ORDER BY id ASC",
            &[&profile, &topic]
        ).await?.into_row().await?;

        if let Some(r) = row {
            let id: i64 = r.get(0).unwrap();
            conn.execute("DELETE FROM cowen_dlq WHERE id = @p1", &[&id]).await?;
            Ok(Some(DlqMessage {
                id: Some(id),
                profile: r.get::<&str, _>(1).unwrap().to_string(),
                topic: r.get::<&str, _>(2).unwrap().to_string(),
                payload: r.get::<&str, _>(3).unwrap().to_string(),
                retry_count: r.get(4).unwrap(),
                error: r.get::<&str, _>(5).map(|s| s.to_string()),
                created_at: r.get(6).unwrap(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn list_dlq(&self, profile: &str, limit: usize) -> Result<Vec<DlqMessage>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let rows = conn.query(
            "SELECT TOP (@p1) id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p2 ORDER BY id DESC",
            &[&(limit as i64), &profile]
        ).await?.into_first_result().await?;
        
        Ok(rows.into_iter().map(|row| DlqMessage {
            id: Some(row.get(0).unwrap()),
            profile: row.get::<&str, _>(1).unwrap().to_string(),
            topic: row.get::<&str, _>(2).unwrap().to_string(),
            payload: row.get::<&str, _>(3).unwrap().to_string(),
            retry_count: row.get(4).unwrap(),
            error: row.get::<&str, _>(5).map(|s| s.to_string()),
            created_at: row.get(6).unwrap(),
        }).collect())
    }

    async fn list_all_dlq(&self, profile: &str) -> Result<Vec<DlqMessage>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let rows = conn.query("SELECT id, profile, topic, payload, retry_count, error, created_at FROM cowen_dlq WHERE profile = @p1", &[&profile]).await?.into_first_result().await?;
        
        Ok(rows.into_iter().map(|row| DlqMessage {
            id: Some(row.get(0).unwrap()),
            profile: row.get::<&str, _>(1).unwrap().to_string(),
            topic: row.get::<&str, _>(2).unwrap().to_string(),
            payload: row.get::<&str, _>(3).unwrap().to_string(),
            retry_count: row.get(4).unwrap(),
            error: row.get::<&str, _>(5).map(|s| s.to_string()),
            created_at: row.get(6).unwrap(),
        }).collect())
    }

    async fn get_cache(&self, profile: &str, key: &str) -> Result<String> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let row = conn.query(
            "SELECT item_value FROM cowen_cache WHERE profile = @p1 AND item_key = @p2 AND (expires_at IS NULL OR expires_at > GETUTCDATE())",
            &[&profile, &key]
        ).await?.into_row().await?
        .ok_or_else(|| anyhow!("Cache entry not found or expired"))?;
        let val: &str = row.get(0).unwrap();
        Ok(val.to_string())
    }

    async fn set_cache(&self, profile: &str, key: &str, value: &str, ttl_secs: u64) -> Result<()> {
        let sql = "
            MERGE cowen_cache WITH (HOLDLOCK) AS target
            USING (SELECT @p1 AS p, @p2 AS k) AS source
            ON (target.profile = source.p AND target.item_key = source.k)
            WHEN MATCHED THEN
                UPDATE SET item_value = @p3, expires_at = DATEADD(second, @p4, GETUTCDATE())
            WHEN NOT MATCHED THEN
                INSERT (profile, item_key, item_value, expires_at)
                VALUES (@p1, @p2, @p3, DATEADD(second, @p4, GETUTCDATE()));";
        self.execute(sql, &[&profile, &key, &value, &(ttl_secs as i64)]).await
    }

    async fn clear_profile(&self, profile: &str) -> Result<()> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        for table in ["cowen_config", "cowen_secret", "cowen_token", "cowen_audit", "cowen_dlq", "cowen_cache"] {
            let sql = format!("DELETE FROM {} WHERE profile = @p1", table);
            conn.execute(sql, &[&profile]).await?;
        }
        Ok(())
    }

    async fn rename_profile(&self, old_name: &str, new_name: &str) -> Result<()> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        for table in ["cowen_config", "cowen_secret", "cowen_token", "cowen_audit", "cowen_dlq", "cowen_cache"] {
            let sql = format!("UPDATE {} SET profile = @p1 WHERE profile = @p2", table);
            conn.execute(sql, &[&new_name, &old_name]).await?;
        }
        Ok(())
    }

    async fn list_all_profiles(&self) -> Result<Vec<String>> {
        let mut conn = self.pool.get().await.map_err(|e| anyhow!("Failed to get MSSQL connection: {}", e))?;
        let rows = conn.query("SELECT DISTINCT profile FROM cowen_config", &[]).await?.into_first_result().await?;
        Ok(rows.into_iter().map(|r| r.get::<&str, _>(0).unwrap().to_string()).collect())
    }

    async fn notify_config_changed(&self, _profile: &str, _key: &str) -> Result<()> { Ok(()) }
    async fn watch_config(&self, _profile: &str) -> Result<std::pin::Pin<Box<dyn tokio_stream::Stream<Item = String> + Send>>> {
        Err(anyhow!("Notifications not supported for MSSQL (Tiberius)"))
    }
}

pub struct MssqlBuilder;
#[async_trait]
impl SqlBuilder for MssqlBuilder {
    fn scheme(&self) -> &str { "mssql" }
    async fn build(&self, url: &str) -> Result<Arc<dyn SqlDriver>> {
        let mgr = Manager::from_jdbc_string(url)
            .map_err(|e| anyhow!("Failed to parse MSSQL URL for pool: {}", e))?;
        let pool = Pool::builder(mgr).max_size(16).build().map_err(|e| anyhow!("Pool build error: {}", e))?;
        
        let mut conn = pool.get().await.map_err(|e| anyhow!("Failed to connect to MSSQL: {}", e))?;
        
        let ddl = [
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_config' AND xtype='U')
             CREATE TABLE cowen_config (profile NVARCHAR(255) NOT NULL, item_key NVARCHAR(255) NOT NULL, item_value NVARCHAR(MAX) NOT NULL, version BIGINT DEFAULT 0, updated_at DATETIME2 DEFAULT GETUTCDATE(), PRIMARY KEY (profile, item_key))",
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_secret' AND xtype='U')
             CREATE TABLE cowen_secret (profile NVARCHAR(255) NOT NULL, item_key NVARCHAR(255) NOT NULL, item_value NVARCHAR(MAX) NOT NULL, updated_at DATETIME2 DEFAULT GETUTCDATE(), PRIMARY KEY (profile, item_key))",
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_token' AND xtype='U')
             CREATE TABLE cowen_token (profile NVARCHAR(255) NOT NULL, item_key NVARCHAR(255) NOT NULL, item_value NVARCHAR(MAX) NOT NULL, expires_at DATETIME2 NULL, PRIMARY KEY (profile, item_key))",
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_audit' AND xtype='U')
             CREATE TABLE cowen_audit (id NVARCHAR(255) PRIMARY KEY, profile NVARCHAR(255) NOT NULL, timestamp DATETIME2 NOT NULL, level NVARCHAR(50) NOT NULL, target NVARCHAR(255) NOT NULL, message NVARCHAR(MAX) NOT NULL, fields NVARCHAR(MAX))",
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_dlq' AND xtype='U')
             CREATE TABLE cowen_dlq (id BIGINT IDENTITY(1,1) PRIMARY KEY, profile NVARCHAR(255) NOT NULL, topic NVARCHAR(255) NOT NULL, payload NVARCHAR(MAX) NOT NULL, retry_count INT DEFAULT 0, error NVARCHAR(MAX), created_at DATETIME2 DEFAULT GETUTCDATE())",
            "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name='cowen_cache' AND xtype='U')
             CREATE TABLE cowen_cache (profile NVARCHAR(255) NOT NULL, item_key NVARCHAR(255) NOT NULL, item_value NVARCHAR(MAX) NOT NULL, expires_at DATETIME2 NULL, PRIMARY KEY (profile, item_key))",
        ];

        for sql in ddl {
            conn.execute(sql, &[]).await?;
        }

        Ok(Arc::new(MssqlDriver::new(pool)))
    }
}

inventory::submit! { super::SqlBuilderRegistration { builder: &MssqlBuilder } }
